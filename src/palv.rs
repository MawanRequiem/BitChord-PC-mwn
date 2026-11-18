//! Palv's browser dashboard can renew an already-issued add-on token. Discord
//! sign-in and the first unlock stay on the site.

use std::time::Duration;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::StatusCode;
use serde::Deserialize;

pub const DASHBOARD: &str = "https://addon.palv.org/";
const RENEW_BEFORE_SECS: u64 = 60 * 60;

pub struct Session {
    token: String,
    expires_at: u64,
}

#[derive(Deserialize)]
struct Claims {
    exp: u64,
}

#[derive(Deserialize)]
struct Reply {
    success: bool,
    token: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Retry,
    Browser,
    RateLimited,
}

impl Session {
    pub fn from_link(link: &str) -> Option<Self> {
        let url = url::Url::parse(link.trim()).ok()?;
        if url.scheme() != "https"
            || url.host_str() != Some("addon.palv.org")
            || url.port().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return None;
        }
        let path = url.path().trim_end_matches('/').strip_prefix('/')?;
        let token = path.strip_suffix("/manifest.json").unwrap_or(path);
        Some(Self {
            token: token.into(),
            expires_at: expiration(token)?,
        })
    }

    pub fn renew_in(&self, unix_now: u64) -> Duration {
        Duration::from_secs(
            self.expires_at
                .saturating_sub(unix_now)
                .saturating_sub(RENEW_BEFORE_SECS),
        )
    }

    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }

    pub fn manifest_link(&self) -> String {
        manifest_link(&self.token)
    }

    fn replacement(&self, token: &str) -> Result<String, Failure> {
        if expiration(token).is_none_or(|exp| exp <= self.expires_at) {
            return Err(Failure::Browser);
        }
        Ok(manifest_link(token))
    }
}

fn manifest_link(token: &str) -> String {
    format!("{DASHBOARD}{token}/manifest.json")
}

fn expiration(token: &str) -> Option<u64> {
    let (header, tail) = token.split_once('.')?;
    let (payload, signature) = tail.split_once('.')?;
    if token.len() > 2048
        || [header, payload, signature].iter().any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        })
    {
        return None;
    }
    // The payload is used for scheduling only. Palv validates the signature.
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: Claims = serde_json::from_slice(&decoded).ok()?;
    (claims.exp > 0).then_some(claims.exp)
}

pub async fn renew(link: &str) -> Result<String, Failure> {
    let session = Session::from_link(link).ok_or(Failure::Browser)?;
    // Only the add-on token is sent, to its original HTTPS origin. Do not follow
    // redirects or reuse the player client's browser headers and account state.
    let client = crate::proxy::apply(reqwest::Client::builder())
        .user_agent(crate::hires::USER_AGENT)
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| Failure::Retry)?;
    let response = client
        .post(DASHBOARD)
        .query(&[("action", "renew_token"), ("token", session.token.as_str())])
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .send()
        .await
        .map_err(|_| Failure::Retry)?;
    check_status(
        response.status(),
        response.headers().contains_key("cf-mitigated"),
    )?;
    let reply: Reply = response.json().await.map_err(|_| Failure::Browser)?;
    let token = reply
        .token
        .filter(|_| reply.success)
        .ok_or(Failure::Browser)?;
    session.replacement(&token)
}

fn check_status(status: StatusCode, challenged: bool) -> Result<(), Failure> {
    if challenged {
        Err(Failure::Browser)
    } else if status.is_success() {
        Ok(())
    } else if status == StatusCode::TOO_MANY_REQUESTS {
        Err(Failure::RateLimited)
    } else if status.is_server_error() {
        Err(Failure::Retry)
    } else {
        Err(Failure::Browser)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(exp: u64) -> String {
        let payload = URL_SAFE_NO_PAD.encode(format!(r#"{{"exp":{exp}}}"#));
        format!("eyJhbGciOiJIUzI1NiJ9.{payload}.signature")
    }

    #[test]
    fn schedules_from_the_token_expiration() {
        let link = format!("https://addon.palv.org/{}/manifest.json", token(20_000));
        let session = Session::from_link(&link).unwrap();
        assert_eq!(session.renew_in(16_000), Duration::from_secs(400));
        assert_eq!(session.renew_in(21_000), Duration::ZERO);
    }

    #[test]
    fn recognizes_only_the_documented_palv_source() {
        let token = token(20_000);
        let session = Session::from_link(&format!("https://addon.palv.org/{token}")).unwrap();
        assert_eq!(
            session.manifest_link(),
            format!("https://addon.palv.org/{token}/manifest.json")
        );
        for url in [
            format!("http://addon.palv.org/{token}"),
            format!("https://addon.palv.org.evil.test/{token}"),
            format!("https://addon.palv.org:8443/{token}"),
            format!("https://user@addon.palv.org/{token}"),
            format!("https://addon.palv.org/extra/{token}"),
            format!("https://addon.palv.org/{token}?token=unrelated"),
            "https://addon.palv.org/manifest.json".into(),
        ] {
            assert!(Session::from_link(&url).is_none());
        }
    }

    #[test]
    fn accepts_only_a_newer_token_at_the_palv_origin() {
        let old = token(20_000);
        let session = Session::from_link(&format!("https://addon.palv.org/{old}")).unwrap();
        let next = token(25_000);
        assert_eq!(
            session.replacement(&next).unwrap(),
            format!("https://addon.palv.org/{next}/manifest.json")
        );
        assert!(session.replacement(&old).is_err());
        assert!(session.replacement(&token(19_000)).is_err());
        assert!(
            session
                .replacement("https://other.example/manifest.json")
                .is_err()
        );
    }

    #[test]
    fn distinguishes_service_failures_from_browser_action() {
        assert_eq!(check_status(StatusCode::OK, false), Ok(()));
        assert_eq!(
            check_status(StatusCode::TOO_MANY_REQUESTS, false),
            Err(Failure::RateLimited)
        );
        assert_eq!(
            check_status(StatusCode::SERVICE_UNAVAILABLE, false),
            Err(Failure::Retry)
        );
        assert_eq!(
            check_status(StatusCode::SERVICE_UNAVAILABLE, true),
            Err(Failure::Browser)
        );
        assert_eq!(
            check_status(StatusCode::UNAUTHORIZED, false),
            Err(Failure::Browser)
        );
        assert_eq!(
            check_status(StatusCode::FOUND, false),
            Err(Failure::Browser)
        );
    }
}
