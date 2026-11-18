use std::fs::File;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use anyhow::Context;
use sha2::{Digest, Sha256};

use super::Playlist;
use crate::model::Track;

const MAX_CSV_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TRACKS: usize = 100_000;
const MAX_PLAYLISTS: usize = 2_000;

#[derive(Default)]
pub struct Import {
    pub playlists: Vec<Playlist>,
    pub track_count: usize,
    pub skipped_rows: usize,
    csv_bytes: u64,
}

pub fn read(paths: &[PathBuf]) -> anyhow::Result<Import> {
    let mut import = Import::default();
    for path in paths {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("Takeout file");
        let file = File::open(path).with_context(|| format!("Could not open {name}"))?;
        let result = match path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("zip") => read_zip(file, &mut import),
            Some(ext) if ext.eq_ignore_ascii_case("csv") => read_csv(name, file, &mut import),
            _ => anyhow::bail!("Choose a Takeout ZIP or playlist CSV"),
        };
        result.with_context(|| format!("Could not import {name}"))?;
    }
    anyhow::ensure!(!import.playlists.is_empty(), "No playable playlist rows found. Choose YouTube and YouTube Music playlists from Google Takeout.");
    let mut seen = std::collections::HashSet::new();
    import.playlists.retain(|playlist| seen.insert(playlist.id.clone()));
    import.track_count = import.playlists.iter().map(|p| p.tracks.len()).sum();
    Ok(import)
}

fn read_csv(name: &str, input: impl Read, import: &mut Import) -> anyhow::Result<()> {
    let remaining = MAX_CSV_BYTES - import.csv_bytes;
    let mut bytes = Vec::new();
    let size = input.take(remaining + 1).read_to_end(&mut bytes)? as u64;
    anyhow::ensure!(size <= remaining, "Too much playlist CSV data. Import fewer files at a time.");
    import.csv_bytes += size;

    let mut csv = csv::ReaderBuilder::new().has_headers(false).flexible(true).trim(csv::Trim::All).from_reader(bytes.as_slice());
    let mut columns = None;
    let mut tracks = Vec::new();
    for row in csv.records() {
        let row = row?;
        let Some((id_col, title_col)) = columns else {
            if let Some(id) = row.iter().position(|name| ["Video ID", "ID Video"].iter().any(|header| name.eq_ignore_ascii_case(header))) {
                let title = row.iter().position(|name| ["Video Title", "Title"].iter().any(|header| name.eq_ignore_ascii_case(header)));
                columns = Some((id, title));
            }
            continue;
        };
        let id = row.get(id_col).unwrap_or_default();
        if id.len() != 11 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
            import.skipped_rows += 1;
            continue;
        }
        anyhow::ensure!(import.track_count < MAX_TRACKS, "Too many tracks. Import fewer playlists at a time.");
        import.track_count += 1;
        let title = title_col.and_then(|i| row.get(i)).filter(|s| !s.is_empty()).unwrap_or(id);
        tracks.push(Track {
            id: id.to_owned(),
            title: title.chars().take(500).collect(),
            artists: Vec::new(),
            album: None,
            duration: None,
            thumbs: Vec::new(),
            track_nr: None,
            plays: None,
            music_video: false,
        });
    }
    if tracks.is_empty() {
        return Ok(());
    }
    anyhow::ensure!(import.playlists.len() < MAX_PLAYLISTS, "Too many playlists. Import fewer files at a time.");
    let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or("Takeout playlist");
    let title = ["-videos", " videos"].iter().find_map(|suffix| stem.strip_suffix(suffix)).unwrap_or(stem);
    let mut key = Sha256::new();
    key.update(title.as_bytes());
    key.update([0]);
    for track in &tracks {
        key.update(track.id.as_bytes());
    }
    import.playlists.push(Playlist { id: format!("takeout-{:x}", key.finalize()), title: title.to_owned(), tracks });
    Ok(())
}

fn read_zip(input: impl Read + Seek, import: &mut Import) -> anyhow::Result<()> {
    let mut zip = zip::ZipArchive::new(input)?;
    for index in 0..zip.len() {
        let raw = zip.by_index_raw(index)?;
        let Some(path) = raw.enclosed_name().filter(|_| raw.is_file() && !raw.is_symlink()) else { continue };
        let Some(folder) = path.parent() else { continue };
        let playlist_folder = folder.file_name().and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("playlists") || name.eq_ignore_ascii_case("playlist"));
        let youtube_folder = folder.parent().and_then(Path::file_name).and_then(|name| name.to_str())
            .is_some_and(|name| name.to_ascii_lowercase().contains("youtube"));
        let csv_file = path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("csv"));
        if playlist_folder && youtube_folder && csv_file {
            drop(raw);
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("Takeout playlist");
            read_csv(name, zip.by_index(index)?, import).with_context(|| format!("Could not read playlist CSV {name}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use super::*;

    #[test]
    fn reads_export_rows_in_order_and_keeps_duplicates() {
        let csv = "\u{feff}Video ID,Playlist Video Creation Timestamp,Video Title\r\nabcDEF12_-3,2026-01-02T03:04:00Z,\"A track, with a comma\"\r\nzxyDEF12_-3,2026-01-03T03:04:00Z,\r\ninvalid,,\r\nabcDEF12_-3,,Repeat\r\n";
        let mut result = Import::default();
        read_csv("Night drive-videos.csv", csv.as_bytes(), &mut result).unwrap();
        assert_eq!(result.skipped_rows, 1);
        let [playlist] = result.playlists.as_slice() else { panic!("expected one playlist") };
        assert_eq!(playlist.title, "Night drive");
        assert_eq!(playlist.tracks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["abcDEF12_-3", "zxyDEF12_-3", "abcDEF12_-3"]);
        assert_eq!(playlist.tracks[0].title, "A track, with a comma");
        assert_eq!(playlist.tracks[1].title, "zxyDEF12_-3");
    }

    #[test]
    fn archive_import_skips_history_and_matches_extracted_csv() {
        let csv = b"Video ID,Playlist video creation timestamp\nabcDEF12_-3,2026-01-02\n";
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in [
            "Takeout/YouTube and YouTube Music/history/watch-history.csv",
            "Takeout/YouTube and YouTube Music/playlists/My mix videos.csv",
        ] {
            zip.start_file(name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).unwrap();
            zip.write_all(csv).unwrap();
        }
        let mut input = zip.finish().unwrap();
        input.set_position(0);
        let mut archive = Import::default();
        read_zip(input, &mut archive).unwrap();
        let [playlist] = archive.playlists.as_slice() else { panic!("expected just the playlist CSV") };
        let mut extracted = Import::default();
        read_csv("My mix videos.csv", &csv[..], &mut extracted).unwrap();
        assert_eq!(playlist.title, "My mix");
        assert_eq!(playlist.id, extracted.playlists[0].id);
    }

    #[test]
    fn selected_csv_snapshots_can_be_saved_and_imported_again() {
        let dir = std::env::temp_dir().join(format!("bitchord-takeout-test-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("My mix-videos.CSV");
        std::fs::write(&path, "Playlist Title,Playlist ID\nMy mix,PL-example\n\nID Video,Timestamp\nabcDEF12_-3,2026-01-02\n").unwrap();
        for _ in 0..2 {
            let import = read(&[path.clone(), path.clone()]).unwrap();
            assert_eq!((import.playlists.len(), import.track_count), (1, 1));
            super::super::save_all(&dir, import.playlists).unwrap();
        }
        let saved = super::super::load(&dir).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].tracks[0].id, "abcDEF12_-3");
        for name in ["My mix-videos.CSV", "local-playlists.json", "local-playlists.lock"] {
            std::fs::remove_file(dir.join(name)).unwrap();
        }
        std::fs::remove_dir(&dir).unwrap();
    }
}
