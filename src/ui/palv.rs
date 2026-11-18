use std::{
    collections::HashMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use super::App;
use crate::{
    backend::Req,
    palv::{Failure, Session},
};

const SCAN_INTERVAL: Duration = Duration::from_secs(60);
const RETRY_DELAY: Duration = Duration::from_secs(600);

pub(super) struct Renewal {
    pub(super) enabled: bool,
    next_scan: Option<Instant>,
    attempts: HashMap<String, Attempt>,
}

enum Attempt {
    Running { retry: bool },
    RetryAt(Instant),
    Paused(&'static str),
}

impl Renewal {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            next_scan: None,
            attempts: HashMap::new(),
        }
    }

    pub(super) fn restart_scan(&mut self) {
        self.next_scan = None;
    }

    pub(super) fn status(&self, link: &str, session: &Session) -> (&'static str, bool) {
        match self.attempts.get(link) {
            Some(Attempt::Running { .. }) => ("Renewing Palv…", false),
            Some(Attempt::RetryAt(_)) if self.enabled => (
                "Palv is temporarily unavailable. BitChord will retry once.",
                false,
            ),
            Some(Attempt::Paused(reason)) => (reason, true),
            _ if session.expires_at() <= unix_now() => ("The Palv session has expired.", true),
            _ if self.enabled => ("BitChord will renew this session near expiry.", false),
            _ => ("Automatic renewal is off.", false),
        }
    }

    pub(super) fn running(&self, link: &str) -> bool {
        matches!(self.attempts.get(link), Some(Attempt::Running { .. }))
    }
}

impl App {
    pub(super) fn tick_palv_renewal(&mut self, ctx: &egui::Context) {
        if !self.palv_renewal.enabled {
            return;
        }
        let now = Instant::now();
        if let Some(next) = self.palv_renewal.next_scan.filter(|&next| next > now) {
            ctx.request_repaint_after(next - now);
            return;
        }
        self.palv_renewal.next_scan = Some(now + SCAN_INTERVAL);
        ctx.request_repaint_after(SCAN_INTERVAL);
        let unix_now = unix_now();
        let due: Vec<_> = self
            .hires_sources
            .iter()
            .filter_map(|source| {
                let session = Session::from_link(source)?;
                let link = session.manifest_link();
                let retry = match self.palv_renewal.attempts.get(&link) {
                    Some(Attempt::RetryAt(at)) if *at <= now => true,
                    None if session.renew_in(unix_now).is_zero() => false,
                    _ => return None,
                };
                Some((link, retry))
            })
            .collect();
        for (link, retry) in due {
            self.start_palv_renewal(link, retry);
        }
    }

    pub(super) fn start_palv_renewal(&mut self, link: String, retry: bool) {
        if self.palv_renewal.running(&link) {
            return;
        }
        self.palv_renewal
            .attempts
            .insert(link.clone(), Attempt::Running { retry });
        self.backend.request(Req::RenewPalv(link));
    }

    pub(super) fn finish_palv_renewal(&mut self, link: String, result: Result<String, Failure>) {
        let attempt = self.palv_renewal.attempts.remove(&link);
        let matches_old = |source: &str| matches_link(source, &link);
        if !self.hires_sources.iter().any(|source| matches_old(source)) {
            return;
        }
        match result {
            Ok(new_link) => {
                for source in self
                    .hires_sources
                    .iter_mut()
                    .filter(|source| matches_old(source))
                {
                    self.hires_checks.remove(source);
                    source.clone_from(&new_link);
                }
                self.hires_sources =
                    super::dedupe_hires(self.hires_sources.iter().map(String::as_str));
                self.hires_sources_changed();
                // A short-lived replacement can still play. Pause automatic renewal
                // if it would immediately send another POST.
                if Session::from_link(&new_link)
                    .is_some_and(|session| session.renew_in(unix_now()).is_zero())
                {
                    self.palv_renewal.attempts.insert(
                        new_link,
                        Attempt::Paused(
                            "Palv issued a short session. Renew in your browser when it expires.",
                        ),
                    );
                }
                if !self.persist_hires_sources() {
                    self.toast("Palv renewed, but BitChord couldn't save the new source URL");
                }
            }
            Err(Failure::Retry)
                if self.palv_renewal.enabled
                    && matches!(attempt, Some(Attempt::Running { retry: false })) =>
            {
                self.palv_renewal
                    .attempts
                    .insert(link, Attempt::RetryAt(Instant::now() + RETRY_DELAY));
            }
            Err(failure) => {
                let reason = match failure {
                    Failure::Retry => "Palv could not be reached for renewal.",
                    Failure::Browser => "Palv declined automatic renewal. Renew in your browser.",
                    Failure::RateLimited => {
                        "Palv rate-limited renewal. Try again later in your browser."
                    }
                };
                self.palv_renewal
                    .attempts
                    .insert(link, Attempt::Paused(reason));
            }
        }
    }

    pub(super) fn hires_sources_changed(&mut self) {
        self.hires_sources_dirty = true;
        self.palv_renewal.restart_scan();
        self.palv_renewal.attempts.retain(|link, attempt| {
            matches!(attempt, Attempt::Running { .. })
                || self
                    .hires_sources
                    .iter()
                    .any(|source| matches_link(source, link))
        });
        self.backend
            .set_hires_source(&self.hires_sources.join("\n"));
    }

    pub(super) fn persist_hires_sources(&mut self) -> bool {
        if self.hires_sources_dirty {
            self.hires_sources_save_failed = crate::vault::set(
                crate::hires::SOURCES_SECRET,
                self.backend.data_dir(),
                self.hires_sources.join("\n").as_bytes(),
            )
            .is_err();
            if self.hires_sources_save_failed {
                log::warn!("Could not save hi-res sources to the credential store");
                return false;
            }
            self.hires_sources_dirty = false;
        }
        true
    }
}

fn matches_link(source: &str, manifest_link: &str) -> bool {
    Session::from_link(source).is_some_and(|session| session.manifest_link() == manifest_link)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
