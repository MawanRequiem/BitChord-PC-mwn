pub const BANDS: [(f32, &str, &str); 16] = [
    (25.0, "25 Hz", "Deep rumble. Boost for subwoofer weight. Cut to remove rumble and free up headroom."),
    (40.0, "40 Hz", "Sub-bass. Boost for deeper bass notes. Cut if the low end shakes or lingers."),
    (63.0, "63 Hz", "Kick and bass thump. Boost for impact. Cut to tame a boomy kick."),
    (100.0, "100 Hz", "Bass punch. Boost to make bass lines fuller. Cut when bass crowds the mix."),
    (160.0, "160 Hz", "Warmth and body. Boost thin recordings. Cut if low notes feel heavy."),
    (250.0, "250 Hz", "Lower-mid thickness. Boost for body. Cut to clear muddy vocals and instruments."),
    (400.0, "400 Hz", "Boxiness and resonance. Boost for fuller guitars. Cut a hollow, boxy sound."),
    (630.0, "630 Hz", "Midrange body. Boost to bring instruments forward. Cut if they sound congested."),
    (1_000.0, "1 kHz", "Vocal and instrument definition. Boost to bring the melody forward. Cut a nasal tone."),
    (1_600.0, "1.6 kHz", "Vocal presence and attack. Boost for distinct words. Cut honk or harshness."),
    (2_500.0, "2.5 kHz", "Speech clarity and guitar bite. Boost for detail. Cut if voices or guitars sound sharp."),
    (4_000.0, "4 kHz", "Consonants and percussion attack. Boost for crisp edges. Cut to soften a fatiguing mix."),
    (6_300.0, "6.3 kHz", "S sounds and cymbal edges. Boost for sparkle. Cut to tame sibilance."),
    (10_000.0, "10 kHz", "Treble shimmer. Boost for brighter cymbals. Cut to reduce hiss or brightness."),
    (12_500.0, "12.5 kHz", "Air and fine detail. Boost for openness. Cut to soften fizzy high treble."),
    (16_000.0, "16 kHz", "Top-end air. Boost for a subtle lift if audible. Cut to soften the highest hiss."),
];

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub bands: [i8; BANDS.len()],
    pub left: u8,
    pub right: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self { bands: [0; BANDS.len()], left: 100, right: 100 }
    }
}

pub(super) struct Processor {
    rate: u32,
    bands: [i8; BANDS.len()],
    filters: [Option<Biquad>; BANDS.len()],
    delay: Vec<[[f32; 2]; BANDS.len()]>,
    gains: [f32; 2],
    master_gain: f32,
    peak_gain: f32,
    headroom: f32,
    equalized: bool,
}

impl Processor {
    pub(super) fn new(channels: u16, rate: u32) -> Self {
        Self {
            rate,
            bands: [0; BANDS.len()],
            filters: [None; BANDS.len()],
            delay: vec![[[0.0; 2]; BANDS.len()]; channels as usize],
            gains: [1.0; 2],
            master_gain: 1.0,
            peak_gain: 1.0,
            headroom: 1.0,
            equalized: false,
        }
    }

    pub(super) fn update(&mut self, settings: Settings, master_gain: f32) {
        self.gains = if self.delay.len() > 1 {
            [settings.left, settings.right].map(|level| f32::from(level.min(100)) / 100.0)
        } else {
            [1.0; 2]
        };
        let bands = settings.bands.map(|db| db.clamp(-12, 12));
        if bands != self.bands {
            for (i, &gain) in bands.iter().enumerate() {
                if gain != self.bands[i] {
                    self.filters[i] = Biquad::peak(BANDS[i].0, self.rate, gain);
                    for channel in &mut self.delay {
                        channel[i] = [0.0; 2];
                    }
                }
            }
            self.bands = bands;
            self.equalized = self.filters.iter().any(Option::is_some);
            self.peak_gain = if bands.iter().any(|&db| db > 0) { self.peak_gain() } else { 1.0 };
        }
        self.master_gain = master_gain;
        let channel_gain = if self.delay.len() == 2 { self.gains[0].max(self.gains[1]) } else { 1.0 };
        let ceiling = (0.97 / (self.peak_gain * channel_gain)).min(1.0);
        // Keep the low end of the volume range unchanged. Above the knee, give
        // the remaining slider travel a rising range that ends at the safe peak.
        let knee = 0.80 * ceiling;
        self.headroom = if self.peak_gain > 1.0 && ceiling < 1.0 && master_gain > knee {
            let travel = (master_gain - knee) / (1.0 - knee);
            (knee + travel * (ceiling - knee)) / master_gain
        } else {
            1.0
        };
    }

    pub(super) fn headroom_db(&self) -> f32 {
        20.0 * self.headroom.log10()
    }

    fn peak_gain(&self) -> f32 {
        let nyquist = f64::from(self.rate) * 0.5;
        let peak = (0..=256)
            .map(|i| 10.0 * (nyquist / 10.0).powf(f64::from(i) / 256.0))
            .chain(BANDS.iter().map(|band| f64::from(band.0)).filter(|&hz| hz < nyquist))
            .map(|hz| {
                let (sin, cos) = (std::f64::consts::TAU * hz / f64::from(self.rate)).sin_cos();
                self.filters.iter().flatten().map(|f| f.power(sin, cos)).product::<f64>()
            })
            .fold(1.0_f64, f64::max);
        peak.sqrt() as f32
    }

    pub(super) fn sample(&mut self, channel: usize, sample: f32) -> f32 {
        let gain = self.gains.get(channel).copied().unwrap_or(1.0) * self.master_gain * self.headroom;
        if !self.equalized {
            return sample * gain;
        }
        let mut output = sample;
        for (filter, delay) in self.filters.iter().zip(&mut self.delay[channel]) {
            if let Some(f) = filter {
                let next = f.b0 * output + delay[0];
                *delay = [f.b1 * output - f.a1 * next + delay[1], f.b2 * output - f.a2 * next];
                output = next;
            }
        }
        output * gain
    }
}

#[derive(Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Biquad {
    fn peak(hz: f32, rate: u32, db: i8) -> Option<Self> {
        if db == 0 || hz >= rate as f32 * 0.5 {
            return None;
        }
        let amplitude = 10.0_f32.powf(f32::from(db) / 40.0);
        let (sin, cos) = (std::f32::consts::TAU * hz / rate as f32).sin_cos();
        let alpha = sin / (2.0 * 1.8);
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
        let power = |c0: f32, c1: f32, c2: f32| {
            let (c0, c1, c2) = (f64::from(c0), f64::from(c1), f64::from(c2));
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
    fn flat_settings_pass_samples_through() {
        let mut output = Processor::new(2, 96_000);
        assert_eq!(output.sample(0, 0.4), 0.4);
        assert_eq!(output.sample(1, -0.000_000_15), -0.000_000_15);
    }

    #[test]
    fn stereo_levels_are_independent_and_leave_other_channels_alone() {
        let mut output = Processor::new(6, 48_000);
        output.update(Settings { left: 25, right: 0, ..Settings::default() }, 1.0);
        assert_eq!(output.sample(0, 0.8), 0.2);
        assert_eq!(output.sample(1, 0.8), 0.0);
        assert_eq!(output.sample(2, 0.8), 0.8);
        let mut mono = Processor::new(1, 48_000);
        mono.update(Settings { left: 0, right: 0, ..Settings::default() }, 1.0);
        assert_eq!(mono.sample(0, 0.8), 0.8);
    }

    #[test]
    fn eq_changes_the_selected_band_and_keeps_channel_history_separate() {
        let mut output = Processor::new(2, 48_000);
        let mut settings = Settings::default();
        settings.bands[8] = -12;
        output.update(settings, 1.0);
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
    fn overlapping_boosts_get_fixed_headroom() {
        let mut settings = Settings::default();
        settings.bands[13] = 12;
        settings.bands[14] = 12;
        let mut output = Processor::new(2, 48_000);
        output.update(settings, 1.0);
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
        settings.bands[8] = 12;
        let mut output = Processor::new(2, 96_000);
        output.update(settings, 0.16);
        assert_eq!(output.headroom_db(), 0.0);
        let mut tail = 0.0;
        for _ in 0..4_800 {
            tail = output.sample(0, 0.75);
        }
        assert!((tail - 0.12).abs() < 0.001, "{tail}");
        output.update(settings, 0.65);
        let mid_gain = output.master_gain * output.headroom;
        output.update(settings, 1.0);
        assert!(output.headroom_db() < -11.0);
        assert!(output.master_gain * output.headroom > mid_gain);
    }
}
