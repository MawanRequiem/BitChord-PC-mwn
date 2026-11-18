use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::Track;

pub mod takeout;

#[derive(Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub title: String,
    pub tracks: Vec<Track>,
}

pub fn load(dir: &Path) -> anyhow::Result<Vec<Playlist>> {
    match std::fs::read(dir.join("local-playlists.json")) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

pub fn save(dir: &Path, playlist: Playlist) -> anyhow::Result<Vec<Playlist>> {
    save_all(dir, [playlist])
}

pub fn save_all(dir: &Path, imported: impl IntoIterator<Item = Playlist>) -> anyhow::Result<Vec<Playlist>> {
    edit(dir, |stored| {
        for playlist in imported {
            if let Some(saved) = stored.iter_mut().find(|saved| saved.id == playlist.id) {
                *saved = playlist;
            } else {
                stored.push(playlist);
            }
        }
    })
}

pub fn remove(dir: &Path, id: &str) -> anyhow::Result<Vec<Playlist>> {
    edit(dir, |playlists| playlists.retain(|playlist| playlist.id != id))
}

fn edit(dir: &Path, update: impl FnOnce(&mut Vec<Playlist>)) -> anyhow::Result<Vec<Playlist>> {
    std::fs::create_dir_all(dir)?;
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join("local-playlists.lock"))?;
    lock.lock()?;
    let mut playlists = load(dir)?;
    update(&mut playlists);
    let pending = dir.join("local-playlists.tmp");
    std::fs::write(&pending, serde_json::to_vec_pretty(&playlists)?)?;
    std::fs::rename(pending, dir.join("local-playlists.json"))?;
    Ok(playlists)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_round_trips_in_order_and_replaces_by_id() {
        let dir = std::env::temp_dir().join(format!("bitchord-local-playlist-test-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let track = |id: &str| Track {
            id: id.into(), title: id.into(), artists: vec![], album: None, duration: None,
            thumbs: vec![], track_nr: None, plays: None, music_video: false,
        };
        let mut snapshot = Playlist { id: "playlist-1".into(), title: "Mix".into(), tracks: vec![track("b"), track("a"), track("b")] };
        save(&dir, snapshot.clone()).unwrap();
        let saved = load(&dir).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].tracks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["b", "a", "b"]);

        snapshot.title = "Updated mix".into();
        save(&dir, snapshot).unwrap();
        let saved = load(&dir).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].title, "Updated mix");
        remove(&dir, "playlist-1").unwrap();
        assert!(load(&dir).unwrap().is_empty());
        std::fs::remove_file(dir.join("local-playlists.json")).unwrap();
        std::fs::remove_file(dir.join("local-playlists.lock")).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}
