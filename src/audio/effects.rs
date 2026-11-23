pub const BANDS: [(f32, &str, &str); 16] = [
    (
        25.0,
        "25 Hz",
        "Deep rumble. Boost for subwoofer weight. Cut to remove rumble and free up headroom.",
    ),
    (
        40.0,
        "40 Hz",
        "Sub-bass. Boost for deeper bass notes. Cut if the low end shakes or lingers.",
    ),
    (
        63.0,
        "63 Hz",
        "Kick and bass thump. Boost for impact. Cut to tame a boomy kick.",
    ),
    (
        100.0,
        "100 Hz",
        "Bass punch. Boost to make bass lines fuller. Cut when bass crowds the mix.",
    ),
    (
        160.0,
        "160 Hz",
        "Warmth and body. Boost thin recordings. Cut if low notes feel heavy.",
    ),
    (
        250.0,
        "250 Hz",
        "Lower-mid thickness. Boost for body. Cut to clear muddy vocals and instruments.",
    ),
    (
        400.0,
        "400 Hz",
        "Boxiness and resonance. Boost for fuller guitars. Cut a hollow, boxy sound.",
    ),
    (
        630.0,
        "630 Hz",
        "Midrange body. Boost to bring instruments forward. Cut if they sound congested.",
    ),
    (
        1_000.0,
        "1 kHz",
        "Vocal and instrument definition. Boost to bring the melody forward. Cut a nasal tone.",
    ),
    (
        1_600.0,
        "1.6 kHz",
        "Vocal presence and attack. Boost for distinct words. Cut honk or harshness.",
    ),
    (
        2_500.0,
        "2.5 kHz",
        "Speech clarity and guitar bite. Boost for detail. Cut if voices or guitars sound sharp.",
    ),
    (
        4_000.0,
        "4 kHz",
        "Consonants and percussion attack. Boost for crisp edges. Cut to soften a fatiguing mix.",
    ),
    (
        6_300.0,
        "6.3 kHz",
        "S sounds and cymbal edges. Boost for sparkle. Cut to tame sibilance.",
    ),
    (
        10_000.0,
        "10 kHz",
        "Treble shimmer. Boost for brighter cymbals. Cut to reduce hiss or brightness.",
    ),
    (
        12_500.0,
        "12.5 kHz",
        "Air and fine detail. Boost for openness. Cut to soften fizzy high treble.",
    ),
    (
        16_000.0,
        "16 kHz",
        "Top-end air. Boost for a subtle lift if audible. Cut to soften the highest hiss.",
    ),
];

pub const FREQUENCY: std::ops::RangeInclusive<f32> = 10.0..=20_000.0;
pub const GAIN: std::ops::RangeInclusive<f32> = -12.0..=12.0;
pub const Q: std::ops::RangeInclusive<f32> = 0.2..=18.0;
pub const PREAMP: std::ops::RangeInclusive<f32> = -24.0..=0.0;

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub bands: [f32; BANDS.len()],
    pub frequency_hz: [f32; BANDS.len()],
    pub q: [f32; BANDS.len()],
    pub enabled: [bool; BANDS.len()],
    pub preamp_db: f32,
    pub auto_headroom: bool,
    pub bypass: bool,
    pub left: u8,
    pub right: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bands: [0.0; BANDS.len()],
            frequency_hz: BANDS.map(|(hz, _, _)| hz),
            q: [1.8; BANDS.len()],
            enabled: [true; BANDS.len()],
            preamp_db: 0.0,
            auto_headroom: true,
            bypass: false,
            left: 100,
            right: 100,
        }
    }
}

impl Settings {
    pub fn sanitized(mut self) -> Self {
        let clamp = |value: f32, fallback, limits: std::ops::RangeInclusive<f32>| {
            if value.is_finite() {
                value.clamp(*limits.start(), *limits.end())
            } else {
                fallback
            }
        };
        for (i, &(hz, _, _)) in BANDS.iter().enumerate() {
            self.bands[i] = clamp(self.bands[i], 0.0, GAIN);
            self.frequency_hz[i] = clamp(self.frequency_hz[i], hz, FREQUENCY);
            self.q[i] = clamp(self.q[i], 1.8, Q);
        }
        self.preamp_db = clamp(self.preamp_db, 0.0, PREAMP);
        self.left = self.left.min(100);
        self.right = self.right.min(100);
        self
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Response {
    rate: u32,
    filters: [Option<Biquad>; BANDS.len()],
    preamp_db: f64,
}

impl Response {
    pub fn new(settings: Settings, rate: u32) -> Self {
        let settings = settings.sanitized();
        Self {
            rate,
            filters: std::array::from_fn(|i| {
                if settings.bypass || !settings.enabled[i] {
                    return None;
                }
                Biquad::peak(
                    f64::from(settings.frequency_hz[i]),
                    rate,
                    f64::from(settings.bands[i]),
                    f64::from(settings.q[i]),
                )
            }),
            preamp_db: if settings.bypass {
                0.0
            } else {
                f64::from(settings.preamp_db)
            },
        }
    }

    pub fn active_bands(&self) -> usize {
        self.filters.iter().flatten().count()
    }

    pub fn db_at(&self, hz: f64) -> f64 {
        10.0 * self.power_at(hz).log10() + self.preamp_db
    }

    fn power_at(&self, hz: f64) -> f64 {
        let (sin, cos) = (std::f64::consts::TAU * hz / f64::from(self.rate)).sin_cos();
        self.filters
            .iter()
            .flatten()
            .map(|filter| filter.power(sin, cos))
            .product()
    }

    fn peak_gain(&self, settings: &Settings) -> f64 {
        if !settings
            .bands
            .iter()
            .enumerate()
            .any(|(i, &db)| db > 0.0 && self.filters[i].is_some())
        {
            return 1.0;
        }
        let nyquist = f64::from(self.rate) * 0.5;
        if nyquist <= 10.0 {
            return 1.0;
        }
        let grid = (0..=384).map(|i| 10.0 * (nyquist / 10.0).powf(f64::from(i) / 384.0));
        // Check neighborhoods of narrow filters as well as the logarithmic grid.
        let centers = (0..BANDS.len())
            .filter(|&i| self.filters[i].is_some())
            .flat_map(|i| {
                let hz = f64::from(settings.frequency_hz[i]);
                let step = hz / (8.0 * f64::from(settings.q[i]));
                (-3..=3).map(move |j| hz + step * f64::from(j))
            });
        grid.chain(centers)
            .filter(|&hz| (0.0..=nyquist).contains(&hz))
            .map(|hz| self.power_at(hz))
            .fold(1.0, f64::max)
            .sqrt()
    }
}

#[derive(Clone, Copy)]
pub(super) struct Prepared {
    pub(super) settings: Settings,
    pub(super) rate: u32,
    response: Response,
    peak_gain: f64,
}

impl Prepared {
    pub(super) fn new(settings: Settings, rate: u32) -> Self {
        let settings = settings.sanitized();
        let response = Response::new(settings, rate);
        Self {
            settings,
            rate,
            response,
            peak_gain: response.peak_gain(&settings),
        }
    }
}

impl Default for Prepared {
    fn default() -> Self {
        Self::new(Settings::default(), 48_000)
    }
}

struct Bank {
    response: Response,
    filtered: bool,
    peak_gain: f64,
    preamp_gain: f64,
    delay: Vec<[[f64; 2]; BANDS.len()]>,
    // The output gain for this block, including master level and protection.
    gain: f64,
}

impl Bank {
    fn new(channels: u16, rate: u32) -> Self {
        Self {
            response: Response::new(Settings::default(), rate),
            filtered: false,
            peak_gain: 1.0,
            preamp_gain: 1.0,
            delay: vec![[[0.0; 2]; BANDS.len()]; usize::from(channels)],
            gain: 1.0,
        }
    }

    fn apply(&mut self, prepared: Prepared) {
        for (i, filter) in prepared.response.filters.iter().enumerate() {
            if *filter != self.response.filters[i] {
                for channel in &mut self.delay {
                    channel[i] = [0.0; 2];
                }
            }
        }
        self.response = prepared.response;
        self.filtered = self.response.active_bands() > 0;
        self.peak_gain = prepared.peak_gain;
        self.preamp_gain = 10.0_f64.powf(self.response.preamp_db / 20.0);
    }

    fn update_gain(&mut self, master: f64, channel_max: f64, auto_headroom: bool) -> f64 {
        let ceiling = (0.97 / (self.peak_gain * self.preamp_gain * channel_max)).min(1.0);
        let knee = 0.80 * ceiling;
        // Preserve low listening levels and keep the remaining volume travel monotonic.
        let headroom = if auto_headroom && self.peak_gain > 1.0 && ceiling < 1.0 && master > knee {
            let travel = (master - knee) / (1.0 - knee);
            (knee + travel * (ceiling - knee)) / master
        } else {
            1.0
        };
        self.gain = self.preamp_gain * master * headroom;
        headroom
    }

    fn sample(&mut self, channel: usize, input: f64) -> f64 {
        if !self.filtered {
            return input * self.gain;
        }
        let mut output = input;
        for (filter, delay) in self.response.filters.iter().zip(&mut self.delay[channel]) {
            if let Some(filter) = filter {
                let next = filter.b0 * output + delay[0];
                *delay = [
                    filter.b1 * output - filter.a1 * next + delay[1],
                    filter.b2 * output - filter.a2 * next,
                ];
                output = next;
            }
        }
        output * self.gain
    }
}

pub(super) struct Processor {
    active: Bank,
    previous: Bank,
    gains: [f64; 2],
    headroom_db: f32,
    fade_frames: u32,
    fade_remaining: u32,
    rendered: bool,
}

impl Processor {
    pub(super) fn new(channels: u16, rate: u32) -> Self {
        Self {
            active: Bank::new(channels, rate),
            previous: Bank::new(channels, rate),
            gains: [1.0; 2],
            headroom_db: 0.0,
            fade_frames: (rate / 100).max(1),
            fade_remaining: 0,
            rendered: false,
        }
    }

    pub(super) fn update(&mut self, prepared: Prepared, master: f32) {
        let channels = self.active.delay.len();
        self.gains = if channels > 1 {
            [prepared.settings.left, prepared.settings.right].map(|level| f64::from(level) / 100.0)
        } else {
            [1.0; 2]
        };
        if self.fade_remaining == 0 && self.active.response != prepared.response {
            if self.rendered {
                self.previous.response = self.active.response;
                self.previous.filtered = self.active.filtered;
                self.previous.peak_gain = self.active.peak_gain;
                self.previous.preamp_gain = self.active.preamp_gain;
                self.previous.delay.copy_from_slice(&self.active.delay);
                self.fade_remaining = self.fade_frames;
            }
            self.active.apply(prepared);
        }

        let master = f64::from(master.clamp(0.0, 1.0));
        let channel_max = if channels == 2 {
            self.gains[0].max(self.gains[1])
        } else {
            1.0
        };
        self.headroom_db = (20.0 * self.active.update_gain(master, channel_max, prepared.settings.auto_headroom).log10()) as f32;
        if self.fade_remaining != 0 {
            self.previous.update_gain(master, channel_max, prepared.settings.auto_headroom);
        }
    }

    pub(super) fn headroom_db(&self) -> f32 {
        self.headroom_db
    }

    pub(super) fn sample(&mut self, channel: usize, sample: f32) -> f32 {
        self.rendered = true;
        let sample = if sample.is_finite() {
            f64::from(sample)
        } else {
            0.0
        };
        let active = self.active.sample(channel, sample);
        let output = if self.fade_remaining == 0 {
            active
        } else {
            // Fade complete, stable filter banks, keeping all channels on the same ramp.
            let old = self.previous.sample(channel, sample);
            let progress = 1.0 - f64::from(self.fade_remaining) / f64::from(self.fade_frames);
            if channel + 1 == self.active.delay.len() {
                self.fade_remaining -= 1;
            }
            old + (active - old) * progress
        };
        (output * self.gains.get(channel).copied().unwrap_or(1.0)) as f32
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Biquad {
    fn peak(hz: f64, rate: u32, db: f64, q: f64) -> Option<Self> {
        if db == 0.0 || hz >= f64::from(rate) * 0.5 {
            return None;
        }
        // RBJ Audio EQ Cookbook, peaking EQ with Q.
        let amplitude = 10.0_f64.powf(db / 40.0);
        let (sin, cos) = (std::f64::consts::TAU * hz / f64::from(rate)).sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha / amplitude;
        Some(Self {
            b0: (1.0 + alpha * amplitude) / a0,
            b1: -2.0 * cos / a0,
            b2: (1.0 - alpha * amplitude) / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha / amplitude) / a0,
        })
    }

    fn power(&self, sin: f64, cos: f64) -> f64 {
        let cos2 = cos * cos - sin * sin;
        let sin2 = 2.0 * sin * cos;
        let power = |c0, c1, c2| {
            let real = c0 + c1 * cos + c2 * cos2;
            let imag = c1 * sin + c2 * sin2;
            real * real + imag * imag
        };
        power(self.b0, self.b1, self.b2) / power(1.0, self.a1, self.a2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_graphic_eq_keeps_its_gains_and_standard_band_settings() {
        let legacy = r#"{"bands":[0,0,0,0,0,0,0,0,-4,0,0,0,0,0,0,0],"left":72,"right":91}"#;
        let settings: Settings = serde_json::from_str(legacy).unwrap();
        assert_eq!(settings.bands[8], -4.0);
        assert_eq!(settings.frequency_hz[8], 1_000.0);
        assert_eq!(settings.q[8], 1.8);
        assert_eq!((settings.left, settings.right), (72, 91));
    }

    #[test]
    fn bell_center_gain_q_and_bypass_follow_the_actual_response() {
        let mut settings = Settings::default();
        settings.bands[8] = -6.25;
        settings.frequency_hz[8] = 1_380.0;
        settings.q[8] = 4.0;
        let response = Response::new(settings, 96_000);
        assert!((response.db_at(1_380.0) + 6.25).abs() < 0.001);
        assert!(response.db_at(690.0) > -0.25);
        settings.q[8] = 0.7;
        assert!(Response::new(settings, 96_000).db_at(690.0) < -2.0);
        settings.bypass = true;
        assert_eq!(Response::new(settings, 96_000).db_at(1_380.0), 0.0);
    }

    #[test]
    fn changing_preamp_fades_from_old_to_new_gain() {
        let mut output = Processor::new(2, 48_000);
        output.update(Prepared::new(Settings::default(), 48_000), 1.0);
        assert_eq!(output.sample(0, 0.5), 0.5);
        assert_eq!(output.sample(1, 0.5), 0.5);
        let settings = Settings {
            preamp_db: -6.0,
            ..Settings::default()
        };
        output.update(Prepared::new(settings, 48_000), 1.0);
        assert_eq!(output.sample(0, 0.5), 0.5);
        for _ in 0..960 {
            output.sample(1, 0.5);
            output.sample(0, 0.5);
        }
        assert!((output.sample(1, 0.5) - 0.2506).abs() < 0.0002);
    }

    #[test]
    fn measured_audio_matches_the_graph_at_common_and_high_res_rates() {
        for rate in [44_100, 48_000, 96_000, 192_000] {
            let mut settings = Settings::default();
            settings.frequency_hz[0] = 120.0;
            settings.q[0] = 12.0;
            settings.bands[0] = 5.5;
            let prepared = Prepared::new(settings, rate);
            let mut output = Processor::new(1, rate);
            output.update(prepared, 0.125);
            let (mut input_power, mut output_power) = (0.0, 0.0);
            for i in 0..rate * 2 {
                let sample = (std::f64::consts::TAU * 120.0 * f64::from(i) / f64::from(rate)).sin();
                let filtered = f64::from(output.sample(0, sample as f32));
                assert!(filtered.is_finite());
                if i >= rate {
                    input_power += (sample * 0.125).powi(2);
                    output_power += filtered.powi(2);
                }
            }
            let measured = 10.0 * (output_power / input_power).log10();
            assert!(
                (measured - prepared.response.db_at(120.0)).abs() < 0.01,
                "{rate}: {measured}"
            );
        }
    }

    #[test]
    fn disabled_and_above_nyquist_bands_are_flat() {
        let mut settings = Settings::default();
        settings.bands[15] = 12.0;
        settings.frequency_hz[15] = 20_000.0;
        assert_eq!(Response::new(settings, 32_000).active_bands(), 0);
        assert_eq!(Response::new(settings, 48_000).active_bands(), 1);
        settings.enabled[15] = false;
        assert_eq!(Response::new(settings, 48_000).db_at(20_000.0), 0.0);
    }

    #[test]
    fn flat_settings_pass_samples_through() {
        let mut output = Processor::new(2, 96_000);
        assert_eq!(output.sample(0, 0.4), 0.4);
        assert_eq!(output.sample(1, -0.000_000_15), -0.000_000_15);
    }

    #[test]
    fn stereo_levels_are_independent_and_leave_other_channels_alone() {
        let mut output = Processor::new(6, 48_000);
        output.update(
            Prepared::new(
                Settings {
                    left: 25,
                    right: 0,
                    ..Settings::default()
                },
                48_000,
            ),
            1.0,
        );
        assert_eq!(output.sample(0, 0.8), 0.2);
        assert_eq!(output.sample(1, 0.8), 0.0);
        assert_eq!(output.sample(2, 0.8), 0.8);
        let mut mono = Processor::new(1, 48_000);
        mono.update(
            Prepared::new(
                Settings {
                    left: 0,
                    right: 0,
                    ..Settings::default()
                },
                48_000,
            ),
            1.0,
        );
        assert_eq!(mono.sample(0, 0.8), 0.8);
    }

    #[test]
    fn eq_changes_the_selected_band_and_keeps_channel_history_separate() {
        let mut output = Processor::new(2, 48_000);
        let mut settings = Settings::default();
        settings.bands[8] = -12.0;
        output.update(Prepared::new(settings, output.active.response.rate), 1.0);
        fn energy(output: &mut Processor, frequency: f32) -> f32 {
            (0..9_600)
                .map(|i| {
                    let sine = (std::f32::consts::TAU * frequency * i as f32 / 48_000.0).sin();
                    let sample = output.sample(0, 0.2 * sine);
                    assert_eq!(output.sample(1, 0.0), 0.0);
                    sample * sample
                })
                .skip(4_800)
                .sum()
        }
        assert!(energy(&mut output, 1_000.0) < energy(&mut output, 4_000.0) * 0.15);
    }

    #[test]
    fn automatic_headroom_is_enabled_by_default() {
        assert!(Settings::default().auto_headroom);
        let legacy: Settings = serde_json::from_str("{}").unwrap();
        assert!(legacy.auto_headroom);
    }

    #[test]
    fn bass_boost_leaves_treble_level_unchanged_without_automatic_headroom() {
        let mut settings = Settings::default();
        settings.auto_headroom = false;
        settings.bands[3] = 9.0;
        let mut output = Processor::new(1, 48_000);
        output.update(Prepared::new(settings, 48_000), 1.0);
        assert_eq!(output.headroom_db(), 0.0);
        let mut input_energy = 0.0_f64;
        let mut output_energy = 0.0_f64;
        for i in 0..24_000 {
            let sample = (std::f32::consts::TAU * 4_000.0 * i as f32 / 48_000.0).sin() * 0.1;
            let processed = output.sample(0, sample);
            if i >= 12_000 {
                input_energy += f64::from(sample).powi(2);
                output_energy += f64::from(processed).powi(2);
            }
        }
        let db = 10.0 * (output_energy / input_energy).log10();
        assert!(db.abs() < 0.05, "treble changed by {db} dB");
    }

    #[test]
    fn automatic_headroom_can_be_enabled_and_disabled_without_changing_filters() {
        let mut settings = Settings::default();
        settings.auto_headroom = false;
        settings.bands[3] = 9.0;
        let mut output = Processor::new(2, 48_000);
        output.update(Prepared::new(settings, 48_000), 1.0);
        assert_eq!(output.headroom_db(), 0.0);
        settings.auto_headroom = true;
        output.update(Prepared::new(settings, 48_000), 1.0);
        assert!(output.headroom_db() < -1.0);
        settings.auto_headroom = false;
        output.update(Prepared::new(settings, 48_000), 1.0);
        assert_eq!(output.headroom_db(), 0.0);
    }

    #[test]
    fn overlapping_boosts_get_fixed_headroom() {
        let mut settings = Settings::default();
        settings.auto_headroom = true;
        settings.bands[13] = 12.0;
        settings.bands[14] = 12.0;
        let mut output = Processor::new(2, 48_000);
        output.update(Prepared::new(settings, output.active.response.rate), 1.0);
        let peak = (0..4_800)
            .map(|i| {
                let angle = std::f32::consts::TAU * 10_390.0 * i as f32 / 48_000.0;
                let sine = angle.sin();
                let full = output.sample(0, sine);
                let half = output.sample(1, sine * 0.5);
                assert!((full * 0.5 - half).abs() < 1e-6);
                full.abs()
            })
            .skip(2_400)
            .fold(0.0_f32, f32::max);
        assert!((0.8..=1.0).contains(&peak), "{peak}");
    }

    #[test]
    fn a_boost_uses_available_volume_headroom_before_reducing_other_bands() {
        let mut settings = Settings::default();
        settings.auto_headroom = true;
        settings.bands[8] = 12.0;
        let mut output = Processor::new(2, 96_000);
        output.update(Prepared::new(settings, 96_000), 0.16);
        assert_eq!(output.headroom_db(), 0.0);
        let mut tail = 0.0;
        for _ in 0..4_800 {
            tail = output.sample(0, 0.75);
        }
        assert!((tail - 0.12).abs() < 0.001, "{tail}");
        output.update(Prepared::new(settings, 96_000), 0.65);
        let mid_gain = output.active.gain;
        output.update(Prepared::new(settings, output.active.response.rate), 1.0);
        assert!(output.headroom_db() < -11.0);
        assert!(output.active.gain > mid_gain);
    }
}
