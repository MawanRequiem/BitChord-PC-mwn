use egui::{Align2, Color32, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::{
    audio::effects::{BANDS, Settings},
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

pub(super) fn show(ui: &mut Ui, settings: &mut Settings, headroom_db: f32) {
    let selection = ui.make_persistent_id("selected-frequency");
    let mut band: usize = ui.data(|data| data.get_temp(selection)).unwrap_or(8);
    let current = PRESETS
        .iter()
        .find(|(_, gains)| gains == &settings.bands)
        .map_or("Custom", |(title, _)| *title);
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("equalizer-preset")
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (label, gains) in PRESETS {
                    if ui.selectable_label(current == label, label).clicked() {
                        settings.bands = gains;
                    }
                }
            });
        if super::widgets::pill(ui, "Reset EQ", None, false).clicked() {
            settings.bands.fill(0);
        }
    });
    ui.add_space(8.0);
    band_graph(ui, &mut settings.bands, &mut band);
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("equalizer-band")
            .selected_text(BANDS[band].1)
            .show_ui(ui, |ui| {
                for (index, (_, label, _)) in BANDS.iter().enumerate() {
                    ui.selectable_value(&mut band, index, *label);
                }
            });
        let gain = &mut settings.bands[band];
        ui.add(egui::Slider::new(gain, -12..=12).suffix(" dB"))
            .on_hover_text("Adjust selected band");
    });
    ui.label(
        RichText::new(BANDS[band].2)
            .font(theme::regular(12.5))
            .color(theme::text_dim()),
    );
    ui.label(RichText::new(format!("Clipping protection · {headroom_db:.1} dB")).font(theme::regular(12.0)).color(theme::text_dim()))
    .on_hover_text("Uses the volume headroom available for this EQ curve. Higher volume settings may need extra reduction to protect peaks.");
    ui.data_mut(|data| data.insert_temp(selection, band));
}

fn band_graph(ui: &mut Ui, gains: &mut [i8; BANDS.len()], selected: &mut usize) {
    let (plot, _) = ui.allocate_exact_size(vec2(ui.available_width(), 186.0), Sense::hover());
    let chart = Rect::from_min_max(plot.min + vec2(39.0, 17.0), plot.max - vec2(16.0, 23.0));
    if !chart.is_positive() {
        return;
    }
    let gain_y =
        |db: i8| egui::remap_clamp(f32::from(db), -12.0..=12.0, chart.bottom()..=chart.top());
    let accent = ui
        .ctx()
        .data(|d| d.get_temp::<Color32>(egui::Id::unique("accent")))
        .unwrap_or(theme::RED);
    let low = BANDS[0].0.ln();
    let high = BANDS[BANDS.len() - 1].0.ln();
    let x = BANDS.map(|(hz, _, _)| egui::remap(hz.ln(), low..=high, chart.left()..=chart.right()));

    for (i, (gain, &at)) in gains.iter_mut().zip(&x).enumerate() {
        let width = (chart.width() / BANDS.len() as f32 * 0.78).max(4.0);
        let area = Rect::from_center_size(
            pos2(at, chart.center().y),
            vec2(width, chart.height() + 18.0),
        )
        .intersect(plot);
        let response = ui.interact(
            area,
            ui.make_persistent_id(("eq-band", i)),
            Sense::click_and_drag(),
        );
        if response.clicked() || response.dragged() {
            *selected = i;
        }
        if response.dragged()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            *gain = egui::remap_clamp(pointer.y, chart.top()..=chart.bottom(), 12.0..=-12.0).round()
                as i8;
        }
        response.on_hover_text(format!("{} · {gain:+} dB", BANDS[i].1));
    }

    let painter = ui.painter();
    painter.rect(
        plot,
        12.0,
        theme::surface_fill(),
        Stroke::new(1.0, theme::glass(0.13)),
        StrokeKind::Inside,
    );
    for db in [-12, -6, 0, 6, 12] {
        let y = gain_y(db);
        painter.hline(
            chart.x_range(),
            y,
            Stroke::new(1.0, theme::glass(if db == 0 { 0.23 } else { 0.09 })),
        );
        let label = if db == 0 {
            "0 dB".into()
        } else {
            format!("{db:+}")
        };
        painter.text(
            pos2(chart.left() - 8.0, y),
            Align2::RIGHT_CENTER,
            label,
            theme::regular(10.5),
            theme::text_dim(),
        );
    }
    for i in [0, 3, 6, 8, 11, 15] {
        painter.text(
            pos2(x[i], chart.bottom() + 9.0),
            Align2::CENTER_TOP,
            BANDS[i].1,
            theme::regular(10.5),
            theme::text_dim(),
        );
    }

    let points: Vec<_> = x
        .iter()
        .zip(gains.iter())
        .map(|(&x, &db)| pos2(x, gain_y(db)))
        .collect();
    painter.add(Shape::line(points.clone(), Stroke::new(2.0, accent)));
    for (i, point) in points.into_iter().enumerate() {
        painter.vline(
            point.x,
            gain_y(gains[i].max(0))..=gain_y(gains[i].min(0)),
            Stroke::new(3.0, theme::with_alpha(accent, 0.30)),
        );
        if i == *selected {
            painter.circle_stroke(point, 8.0, Stroke::new(1.5, accent));
        }
        painter.circle_filled(point, if i == *selected { 4.8 } else { 3.7 }, accent);
    }
}
