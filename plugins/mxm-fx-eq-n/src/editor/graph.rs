//! The graph: a log-frequency axis from 10 Hz to 30 kHz (held below this rate's limit), gain on
//! ±30 dB, the composite curve from the designs the audio runs, the spectra behind it, and points
//! carrying their slot numbers.
//!
//! Mouse, after `plugins/mxm-fx-curve/src/canvas.rs` (plan §5.2):
//! - **Clicking the line adds a band**: a 0 dB bell in the lowest Off slot, so the sound does not
//!   change until it is dragged. A press on the line that becomes a drag adds it and drags it.
//! - **A point is drawn at its band's own gain** (Pro-Q's convention), the composite drawn
//!   separately. A shape with no gain (a cut, the notch, the band pass) sits on the 0 dB line.
//! - **Dragging a point** sets frequency (x) and gain (y); Shift drags at a tenth of the speed.
//! - **The wheel changes Q only over a point**: the selected one while the graph has the keyboard,
//!   or any one with Alt (design system §7.1). Elsewhere it scrolls the view. A turn is one gesture.
//! - **A right-click** opens the shape menu and Delete; **Ctrl/Cmd held on a point solos it** from
//!   the press to the release. BACK during a drag puts the band back where the drag began.
//! - **The keyboard language operates the graph** (`navigation::surface`): see
//!   [`EqApp::on_surface`].
//! - **Band identity never rests on hue**: every point carries its number, the selected one a ring,
//!   a bypassed one is hollow. The spectra are a fill (post) and an outline (pre).

use egui::{Color32, Pos2, Rect, Sense, Shape as Painted, Stroke, Ui, Vec2, pos2, vec2};
use mxm_fx_eq_n_dsp::{
    Band, Cascade, Design, FREQ_MAX, FREQ_MIN, GAIN_RANGE, Q_MAX, Q_MIN, SLOTS, Shape, design,
};
use mxm_ui::theme::Tokens;
use mxm_ui::visual::{AXIS_STROKE, CANVAS_RADIUS, EMPHASIS_STROKE};
use nice_plug::prelude::Param;

use super::spectrum::FLOOR_DB;
use super::{Edge, EqApp, Field, band_index, graph_id};
use crate::params::shape_name;
use crate::view::Show;

pub const POINT_RADIUS: f32 = 9.0;
const HIT_RADIUS: f32 = 12.0;
const LINE_HIT: f32 = 8.0;
/// Where the spectra sit: 0 dB at the top, this at the bottom.
const SPECTRUM_BOTTOM_DB: f32 = -96.0;
/// The wheel's Q step per line, as a share of Q's logarithmic travel.
const WHEEL_Q_FRACTION: f32 = 0.02;

/// The axes, in screen points.
pub struct Axes {
    pub rect: Rect,
    inner: Rect,
    log_low: f32,
    log_high: f32,
}

impl Axes {
    pub fn new(rect: Rect, high: f32) -> Axes {
        let high = high.min(FREQ_MAX as f32);
        Axes {
            rect,
            inner: rect.shrink2(vec2(0.0, 14.0)),
            log_low: (FREQ_MIN as f32).log2(),
            log_high: high.max(FREQ_MIN as f32 * 2.0).log2(),
        }
    }

    pub fn x(&self, freq: f32) -> f32 {
        let t = (freq.max(1.0).log2() - self.log_low) / (self.log_high - self.log_low);
        self.rect.left() + t * self.rect.width()
    }

    pub fn freq(&self, x: f32) -> f32 {
        let t = (x - self.rect.left()) / self.rect.width().max(1.0);
        2f32.powf(self.log_low + t * (self.log_high - self.log_low))
    }

    pub fn y(&self, db: f32) -> f32 {
        self.inner.center().y - db / GAIN_RANGE as f32 * self.inner.height() * 0.5
    }

    pub fn db(&self, y: f32) -> f32 {
        (self.inner.center().y - y) / (self.inner.height() * 0.5).max(1.0) * GAIN_RANGE as f32
    }

    fn spectrum_y(&self, db: f32) -> f32 {
        let t = ((db - SPECTRUM_BOTTOM_DB) / -SPECTRUM_BOTTOM_DB).clamp(0.0, 1.05);
        self.inner.bottom() - t * self.inner.height()
    }
}

/// A point being dragged. `pos` is where the pointer would have it, before clamping, so a fine drag
/// moves it by a tenth of the pointer's travel.
pub struct Drag {
    pub slot: usize,
    pub pos: Pos2,
}

/// What the graph draws this frame, from the bands' designs.
pub struct Curves {
    cascades: [Option<Cascade>; SLOTS],
    pub xs: Vec<f32>,
    pub freqs: Vec<f32>,
    pub composite: Vec<f32>,
    /// The composite at the gains the bands have now, when a dynamic band is moving (plan §7.1):
    /// what is heard, every band with a gain at its published heard gain. The drawn composite stays
    /// the one a click and a drag work on.
    pub moving: Option<Vec<f32>>,
    sample_rate: f64,
}

/// A band's gain is moving when the heard gain is this far from the drawn one, in dB.
const MOVING_DB: f32 = 0.01;

impl Curves {
    pub fn compute(app: &EqApp, axes: &Axes) -> Curves {
        let sample_rate = app.sample_rate();
        let scale = app.gain_scale_now();
        let cascades = std::array::from_fn(|slot| {
            let band = app.band_now(slot);
            let shape = band.shape.filter(|_| !band.bypass)?;
            Some(design(
                Design {
                    shape,
                    slope: band.slope,
                    freq: f64::from(band.freq),
                    gain_db: f64::from(band.gain_db * scale),
                    q: f64::from(band.q),
                },
                sample_rate,
            ))
        });
        // The bands as heard, once a dynamic band moves: every band with a gain at the gain the DSP
        // has it at now, from the audio thread, glide, movement and bound included, before the
        // gain scale. Frequency and Q are not published, so they are drawn as set. A slot's gains
        // are a block old at most, its last band's for that block after a change of band.
        let moving_now = |slot: usize| {
            let band = app.band_now(slot);
            let (drawn, heard) = app.telemetry.gains(slot);
            band.shape.is_some_and(|shape| shape.is_dynamic())
                && !band.bypass
                && (heard - drawn).abs() >= MOVING_DB
        };
        let any_moving = (0..SLOTS).any(moving_now);
        let heard_scale = app.telemetry.heard_scale();
        let heard_cascades: [Option<Cascade>; SLOTS] = std::array::from_fn(|slot| {
            let band = app.band_now(slot);
            let shape = band
                .shape
                .filter(|shape| any_moving && !band.bypass && shape.has_gain())?;
            let (_, heard) = app.telemetry.gains(slot);
            Some(design(
                Design {
                    shape,
                    slope: band.slope,
                    freq: f64::from(band.freq),
                    gain_db: f64::from(heard * heard_scale),
                    q: f64::from(band.q),
                },
                sample_rate,
            ))
        });
        let count = (axes.rect.width() / 2.0).max(2.0) as usize;
        let xs: Vec<f32> = (0..=count)
            .map(|index| axes.rect.left() + axes.rect.width() * index as f32 / count as f32)
            .collect();
        let freqs: Vec<f32> = xs.iter().map(|&x| axes.freq(x)).collect();
        let mut curves = Curves {
            cascades,
            xs,
            composite: Vec::new(),
            moving: None,
            freqs,
            sample_rate,
        };
        curves.composite = curves
            .freqs
            .iter()
            .map(|&freq| curves.composite_at(freq))
            .collect();
        if any_moving {
            let heard: Vec<&Cascade> = (0..SLOTS)
                .filter_map(|slot| {
                    heard_cascades[slot]
                        .as_ref()
                        .or(curves.cascades[slot].as_ref())
                })
                .collect();
            curves.moving = Some(
                curves
                    .freqs
                    .iter()
                    .map(|&freq| {
                        heard
                            .iter()
                            .map(|cascade| cascade.response_db(f64::from(freq), sample_rate))
                            .sum::<f64>() as f32
                    })
                    .collect(),
            );
        }
        curves
    }

    pub fn composite_at(&self, freq: f32) -> f32 {
        self.cascades
            .iter()
            .filter_map(Option::as_ref)
            .map(|cascade| cascade.response_db(f64::from(freq), self.sample_rate))
            .sum::<f64>() as f32
    }

    /// The shortest distance from `point` to the drawn composite line.
    fn distance(&self, axes: &Axes, point: Pos2) -> f32 {
        let mut best = f32::INFINITY;
        for index in 1..self.xs.len() {
            let (x0, x1) = (self.xs[index - 1], self.xs[index]);
            if x1 < point.x - 40.0 || x0 > point.x + 40.0 {
                continue;
            }
            let a = pos2(x0, axes.y(self.composite[index - 1]));
            let b = pos2(x1, axes.y(self.composite[index]));
            let along = b - a;
            let t = ((point - a).dot(along) / along.length_sq().max(1.0e-6)).clamp(0.0, 1.0);
            best = best.min(point.distance(a + along * t));
        }
        best
    }
}

/// `f0 · r(Q)` and `f0 / r(Q)` are a second-order band's edges, with `r(Q) = √(1 + 1/4Q²) + 1/2Q`,
/// so that their difference is `f0 / Q` and their product `f0²`.
pub fn edge_ratio(q: f32) -> f32 {
    let q = q.max(Q_MIN as f32);
    (1.0 + 1.0 / (4.0 * q * q)).sqrt() + 1.0 / (2.0 * q)
}

/// A band's frequency as text, as the graph's readout shows it.
pub fn format_freq(freq: f32) -> String {
    if (freq * 10.0).round() / 10.0 >= 1_000.0 {
        format!("{:.2} kHz", freq / 1_000.0)
    } else {
        format!("{freq:.1} Hz")
    }
}

impl EqApp {
    /// A band as text: its shape, where it runs, and what it has of gain, Q and slope.
    pub(crate) fn point_text(&self, slot: usize) -> String {
        let band = self.band_now(slot);
        let mut text = format!(
            "{}   {}",
            band.shape.map_or("", shape_name),
            format_freq(self.clamp_freq(band.freq))
        );
        if let Some(shape) = band.shape {
            if shape.has_gain() {
                text += &format!("   {:+.1} dB", band.gain_db);
            }
            if shape.has_q(band.slope) {
                text += &format!("   Q {:.3}", band.q);
            }
            if shape.has_slope() {
                text += &format!("   {} dB/oct", band.slope.db_per_octave());
            }
        }
        if band.bypass {
            text += "   bypassed";
        }
        text
    }

    /// Where slot `slot`'s point is drawn: at its own gain, at the frequency it runs at.
    /// Where a dynamic band's Range would take its gain, as heard: the drawn gain plus Range, held
    /// inside ±30 dB, under the gain scale. `None` for a static band or a shape with no dynamics.
    pub(crate) fn range_reach_db(&self, slot: usize) -> Option<f32> {
        let band = self.band_now(slot);
        let range = self.dynamics_now(slot).range_db;
        if !band.shape.is_some_and(|shape| shape.is_dynamic()) || range == 0.0 {
            return None;
        }
        let limit = GAIN_RANGE as f32;
        Some((band.gain_db + range).clamp(-limit, limit) * self.gain_scale_now())
    }

    pub(crate) fn point_position(&self, axes: &Axes, band: &Band) -> Pos2 {
        let scale = self.gain_scale_now();
        let db = match band.shape {
            Some(shape) if shape.has_gain() => band.gain_db * scale,
            _ => 0.0,
        };
        let range = GAIN_RANGE as f32 * 1.1;
        pos2(
            axes.x(self.clamp_freq(band.freq)),
            axes.y(db.clamp(-range, range)),
        )
    }

    pub(crate) fn positions(&self, axes: &Axes) -> Vec<(usize, Pos2)> {
        (0..SLOTS)
            .filter(|&slot| self.band_in_use(slot))
            .map(|slot| {
                let band = self.band_now(slot);
                (slot, self.point_position(axes, &band))
            })
            .collect()
    }

    /// Draws the graph into `size` and handles its pointer.
    pub(crate) fn graph(&mut self, ui: &mut Ui, tokens: &Tokens, size: Vec2) {
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let response = ui.interact(rect, graph_id(), Sense::click_and_drag());
        // One stop of the cursor, which the keyboard language operates directly.
        mxm_ui::navigation::surface(ui, &response, rect);
        self.graph_rect = rect;
        let axes = Axes::new(rect, self.high());

        let curves = Curves::compute(self, &axes);
        self.graph_pointer(ui, &response, &axes, &curves);
        let curves = Curves::compute(self, &axes);
        self.paint_graph(ui, tokens, &axes, &curves);

        let name = match self.selected {
            Some(slot) => format!("Equaliser curve, band {} selected", slot + 1),
            None => "Equaliser curve".to_owned(),
        };
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &name));
        // Every point has a name and a text value (plan §5.4), as `canvas.rs`'s points do. They sense
        // hover only, so the graph keeps every press.
        for (slot, at) in self.positions(&axes) {
            let point = ui.interact(
                Rect::from_center_size(at, Vec2::splat(2.0 * POINT_RADIUS)),
                graph_id().with(("point", slot)),
                Sense::hover(),
            );
            let text = self.point_text(slot);
            point.widget_info(|| {
                let mut info = egui::WidgetInfo::labeled(
                    egui::WidgetType::Other,
                    true,
                    format!("Band {}", slot + 1),
                );
                info.current_text_value = Some(text.clone());
                info
            });
        }
    }

    fn graph_pointer(
        &mut self,
        ui: &mut Ui,
        response: &egui::Response,
        axes: &Axes,
        curves: &Curves,
    ) {
        let positions = self.positions(axes);
        let hit = |at: Pos2| {
            positions
                .iter()
                .map(|&(slot, point)| (slot, point.distance(at)))
                .filter(|&(_, distance)| distance <= HIT_RADIUS)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(slot, _)| slot)
        };
        let modifiers = ui.input(|input| input.modifiers);
        let pointer = response.interact_pointer_pos();

        if response.secondary_clicked() {
            self.context_slot = pointer.and_then(hit);
            if let Some(slot) = self.context_slot {
                self.select(slot);
            }
        }
        response.context_menu(|ui| self.context_menu(ui));

        // Ctrl/Cmd held on a point solos it, from the press to the release.
        if modifiers.command
            && response.is_pointer_button_down_on()
            && let Some(slot) = ui.input(|input| input.pointer.press_origin()).and_then(hit)
        {
            self.pointer_solo = Some(slot);
        }

        if response.clicked()
            && let Some(at) = pointer
        {
            if let Some(slot) = hit(at) {
                self.select(slot);
            } else if curves.distance(axes, at) <= LINE_HIT {
                self.insert_at(axes.freq(at.x));
            } else {
                self.selected = None;
                self.focus_freq = self.clamp_freq(axes.freq(at.x));
            }
        }

        if response.drag_started() {
            let origin = ui
                .input(|input| input.pointer.press_origin())
                .filter(|origin| axes.rect.contains(*origin))
                .or(pointer);
            if let Some(origin) = origin {
                self.close_open();
                // From the line, the new band is made inside the drag's gesture.
                let slot = hit(origin).or_else(|| {
                    (curves.distance(axes, origin) <= LINE_HIT)
                        .then(|| self.insert_opened(axes.freq(origin.x)))
                        .flatten()
                });
                if let Some(slot) = slot {
                    self.select(slot);
                    let band = self.band_now(slot);
                    self.drag = Some(Drag {
                        slot,
                        pos: self.point_position(axes, &band),
                    });
                }
            }
        }

        if response.dragged()
            && let Some(drag) = self.drag.as_mut()
        {
            let fine = if modifiers.shift { 0.1 } else { 1.0 };
            drag.pos += response.drag_delta() * fine;
            let (slot, pos) = (drag.slot, drag.pos);
            let freq = axes.freq(pos.x).clamp(FREQ_MIN as f32, self.high());
            let freq_index = band_index(slot, Field::Freq);
            self.send(freq_index, self.normalised(freq_index, freq));
            self.focus_freq = freq;
            let band = self.band_now(slot);
            let scale = self
                .params
                .gain_scale
                .preview_plain(self.normalised_now(super::GAIN_SCALE_INDEX));
            if band.shape.is_some_and(Shape::has_gain) && scale > f32::EPSILON {
                let range = GAIN_RANGE as f32;
                let gain = (axes.db(pos.y) / scale).clamp(-range, range);
                let gain_index = band_index(slot, Field::Gain);
                self.send(gain_index, self.normalised(gain_index, gain));
            }
        }
        if response.drag_stopped() && self.drag.take().is_some() {
            // BACK during the drag cancels it: the band goes back to where the drag began.
            if mxm_ui::drag::cancelled(ui.ctx(), response.id) {
                self.revert_open();
            } else {
                self.close_open();
            }
        }

        self.wheel_q(ui, axes, &hit, modifiers);
    }

    /// The wheel turns Q, only over a point: the selected one while the graph has the keyboard, or
    /// any one with Alt. A turn is one gesture, ended by a pause.
    fn wheel_q(
        &mut self,
        ui: &mut Ui,
        axes: &Axes,
        hit: &dyn Fn(Pos2) -> Option<usize>,
        modifiers: egui::Modifiers,
    ) {
        let Some(hover) = ui.input(|input| input.pointer.hover_pos()) else {
            return;
        };
        if !axes.rect.contains(hover) {
            return;
        }
        let lines = ui.input(|input| {
            input
                .events
                .iter()
                .map(|event| match event {
                    egui::Event::MouseWheel { unit, delta, .. } => match unit {
                        egui::MouseWheelUnit::Line => delta.y,
                        egui::MouseWheelUnit::Point => delta.y / 50.0,
                        egui::MouseWheelUnit::Page => delta.y * 3.0,
                    },
                    _ => 0.0,
                })
                .sum::<f32>()
        });
        let target = hit(hover).filter(|&slot| {
            modifiers.alt || (self.cursor_on_graph() && self.selected == Some(slot))
        });
        let Some(slot) = target.filter(|_| lines != 0.0) else {
            return;
        };
        let band = self.band_now(slot);
        if !band.shape.is_some_and(|shape| shape.has_q(band.slope)) {
            return;
        }
        let index = band_index(slot, Field::Q);
        if !self.open.holds(index) || self.wheel_until.is_none() {
            self.close_open();
        }
        let travel = (Q_MAX / Q_MIN).ln() as f32;
        let q = (self.current_plain(index) * (lines * WHEEL_Q_FRACTION * travel).exp())
            .clamp(Q_MIN as f32, Q_MAX as f32);
        self.send(index, self.normalised(index, q));
        self.wheel_until = Some(ui.input(|input| input.time) + super::panel::WHEEL_GESTURE_SECONDS);
        self.select(slot);
        ui.input_mut(|input| input.smooth_scroll_delta = Vec2::ZERO);
    }

    fn context_menu(&mut self, ui: &mut Ui) {
        let Some(slot) = self.context_slot.filter(|&slot| self.band_in_use(slot)) else {
            ui.close();
            return;
        };
        let current = self.band_now(slot).shape;
        ui.label(egui::RichText::new(format!("Band {}", slot + 1)).strong());
        for shape in Shape::ALL {
            if ui
                .radio(current == Some(shape), shape_name(shape))
                .clicked()
            {
                self.set_shape(slot, shape);
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Delete").clicked() {
            self.delete(slot);
            ui.close();
        }
    }

    fn paint_graph(&mut self, ui: &mut Ui, tokens: &Tokens, axes: &Axes, curves: &Curves) {
        let rect = axes.rect;
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CANVAS_RADIUS, tokens.surface_1);
        let faint = tokens.text_primary.gamma_multiply(0.12);
        let label_color = tokens.text_secondary;
        let small = egui::FontId::proportional(11.0);

        // Grid.
        for freq in [
            20.0, 50.0, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0, 20_000.0,
        ] {
            let x = axes.x(freq);
            if x <= rect.left() + 1.0 || x >= rect.right() - 1.0 {
                continue;
            }
            // Its label stays inside the graph, or is left out.
            let labelled = x + 28.0 < rect.right();
            painter.line_segment(
                [pos2(x, rect.top()), pos2(x, rect.bottom())],
                Stroke::new(AXIS_STROKE, faint),
            );
            let text = if freq >= 1_000.0 {
                format!("{}k", freq / 1_000.0)
            } else {
                format!("{freq}")
            };
            if labelled {
                painter.text(
                    pos2(x + 3.0, rect.bottom() - 2.0),
                    egui::Align2::LEFT_BOTTOM,
                    text,
                    small.clone(),
                    label_color,
                );
            }
        }
        for step in -5..=5 {
            let db = step as f32 * 6.0;
            let y = axes.y(db);
            let stroke = if step == 0 {
                Stroke::new(AXIS_STROKE, tokens.text_primary.gamma_multiply(0.35))
            } else {
                Stroke::new(AXIS_STROKE, faint)
            };
            painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], stroke);
            if step % 2 == 0 {
                painter.text(
                    pos2(rect.left() + 4.0, y - 1.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{db:+.0}"),
                    small.clone(),
                    label_color,
                );
            }
        }

        // The spectra, behind everything: post as a fill, pre as an outline.
        let settings = self.params.view.get();
        if settings.analyser {
            self.spectrum
                .analyse(&curves.freqs, ui.input(|input| input.time), &settings);
            let points = |values: &[f32]| -> Vec<Pos2> {
                curves
                    .xs
                    .iter()
                    .zip(values)
                    .map(|(&x, &db)| pos2(x, axes.spectrum_y(db)))
                    .collect()
            };
            if matches!(settings.show, Show::Post | Show::Both)
                && self.spectrum.post_db.len() == curves.xs.len()
                && self.spectrum.post_db.iter().any(|db| *db > FLOOR_DB)
            {
                let line = points(&self.spectrum.post_db);
                let fill = tokens.accent.gamma_multiply(0.16);
                let mut mesh = egui::Mesh::default();
                let bottom = axes.inner.bottom();
                for pair in line.windows(2) {
                    let base = mesh.vertices.len() as u32;
                    mesh.colored_vertex(pair[0], fill);
                    mesh.colored_vertex(pair[1], fill);
                    mesh.colored_vertex(pos2(pair[1].x, bottom), fill);
                    mesh.colored_vertex(pos2(pair[0].x, bottom), fill);
                    mesh.add_triangle(base, base + 1, base + 2);
                    mesh.add_triangle(base, base + 2, base + 3);
                }
                painter.add(Painted::mesh(mesh));
                painter.add(Painted::line(
                    line,
                    Stroke::new(AXIS_STROKE, tokens.accent.gamma_multiply(0.45)),
                ));
            }
            if matches!(settings.show, Show::Pre | Show::Both)
                && self.spectrum.pre_db.len() == curves.xs.len()
                && self.spectrum.pre_db.iter().any(|db| *db > FLOOR_DB)
            {
                painter.add(Painted::line(
                    points(&self.spectrum.pre_db),
                    Stroke::new(AXIS_STROKE, tokens.text_secondary.gamma_multiply(0.8)),
                ));
            }
        }

        // The soloed band's region.
        if let Some(slot) = self.soloed().filter(|&slot| self.band_in_use(slot)) {
            let band = self.band_now(slot);
            let freq = self.clamp_freq(band.freq);
            let (low, high) = match band.shape {
                Some(Shape::LowCut | Shape::LowShelf) => (FREQ_MIN as f32, freq),
                Some(Shape::HighCut | Shape::HighShelf) => (freq, 1.0e6),
                _ => {
                    let ratio = edge_ratio(band.q);
                    (freq / ratio, freq * ratio)
                }
            };
            let region = Rect::from_x_y_ranges(
                axes.x(low).max(rect.left())..=axes.x(high).min(rect.right()),
                rect.y_range(),
            );
            painter.rect_filled(region, 0.0, tokens.text_primary.gamma_multiply(0.07));
            for x in [region.left(), region.right()] {
                painter.extend(Painted::dashed_line(
                    &[pos2(x, rect.top()), pos2(x, rect.bottom())],
                    Stroke::new(AXIS_STROKE, tokens.text_primary.gamma_multiply(0.6)),
                    4.0,
                    4.0,
                ));
            }
            painter.text(
                pos2(region.center().x, rect.top() + 4.0),
                egui::Align2::CENTER_TOP,
                format!("SOLO {}", slot + 1),
                egui::FontId::proportional(12.0),
                tokens.text_primary,
            );
        }

        // The composite: the sum the audio makes. With a dynamic band moving, the drawn curve is
        // faint and the curve as heard now is drawn over it.
        let line = |dbs: &[f32]| -> Vec<Pos2> {
            curves
                .xs
                .iter()
                .zip(dbs)
                .map(|(&x, &db)| pos2(x, axes.y(db)))
                .collect()
        };
        match &curves.moving {
            Some(moving) => {
                painter.add(Painted::line(
                    line(&curves.composite),
                    Stroke::new(1.5, tokens.accent.gamma_multiply(0.4)),
                ));
                painter.add(Painted::line(line(moving), Stroke::new(2.0, tokens.accent)));
            }
            None => {
                painter.add(Painted::line(
                    line(&curves.composite),
                    Stroke::new(2.0, tokens.accent),
                ));
            }
        }

        // The edges, once MICRO + ← → took one for MOVE.
        if let (Some(edge), Some(slot)) = (self.edge, self.selected)
            && self.cursor_on_graph()
        {
            let band = self.band_now(slot);
            if band.shape.is_some_and(|shape| shape.has_q(band.slope)) {
                let ratio = edge_ratio(band.q);
                let centre = self.clamp_freq(band.freq);
                for (side, freq) in [(Edge::Start, centre / ratio), (Edge::End, centre * ratio)] {
                    let x = axes.x(freq);
                    let width = if side == edge {
                        EMPHASIS_STROKE
                    } else {
                        AXIS_STROKE
                    };
                    painter.line_segment(
                        [
                            pos2(x, axes.y(GAIN_RANGE as f32)),
                            pos2(x, axes.y(-GAIN_RANGE as f32)),
                        ],
                        Stroke::new(width, tokens.text_primary.gamma_multiply(0.7)),
                    );
                }
            }
        }

        // The keyboard's position on the line, where no band is selected.
        let keyboard = mxm_ui::navigation::shown(ui.ctx()) && self.cursor_on_graph();
        if keyboard && self.selected.is_none() {
            let x = axes.x(self.focus_freq);
            painter.extend(Painted::dashed_line(
                &[pos2(x, rect.top()), pos2(x, rect.bottom())],
                Stroke::new(AXIS_STROKE, tokens.focus),
                3.0,
                3.0,
            ));
            let y = axes.y(curves.composite_at(self.focus_freq));
            painter.add(Painted::convex_polygon(
                vec![
                    pos2(x, y - 6.0),
                    pos2(x + 6.0, y),
                    pos2(x, y + 6.0),
                    pos2(x - 6.0, y),
                ],
                Color32::TRANSPARENT,
                Stroke::new(EMPHASIS_STROKE, tokens.focus),
            ));
        }

        // Points, numbered; a bypassed band hollow, the selected one ringed. A dynamic band has its
        // own ring and a line from the point to where its Range would take it, bounded and scaled
        // as the audio has it, so a gain scale of 0 % leaves no line.
        let number_font = egui::FontId::proportional(11.0);
        for (slot, at) in self.positions(axes) {
            let band = self.band_now(slot);
            if let Some(reach_db) = self.range_reach_db(slot) {
                let reach = pos2(at.x, axes.y(reach_db));
                if (reach.y - at.y).abs() >= 1.0 {
                    let colour = tokens.accent.gamma_multiply(0.6);
                    painter.line_segment([at, reach], Stroke::new(EMPHASIS_STROKE, colour));
                    painter.line_segment(
                        [reach - vec2(5.0, 0.0), reach + vec2(5.0, 0.0)],
                        Stroke::new(EMPHASIS_STROKE, colour),
                    );
                }
                // Dashed for a band that listens to the side chain (plan §7.2).
                let ring = Stroke::new(AXIS_STROKE, tokens.accent);
                if self.key_now(slot) {
                    let radius = POINT_RADIUS + 2.5;
                    let points: Vec<Pos2> = (0..=48)
                        .map(|step| {
                            let angle = std::f32::consts::TAU * step as f32 / 48.0;
                            at + vec2(angle.cos(), angle.sin()) * radius
                        })
                        .collect();
                    painter.extend(Painted::dashed_line(&points, ring, 2.0, 2.0));
                } else {
                    painter.circle_stroke(at, POINT_RADIUS + 2.5, ring);
                }
            }
            if self.selected == Some(slot) {
                painter.circle_stroke(
                    at,
                    POINT_RADIUS + 4.0,
                    Stroke::new(EMPHASIS_STROKE, tokens.text_primary),
                );
            }
            let text_color = if band.bypass {
                painter.circle_stroke(
                    at,
                    POINT_RADIUS,
                    Stroke::new(EMPHASIS_STROKE, tokens.accent),
                );
                tokens.text_primary
            } else {
                painter.circle_filled(at, POINT_RADIUS, tokens.accent);
                tokens.canvas
            };
            painter.text(
                at,
                egui::Align2::CENTER_CENTER,
                format!("{}", slot + 1),
                number_font.clone(),
                text_color,
            );
        }

        // A reading of the selected band, or of the keyboard's position.
        let reading = match self.selected {
            Some(slot) => format!("Band {}   {}", slot + 1, self.point_text(slot)),
            None => format_freq(self.focus_freq),
        };
        painter.text(
            rect.left_top() + vec2(28.0, 6.0),
            egui::Align2::LEFT_TOP,
            reading,
            egui::FontId::monospace(12.0),
            tokens.text_primary,
        );

        // Which view has the keyboard is a border's thickness, not a colour alone.
        painter.rect_stroke(
            rect,
            CANVAS_RADIUS,
            super::panel::view_border(keyboard, tokens),
            egui::StrokeKind::Inside,
        );
    }
}
