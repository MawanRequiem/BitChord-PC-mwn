#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Track;

    fn track(id: &str) -> Track {
        Track { id: id.into(), title: id.into(), artists: vec![], album: None, duration: None, thumbs: vec![], track_nr: None, plays: None, music_video: false }
    }

    #[test]
    fn counts_only_actual_listening_once_per_play() {
        let mut history = History::default();
        for time in 0..35 { history.observe(1, &track("a"), true, time as f64); }
        assert_eq!(history.entries[0].plays, 1);
        for time in 35..75 { history.observe(1, &track("a"), false, time as f64); }
        assert_eq!(history.entries[0].plays, 1);
        for time in 75..110 { history.observe(2, &track("a"), true, time as f64); }
        assert_eq!(history.entries[0].plays, 2);
    }

    #[test]
    fn mixes_are_ranked_and_survive_serialization() {
        let mut history = History::default();
        history.record(track("a"), 10);
        history.record(track("b"), 20);
        history.record(track("a"), 30);
        let history: History = serde_json::from_str(&serde_json::to_string(&history).unwrap()).unwrap();
        assert_eq!(history.on_repeat().iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(history.rediscover().iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["b", "a"]);
    }

    #[test]
    fn pausing_history_and_large_time_jumps_do_not_create_plays() {
        let mut history = History::default();
        history.paused = true;
        for time in 0..40 { history.observe(1, &track("a"), true, time as f64); }
        assert!(history.entries.is_empty());
        history.paused = false;
        history.observe(1, &track("a"), true, 40.0);
        history.observe(1, &track("a"), true, 4000.0);
        assert!(history.entries.is_empty());
    }
}
use crate::model::Track;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct Entry {
    pub(super) track: Track,
    pub(super) plays: u32,
    pub(super) last_played: u64,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct History {
    pub(super) entries: Vec<Entry>,
    pub(super) paused: bool,
    #[serde(skip)]
    listening: Option<Listening>,
}

struct Listening {
    generation: u64,
    last_tick: f64,
    seconds: f64,
    playing: bool,
    counted: bool,
}

impl History {
    pub(super) fn observe(&mut self, generation: u64, track: &Track, playing: bool, now: f64) {
        if self.paused || generation == 0 {
            self.listening = None;
            return;
        }
        let listen = self.listening.get_or_insert_with(|| Listening { generation, last_tick: now, seconds: 0.0, playing, counted: false });
        if listen.generation != generation {
            *listen = Listening { generation, last_tick: now, seconds: 0.0, playing, counted: false };
            return;
        }
        if playing && listen.playing {
            listen.seconds += (now - listen.last_tick).clamp(0.0, 2.0);
        }
        listen.last_tick = now;
        listen.playing = playing;
        if listen.seconds >= 30.0 && !listen.counted {
            listen.counted = true;
            let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
            self.record(track.clone(), timestamp);
        }
    }

    fn record(&mut self, track: Track, now: u64) {
        let plays = self.entries.iter().find(|e| e.track.id == track.id).map_or(1, |e| e.plays.saturating_add(1));
        self.entries.retain(|entry| entry.track.id != track.id);
        self.entries.insert(0, Entry { track, plays, last_played: now });
        self.entries.truncate(500);
    }

    pub(super) fn on_repeat(&self) -> Vec<Track> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|a, b| b.plays.cmp(&a.plays).then(b.last_played.cmp(&a.last_played)));
        entries.into_iter().take(30).map(|entry| entry.track.clone()).collect()
    }

    pub(super) fn rediscover(&self) -> Vec<Track> {
        self.entries.iter().rev().take(30).map(|entry| entry.track.clone()).collect()
    }
}

