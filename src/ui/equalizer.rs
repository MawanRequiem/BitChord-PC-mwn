use egui::{Align2, Color32, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::{
    audio::effects::{BANDS, FREQUENCY, GAIN, PREAMP, Q, Response, Settings},
    theme,
};

const PRESETS: [(&str, [i8; BANDS.len()]); 4] = [
    ("Flat", [0; BANDS.len()]),
    (
        "Bass lift",
        [3, 4, 4, 3, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    (
        "Vocal clarity",
        [-2, -2, -1, 0, 0, -1, 0, 1, 2, 3, 3, 2, 0, 0, 0, 0],
    ),
    (
        "Warm",
        [1, 2, 3, 3, 2, 1, 0, 0, 0, 0, -1, -2, -2, -1, -1, -1],
    ),
];

pub(super) fn show(
    ui: &mut Ui,
    settings: &mut Settings,
    mixer_rate: Option<u32>,
    headroom_db: f32,
) {
    let selection = ui.make_persistent_id("selected-frequency");
    let mut band: usize = ui.data(|data| data.get_temp(selection)).unwrap_or(8);
    let rate = mixer_rate.unwrap_or(48_000);
    let defaults = Settings::default();
    let current = PRESETS
        .iter()
        .find(|(_, gains)| {
            gains.map(f32::from) == settings.bands
                && settings.q == defaults.q
                && settings.frequency_hz == defaults.frequency_hz
                && settings.preamp_db == 0.0
                && settings.enabled == defaults.enabled
        })
        .map_or("Custom", |(title, _)| *title);
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("equalizer-preset")
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (label, gains) in PRESETS {
                    if ui.selectable_label(current == label, label).clicked() {
                        settings.bands = gains.map(f32::from);
                        settings.frequency_hz = defaults.frequency_hz;
                        settings.q = defaults.q;
                        settings.enabled = defaults.enabled;
                        settings.preamp_db = defaults.preamp_db;
                    }
                }
            });
        let mut enabled = !settings.bypass;
        if ui.checkbox(&mut enabled, "EQ enabled").changed() {
            settings.bypass = !enabled;
        }
        ui.label("Preamp");
        ui.add(
            egui::DragValue::new(&mut settings.preamp_db)
                .range(PREAMP)
                .speed(0.1)
                .fixed_decimals(1)
                .suffix(" dB"),
        )
        .on_hover_text("Apply a fixed gain reduction to the EQ output.");
    });

    ui.add_space(8.0);
    band_graph(ui, settings, rate, &mut band);
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("equalizer-band").selected_text(format!("Band {}", band + 1))
            .show_ui(ui, |ui| {
                for (index, &hz) in settings.frequency_hz.iter().enumerate() {
                    ui.selectable_value(&mut band, index, format!("{} · {}", index + 1, hz_label(hz)));
                }
            });
        ui.label("Frequency");
        ui.add(egui::DragValue::new(&mut settings.frequency_hz[band]).range(FREQUENCY).speed(1.0).max_decimals(1).suffix(" Hz"));
        ui.label("Gain");
        ui.add(egui::DragValue::new(&mut settings.bands[band]).range(GAIN).speed(0.1).fixed_decimals(2).suffix(" dB"));
        ui.label("Q");
        ui.add(egui::DragValue::new(&mut settings.q[band]).range(Q).speed(0.02).fixed_decimals(2))
            .on_hover_text("Controls filter width. Higher Q affects a narrower range around the selected frequency.");
        ui.checkbox(&mut settings.enabled[band], "Band enabled");
    });

    let hz = settings.frequency_hz[band];
    if hz >= rate as f32 * 0.5 {
        ui.label(
            RichText::new(
                "This frequency is above the usable range at the current mixer rate.",
            )
            .color(theme::text_dim()),
        );
    } else if let Some((_, _, help)) = BANDS
        .iter()
        .min_by(|a, b| (a.0 / hz).ln().abs().total_cmp(&(b.0 / hz).ln().abs()))
    {
        ui.label(
            RichText::new(*help)
                .font(theme::regular(12.5))
                .color(theme::text_dim()),
        );
    }
    let format = if mixer_rate.is_some() {
        "mixer"
    } else {
        "preview"
    };
    ui.label(
        RichText::new(format!(
            "{} {format} · Auto headroom {headroom_db:.1} dB",
            hz_label(rate as f32),
        ))
        .font(theme::regular(12.0))
        .color(theme::text_dim()),
    )
    .on_hover_text(
        "The graph includes your preamp. Automatic peak protection is reported separately.",
    );
    ui.data_mut(|data| data.insert_temp(selection, band));
}

fn hz_label(hz: f32) -> String {
    if hz < 1_000.0 {
        format!("{hz:.0} Hz")
    } else {
        format!("{} kHz", hz / 1000.0)
    }
}

fn band_graph(ui: &mut Ui, settings: &mut Settings, rate: u32, selected: &mut usize) {
    let (plot, _) = ui.allocate_exact_size(vec2(ui.available_width(), 190.0), Sense::hover());
    let chart = Rect::from_min_max(plot.min + vec2(40.0, 17.0), plot.max - vec2(25.0, 23.0));
    if !chart.is_positive() {
        return;
    }
    let lower = f64::from(*FREQUENCY.start());
    let upper = f64::from(*FREQUENCY.end()).min(f64::from(rate) * 0.5);
    if upper <= lower {
        return;
    }
    let response = Response::new(*settings, rate);
    let mut samples: Vec<_> = (0..=400)
        .map(|i| lower * (upper / lower).powf(f64::from(i) / 400.0))
        .collect();
    samples.extend(
        settings
            .frequency_hz
            .iter()
            .map(|&hz| f64::from(hz))
            .filter(|hz| (lower..=upper).contains(hz)),
    );
    samples.sort_unstable_by(f64::total_cmp);
    let samples: Vec<_> = samples
        .into_iter()
        .map(|hz| (hz, response.db_at(hz) as f32))
        .collect();
    let extent = samples.iter().map(|(_, db)| db.abs()).fold(12.0, f32::max);
    let limit = (extent / 6.0).ceil() * 6.0;
    let gain_y = |db| egui::remap_clamp(db, -limit..=limit, chart.bottom()..=chart.top());
    let frequency_x = |hz: f64| {
        egui::remap(
            hz.ln() as f32,
            lower.ln() as f32..=upper.ln() as f32,
            chart.left()..=chart.right(),
        )
    };
    let accent = ui
        .ctx()
        .data(|d| d.get_temp::<Color32>(egui::Id::unique("accent")))
        .unwrap_or(theme::RED);

    let painter = ui.painter();
    painter.rect(
        plot,
        12.0,
        theme::surface_fill(),
        Stroke::new(1.0, theme::glass(0.13)),
        StrokeKind::Inside,
    );
    for db in [-limit, -limit / 2.0, 0.0, limit / 2.0, limit] {
        let y = gain_y(db);
        painter.hline(
            chart.x_range(),
            y,
            Stroke::new(1.0, theme::glass(if db == 0.0 { 0.23 } else { 0.09 })),
        );
        let label = if db == 0.0 {
            "0 dB".into()
        } else {
            format!("{db:+.0}")
        };
        painter.text(
            pos2(chart.left() - 8.0, y),
            Align2::RIGHT_CENTER,
            label,
            theme::regular(10.5),
            theme::text_dim(),
        );
    }
    for hz in [10.0, 25.0, 100.0, 400.0, 1_000.0, 4_000.0, 20_000.0] {
        if hz <= upper {
            painter.text(
                pos2(frequency_x(hz), chart.bottom() + 9.0),
                Align2::CENTER_TOP,
                hz_label(hz as f32),
                theme::regular(10.5),
                theme::text_dim(),
            );
        }
    }
    let points = samples
        .into_iter()
        .map(|(hz, db)| pos2(frequency_x(hz), gain_y(db)))
        .collect();
    painter.add(Shape::line(points, Stroke::new(2.0, accent)));

    for i in 0..BANDS.len() {
        let hz = settings.frequency_hz[i];
        if f64::from(hz) > upper || hz >= rate as f32 * 0.5 {
            continue;
        }
        let center = pos2(frequency_x(f64::from(hz)), gain_y(settings.bands[i]));
        let hit = Rect::from_center_size(center, vec2(24.0, 24.0));
        let node = ui.interact(
            hit,
            ui.make_persistent_id(("eq-band", i)),
            Sense::click_and_drag(),
        );
        if node.clicked() || node.dragged() {
            *selected = i;
        }
        if node.dragged()
            && let Some(pointer) = node.interact_pointer_pos()
        {
            let db = egui::remap_clamp(pointer.y, chart.top()..=chart.bottom(), limit..=-limit);
            settings.bands[i] = (db.clamp(*GAIN.start(), *GAIN.end()) * 10.0).round() / 10.0;
        }
        let node_color = if settings.bypass || !settings.enabled[i] {
            theme::text_faint()
        } else {
            accent
        };
        if i == *selected {
            painter.circle_stroke(center, 8.0, Stroke::new(1.5, node_color));
        }
        painter.circle_filled(center, if i == *selected { 4.8 } else { 3.7 }, node_color);
        node.on_hover_text(format!(
            "{} · {:+.2} dB · Q {:.2}",
            hz_label(hz),
            settings.bands[i],
            settings.q[i]
        ));
    }
}
