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
    Band, Cascade, Design, FREQ_MAX, FREQ_MIN, GAIN_RANGE, Placement, Q_MAX, Q_MIN, SLOTS, Shape,
    design,
};
use mxm_ui::theme::Tokens;
use mxm_ui::visual::{AXIS_STROKE, CANVAS_RADIUS, EMPHASIS_STROKE};
use nice_plug::prelude::Param;

use super::spectrum::FLOOR_DB;
use super::{Edge, EqApp, Field, band_index, grab, graph_id};
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

    pub fn spectrum_y(&self, db: f32) -> f32 {
        let t = ((db - SPECTRUM_BOTTOM_DB) / -SPECTRUM_BOTTOM_DB).clamp(0.0, 1.05);
        self.inner.bottom() - t * self.inner.height()
    }
}

/// A point being dragged. `pos` is where the pointer would have it, before clamping, so a fine drag
/// moves it by a tenth of the pointer's travel.
pub struct Drag {
    pub slot: usize,
    pub pos: Pos2,
    /// A grab's drag (plan §7.4): the bell stays at its peak and only its gain follows.
    pub gain_only: bool,
    /// A proposal's pair (plan §7.5): the other band, and how far its gain stands from this one's,
    /// so the two move together.
    pub partner: Option<(usize, f32)>,
}

/// Where a proposal's triangle points (plan §7.5): at the spectrum the finder listens to, the
/// pre-EQ line where it is drawn (else the post-EQ one), held inside the graph; on the curve when
/// neither is drawn.
pub fn proposal_marker(axes: &Axes, curves: &Curves, spectrum: Option<&[f32]>, freq: f32) -> Pos2 {
    let x = axes.x(freq);
    let level = spectrum
        .filter(|line| line.len() == curves.xs.len() && line.len() > 1)
        .and_then(|line| {
            let at = curves
                .xs
                .partition_point(|&other| other < x)
                .clamp(1, curves.xs.len() - 1);
            let (x0, x1) = (curves.xs[at - 1], curves.xs[at]);
            let t = ((x - x0) / (x1 - x0).max(f32::MIN_POSITIVE)).clamp(0.0, 1.0);
            let db = line[at - 1] + (line[at] - line[at - 1]) * t;
            (db > FLOOR_DB).then_some(db)
        });
    let y = match level {
        Some(db) => axes.spectrum_y(db),
        None => axes.y(curves.composite_at(freq)),
    };
    let margin = TRIANGLE_HEIGHT + 2.0;
    pos2(
        x,
        y.clamp(axes.rect.top() + margin, axes.rect.bottom() - 2.0),
    )
}

/// A proposal's triangle, pointing down at `at`.
pub fn proposal_triangle(at: Pos2) -> [Pos2; 3] {
    [
        at + vec2(-TRIANGLE_HEIGHT * 0.6, -TRIANGLE_HEIGHT),
        at + vec2(TRIANGLE_HEIGHT * 0.6, -TRIANGLE_HEIGHT),
        at,
    ]
}

const TRIANGLE_HEIGHT: f32 = 10.0;

/// A proposal's label: its frequency and the depth Apply all gives it, a pair's two.
pub fn proposal_text(proposal: &super::finder::Proposal) -> String {
    let freq = format_freq(proposal.freq);
    match proposal.bands.as_slice() {
        [one] => format!("{freq} {:+.1} dB", one.gain_db),
        bands => {
            let depths: Vec<String> = bands
                .iter()
                .map(|band| {
                    let letter = match band.domain {
                        super::finder::Domain::Left => "L",
                        super::finder::Domain::Right => "R",
                        super::finder::Domain::Mid => "M",
                        super::finder::Domain::Side => "S",
                        super::finder::Domain::Average => "",
                    };
                    format!("{letter} {:+.1}", band.gain_db)
                })
                .collect();
            format!("{freq} {} dB", depths.join(" "))
        }
    }
}

/// The hover over a proposal (plan §7.5).
const PROPOSAL_DESCRIPTION: &str =
    "Click to make this cut as a band, or drag up or down to set its depth as you make it.";

/// Where a frozen peak's marker is drawn and found: at its level, held inside the graph, so a peak
/// louder than the top of the drawn range (a loud tone under Tilt) is still seen and grabbed.
pub fn peak_marker(axes: &Axes, peak: &grab::Peak) -> Pos2 {
    let margin = PEAK_MARKER_RADIUS + 2.0;
    pos2(
        axes.x(peak.freq),
        axes.spectrum_y(peak.db)
            .clamp(axes.rect.top() + margin, axes.rect.bottom() - margin),
    )
}

/// A peak's label of `size` for its marker at `at`: above it, or below where the graph's top
/// leaves no room, and shifted sideways into the graph near either edge.
pub fn peak_label(axes: &Axes, at: Pos2, size: Vec2) -> Rect {
    let size = size + vec2(4.0, 2.0);
    let offset = 2.0 * PEAK_MARKER_RADIUS + size.y / 2.0;
    let above = Rect::from_center_size(at - vec2(0.0, offset), size);
    let rect = if above.top() >= axes.rect.top() {
        above
    } else {
        Rect::from_center_size(at + vec2(0.0, offset), size)
    };
    let shift =
        (axes.rect.left() - rect.left()).max(0.0) - (rect.right() - axes.rect.right()).max(0.0);
    rect.translate(vec2(shift, 0.0))
}

const PEAK_MARKER_RADIUS: f32 = 4.0;

/// The bottom-right corner of the side-chain line's "SC", by the line's end at `end`: just above
/// it, held a line of text below the graph's top so a loud key's label is not cut off.
pub fn side_chain_label_at(axes: &Axes, end: Pos2) -> Pos2 {
    let at = end + vec2(-4.0, -4.0);
    pos2(at.x, at.y.max(axes.rect.top() + SIDE_CHAIN_LABEL_HEIGHT))
}

/// Room for the "SC" label's text, at its 11-point size.
const SIDE_CHAIN_LABEL_HEIGHT: f32 = 16.0;

/// The hover over a frozen peak (plan §7.4).
const PEAK_DESCRIPTION: &str =
    "Drag up or down to make a bell at this peak, its width set from the peak's.";
const PEAK_FULL_DESCRIPTION: &str = "Every band slot is in use, so a peak cannot make a bell.";

/// What the graph draws this frame, from the bands' designs.
pub struct Curves {
    cascades: [Option<Cascade>; SLOTS],
    placements: [Placement; SLOTS],
    /// With a band placed off Stereo (plan §7.3), each channel's response to itself; empty
    /// otherwise, when `composite` is the one curve.
    pub channels: Vec<Line>,
    /// `channels` at the gains the bands have now, while a dynamic band moves.
    pub moving_channels: Vec<Line>,
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

/// One channel's response to itself, labelled with the channel's letter (plan §7.3).
pub struct Line {
    pub letter: &'static str,
    pub db: Vec<f32>,
}

/// `(a + jb)(c + jd)`.
fn times((a, b): (f64, f64), (c, d): (f64, f64)) -> (f64, f64) {
    (a * c - b * d, a * d + b * c)
}

fn db_of((re, im): (f64, f64)) -> f32 {
    (10.0 * (re * re + im * im).max(1.0e-30).log10()) as f32
}

/// Each channel's response to itself at `freq` (plan §7.3): the channel stage is `diag(dl, dr)` in
/// the left/right basis, the mid/side stage `diag(dm, ds)` in the mid/side one, run in that order.
/// Left and Right when a Left or Right band is in the sound, Mid and Side when a Mid or Side band
/// is; with one family the other's stage is the identity and each line is exactly what its
/// channel hears.
fn self_responses(
    cascades: &[Option<Cascade>; SLOTS],
    placements: &[Placement; SLOTS],
    freq: f32,
    sample_rate: f64,
) -> [Option<(&'static str, f32)>; 4] {
    let one = (1.0, 0.0);
    let (mut dl, mut dr, mut dm, mut ds) = (one, one, one, one);
    let (mut channels, mut mid_side) = (false, false);
    for (cascade, placement) in cascades.iter().zip(placements) {
        let Some(cascade) = cascade else {
            continue;
        };
        let h = cascade.response(f64::from(freq), sample_rate);
        match placement {
            Placement::Stereo => {
                dl = times(dl, h);
                dr = times(dr, h);
            }
            Placement::Left => {
                dl = times(dl, h);
                channels = true;
            }
            Placement::Right => {
                dr = times(dr, h);
                channels = true;
            }
            Placement::Mid => {
                dm = times(dm, h);
                mid_side = true;
            }
            Placement::Side => {
                ds = times(ds, h);
                mid_side = true;
            }
        }
    }
    let half = |(a, b): (f64, f64), (c, d): (f64, f64), sign: f64| {
        (0.5 * (a + sign * c), 0.5 * (b + sign * d))
    };
    let mut lines = [None; 4];
    if channels {
        // L→L = ½(dm + ds)·dl, R→R = ½(dm + ds)·dr.
        let both = half(dm, ds, 1.0);
        lines[0] = Some(("L", db_of(times(both, dl))));
        lines[1] = Some(("R", db_of(times(both, dr))));
    }
    if mid_side {
        // M→M = dm·½(dl + dr), S→S = ds·½(dl + dr).
        let both = half(dl, dr, 1.0);
        lines[2] = Some(("M", db_of(times(dm, both))));
        lines[3] = Some(("S", db_of(times(ds, both))));
    }
    lines
}

/// Every line of `self_responses` over `freqs`, in L, R, M, S order.
fn lines_over(
    cascades: &[Option<Cascade>; SLOTS],
    placements: &[Placement; SLOTS],
    freqs: &[f32],
    sample_rate: f64,
) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    for (index, &freq) in freqs.iter().enumerate() {
        for (slot, line) in self_responses(cascades, placements, freq, sample_rate)
            .into_iter()
            .flatten()
            .enumerate()
        {
            if index == 0 {
                lines.push(Line {
                    letter: line.0,
                    db: Vec::with_capacity(freqs.len()),
                });
            }
            lines[slot].db.push(line.1);
        }
    }
    lines
}

impl Curves {
    pub fn compute(app: &EqApp, axes: &Axes) -> Curves {
        let sample_rate = app.sample_rate();
        let scale = app.gain_scale_now();
        // A placed band in mono is not heard, so it is not drawn into the curve (plan §7.3).
        let placements: [Placement; SLOTS] = std::array::from_fn(|slot| app.placement_now(slot));
        let cascades = std::array::from_fn(|slot| {
            let band = app.band_now(slot);
            let shape = band
                .shape
                .filter(|_| !band.bypass && app.placement_heard(slot))?;
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
            let shape = band.shape.filter(|shape| {
                any_moving && !band.bypass && shape.has_gain() && app.placement_heard(slot)
            })?;
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
            placements,
            channels: Vec::new(),
            moving_channels: Vec::new(),
            xs,
            composite: Vec::new(),
            moving: None,
            freqs,
            sample_rate,
        };
        let placed = (0..SLOTS).any(|slot| {
            curves.cascades[slot].is_some() && curves.placements[slot] != Placement::Stereo
        });
        if placed {
            curves.channels = lines_over(
                &curves.cascades,
                &curves.placements,
                &curves.freqs,
                sample_rate,
            );
            if any_moving {
                let heard: [Option<Cascade>; SLOTS] =
                    std::array::from_fn(|slot| heard_cascades[slot].or(curves.cascades[slot]));
                curves.moving_channels =
                    lines_over(&heard, &curves.placements, &curves.freqs, sample_rate);
            }
        }
        curves.composite = curves
            .freqs
            .iter()
            .map(|&freq| curves.composite_at(freq))
            .collect();
        if any_moving && !placed {
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

    /// The line a click and the keyboard work on at `freq`: the one curve, or with placed bands
    /// the first channel line.
    pub fn composite_at(&self, freq: f32) -> f32 {
        if !self.channels.is_empty() {
            return self_responses(&self.cascades, &self.placements, freq, self.sample_rate)
                .into_iter()
                .flatten()
                .next()
                .map_or(0.0, |(_, db)| db);
        }
        self.cascades
            .iter()
            .filter_map(Option::as_ref)
            .map(|cascade| cascade.response_db(f64::from(freq), self.sample_rate))
            .sum::<f64>() as f32
    }

    /// The lines drawn: the one curve, or with placed bands each channel's.
    fn drawn(&self) -> Vec<&[f32]> {
        if self.channels.is_empty() {
            vec![&self.composite]
        } else {
            self.channels
                .iter()
                .map(|line| line.db.as_slice())
                .collect()
        }
    }

    /// The shortest distance from `point` to a drawn line.
    pub(crate) fn distance(&self, axes: &Axes, point: Pos2) -> f32 {
        let mut best = f32::INFINITY;
        for db in self.drawn() {
            for index in 1..self.xs.len() {
                let (x0, x1) = (self.xs[index - 1], self.xs[index]);
                if x1 < point.x - 40.0 || x0 > point.x + 40.0 {
                    continue;
                }
                let a = pos2(x0, axes.y(db[index - 1]));
                let b = pos2(x1, axes.y(db[index]));
                let along = b - a;
                let t = ((point - a).dot(along) / along.length_sq().max(1.0e-6)).clamp(0.0, 1.0);
                best = best.min(point.distance(a + along * t));
            }
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
        // Its placement, always in a stereo layout; in mono only a placed band's (plan §7.3).
        let placement = self.placement_now(slot);
        if !self.telemetry.mono() || placement != Placement::Stereo {
            text += &format!("   {}", crate::params::placement_name(placement));
        }
        if !self.placement_heard(slot) {
            text += ", not heard in mono";
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
        // A band the layout does not hear moves nothing, so it has no range to show (plan §7.3).
        if !band.shape.is_some_and(|shape| shape.is_dynamic())
            || range == 0.0
            || !self.placement_heard(slot)
        {
            return None;
        }
        let limit = GAIN_RANGE as f32;
        Some((band.gain_db + range).clamp(-limit, limit) * self.gain_scale_now())
    }

    /// The spectrum line a proposal's triangle points at: the pre-EQ line where it is drawn, else
    /// the post-EQ one.
    pub(crate) fn proposal_spectrum(&self) -> Option<&[f32]> {
        let settings = self.params.view.get();
        if !settings.analyser {
            return None;
        }
        let [pre, post, _] = self.shown_spectra();
        match settings.show {
            Show::Pre | Show::Both => Some(pre),
            Show::Post => Some(post),
        }
    }

    /// The proposal at `at` (plan §7.5): its drawn label, or the nearest triangle within the hit
    /// radius.
    pub(crate) fn proposal_at(&self, axes: &Axes, curves: &Curves, at: Pos2) -> Option<usize> {
        if !self.find {
            return None;
        }
        let labelled = self
            .proposal_labels
            .iter()
            .find(|(index, rect)| rect.contains(at) && *index < self.proposals.len())
            .map(|&(index, _)| index);
        if labelled.is_some() {
            return labelled;
        }
        let spectrum = self.proposal_spectrum();
        self.proposals
            .iter()
            .enumerate()
            .map(|(index, proposal)| {
                let tip = proposal_marker(axes, curves, spectrum, proposal.freq);
                let centre = tip - vec2(0.0, TRIANGLE_HEIGHT / 2.0);
                (index, centre.distance(at))
            })
            .filter(|&(_, distance)| distance <= HIT_RADIUS)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }

    /// The lines the proposals would make (plan §7.5), over `curves`' frequencies: the one curve,
    /// or with placed bands among the EQ and the proposals each channel's.
    fn proposal_lines(&self, curves: &Curves) -> Vec<Vec<f32>> {
        use super::finder::{Domain, domain_db, stages};
        let heard = self.heard_eq();
        let rate = heard.sample_rate;
        let mut bands: Vec<(Cascade, Placement)> = heard.bands;
        for proposal in &self.proposals {
            for band in &proposal.bands {
                bands.push((band.cascade(rate), band.domain.placement()));
            }
        }
        let channels = bands
            .iter()
            .any(|(_, placement)| matches!(placement, Placement::Left | Placement::Right));
        let mid_side = bands.iter().any(|(_, placement)| placement.is_mid_side());
        let domains: Vec<Domain> = match (channels, mid_side) {
            (false, false) => vec![Domain::Left],
            (true, false) => vec![Domain::Left, Domain::Right],
            (false, true) => vec![Domain::Mid, Domain::Side],
            (true, true) => vec![Domain::Left, Domain::Right, Domain::Mid, Domain::Side],
        };
        let mut lines = vec![Vec::with_capacity(curves.freqs.len()); domains.len()];
        for &freq in &curves.freqs {
            let diagonal = stages(bands.iter().map(|(c, p)| (c, *p)), freq, rate);
            for (line, &domain) in lines.iter_mut().zip(&domains) {
                line.push(domain_db(domain, diagonal));
            }
        }
        lines
    }

    /// The frozen peak at `at`: its drawn label, or the nearest marker within the hit radius.
    pub(crate) fn peak_at(&self, axes: &Axes, at: Pos2) -> Option<grab::Peak> {
        let frozen = self.frozen.as_ref()?;
        let labelled = self
            .peak_labels
            .iter()
            .find(|(_, rect)| rect.contains(at))
            .and_then(|&(index, _)| frozen.peaks.get(index).copied());
        if labelled.is_some() {
            return labelled;
        }
        frozen
            .peaks
            .iter()
            .map(|peak| (*peak, peak_marker(axes, peak).distance(at)))
            .filter(|&(_, distance)| distance <= HIT_RADIUS)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(peak, _)| peak)
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
        // A freeze made at another width ends before the pointer reaches it (plan §7.4).
        if self
            .frozen
            .as_ref()
            .is_some_and(|frozen| !frozen.drawn_on(&curves.freqs))
        {
            self.unfreeze();
        }
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

        // A proposal under the pointer says what a click and a drag do (plan §7.5).
        if let Some(at) = response.hover_pos()
            && hit(at).is_none()
            && self.proposal_at(axes, curves, at).is_some()
        {
            response
                .clone()
                .on_hover_text_at_pointer(PROPOSAL_DESCRIPTION);
        }
        // A frozen peak under the pointer says what a drag from it does, or why it cannot.
        if let Some(at) = response.hover_pos()
            && hit(at).is_none()
            && self.peak_at(axes, at).is_some()
        {
            let text = if self.free_slot_exists() {
                PEAK_DESCRIPTION
            } else {
                PEAK_FULL_DESCRIPTION
            };
            response.clone().on_hover_text_at_pointer(text);
        }

        if response.secondary_clicked() {
            self.context_slot = pointer.and_then(hit);
            if let Some(slot) = self.context_slot {
                self.select(slot);
            }
        }
        // MENU opens the same menu a right-click does, at the selected band's point.
        let keys = std::mem::take(&mut self.menu_asked)
            .then(|| {
                let slot = self.selected?;
                positions
                    .iter()
                    .find(|(other, _)| *other == slot)
                    .map(|(_, at)| Rect::from_center_size(*at, Vec2::splat(2.0 * POINT_RADIUS)))
            })
            .flatten();
        mxm_ui::reach::context_menu(response, keys, |ui| self.context_menu(ui));

        // Ctrl/Cmd held on a point solos it, from the press to the release.
        if modifiers.command
            && response.is_pointer_button_down_on()
            && let Some(slot) = ui.input(|input| input.pointer.press_origin()).and_then(hit)
        {
            self.pointer_solo = Some(slot);
        }

        if let Some(at) = ui.input(|input| {
            input.events.iter().rev().find_map(|event| match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } => Some(*pos),
                _ => None,
            })
        }) {
            self.press_at = Some(at);
        }
        if response.clicked()
            && let Some(at) = self.press_at.or(pointer)
        {
            // A click on a proposal applies it (plan §7.5); a click on a peak makes nothing (plan
            // §7.4), even where its marker lies on the curve.
            if let Some(slot) = hit(at) {
                self.select(slot);
            } else if let Some(index) = self.proposal_at(axes, curves, at) {
                self.apply_proposals(&[index], false);
            } else if curves.distance(axes, at) <= LINE_HIT && self.peak_at(axes, at).is_none() {
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
                // From a frozen peak, the drag grabs it (plan §7.4); from the line, the new band
                // is made inside the drag's gesture.
                let point = hit(origin);
                // From a proposal, the drag applies it inside its gesture, its gain following
                // (plan §7.5); a pair's two bands move together.
                let applied: Vec<usize> = match point {
                    None => self
                        .proposal_at(axes, curves, origin)
                        .map(|index| self.apply_proposals(&[index], true))
                        .unwrap_or_default(),
                    Some(_) => Vec::new(),
                };
                let peak = (point.is_none() && applied.is_empty())
                    .then(|| self.peak_at(axes, origin))
                    .flatten();
                let grabbed = applied
                    .first()
                    .copied()
                    .or_else(|| peak.and_then(|peak| self.insert_peak(peak, true)));
                let slot = point.or(grabbed).or_else(|| {
                    (curves.distance(axes, origin) <= LINE_HIT)
                        .then(|| self.insert_opened(axes.freq(origin.x)))
                        .flatten()
                });
                if let Some(slot) = slot {
                    self.select(slot);
                    let band = self.band_now(slot);
                    let partner = applied
                        .get(1)
                        .map(|&other| (other, self.band_now(other).gain_db - band.gain_db));
                    self.drag = Some(Drag {
                        slot,
                        pos: self.point_position(axes, &band),
                        gain_only: grabbed.is_some(),
                        partner,
                    });
                }
            }
        }

        if response.dragged()
            && let Some(drag) = self.drag.as_mut()
        {
            let fine = if modifiers.shift { 0.1 } else { 1.0 };
            drag.pos += response.drag_delta() * fine;
            let (slot, pos, gain_only, partner) =
                (drag.slot, drag.pos, drag.gain_only, drag.partner);
            if !gain_only {
                let freq = axes.freq(pos.x).clamp(FREQ_MIN as f32, self.high());
                let freq_index = band_index(slot, Field::Freq);
                self.send(freq_index, self.normalised(freq_index, freq));
                self.focus_freq = freq;
            }
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
                if let Some((other, offset)) = partner {
                    let other_index = band_index(other, Field::Gain);
                    let other_gain = (gain + offset).clamp(-range, range);
                    self.send(other_index, self.normalised(other_index, other_gain));
                }
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
        let mut rows = Vec::new();
        let mut current_row = None;
        let mut chosen = false;
        for shape in Shape::ALL {
            let row = ui.radio(current == Some(shape), shape_name(shape));
            if row.clicked() {
                self.set_shape(slot, shape);
                ui.close();
                chosen = true;
            }
            if current == Some(shape) {
                current_row = Some(row.clone());
            }
            rows.push(row);
        }
        ui.separator();
        let delete = ui.button("Delete");
        if delete.clicked() {
            self.delete(slot);
            ui.close();
            chosen = true;
        }
        rows.push(delete);
        // The keys walk the menu (the owner, 2026-10-15): an open menu has the keyboard (the
        // shared navigation is inert), and egui moves focus with ↑ ↓ and clicks with Enter or Space
        // only from a focused row. While none of its rows has it, the current shape's takes it.
        if !chosen && !rows.iter().any(egui::Response::has_focus) {
            current_row
                .unwrap_or_else(|| rows[0].clone())
                .request_focus();
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
        if !settings.analyser && (self.freezing() || self.hold_basis.is_some()) {
            self.unfreeze();
        }
        if settings.analyser {
            let now = ui.input(|input| input.time);
            // A freeze holds one source and one representation (plan §7.4): a setting, the size or
            // a reset under it ends it. On the same changes the hold restarts and the display
            // starts from its new readings, so neither keeps levels of before.
            let basis = grab::Basis {
                view: settings.display(),
                generation: self.spectrum.generation(),
            };
            if self
                .frozen
                .as_ref()
                .is_some_and(|frozen| !frozen.holds(&basis, &curves.freqs))
            {
                self.unfreeze();
            }
            if self.frozen.is_none() && self.hold_basis != Some(basis) {
                self.hold.clear();
                self.hold_basis = Some(basis);
                self.spectrum.snap();
            }
            self.spectrum.analyse(&curves.freqs, now, &settings);
            let post_shown = matches!(settings.show, Show::Post | Show::Both);
            if self.frozen.is_none() {
                let source = if post_shown {
                    &self.spectrum.post_db
                } else {
                    &self.spectrum.pre_db
                };
                self.hold.push(now, source);
            }
            if std::mem::take(&mut self.freeze_asked) && self.frozen.is_none() {
                let held = self.hold.highest();
                if held.len() == curves.xs.len() {
                    let peaks = grab::peaks(&curves.freqs, &held, SPECTRUM_BOTTOM_DB);
                    let (post, pre) = if post_shown {
                        let pre = if settings.show == Show::Both {
                            self.spectrum.pre_db.clone()
                        } else {
                            Vec::new()
                        };
                        (held, pre)
                    } else {
                        (Vec::new(), held)
                    };
                    let key = if settings.side_chain {
                        self.spectrum.key_db.clone()
                    } else {
                        Vec::new()
                    };
                    self.frozen = Some(grab::Frozen {
                        basis,
                        source_post: post_shown,
                        post,
                        pre,
                        key,
                        peaks,
                        freqs: curves.freqs.clone(),
                    });
                }
            }
            let [pre_db, post_db, key_db] = self.shown_spectra();
            let points = |values: &[f32]| -> Vec<Pos2> {
                curves
                    .xs
                    .iter()
                    .zip(values)
                    .map(|(&x, &db)| pos2(x, axes.spectrum_y(db)))
                    .collect()
            };
            if matches!(settings.show, Show::Post | Show::Both)
                && post_db.len() == curves.xs.len()
                && post_db.iter().any(|db| *db > FLOOR_DB)
            {
                let line = points(post_db);
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
                && pre_db.len() == curves.xs.len()
                && pre_db.iter().any(|db| *db > FLOOR_DB)
            {
                painter.add(Painted::line(
                    points(pre_db),
                    Stroke::new(AXIS_STROKE, tokens.text_secondary.gamma_multiply(0.8)),
                ));
            }
            // The side chain's spectrum, the reference line (plan §7.4): dashed and lettered, never
            // told apart by hue; at the floor while nothing comes in.
            if settings.side_chain
                && self.telemetry.side_chain_port()
                && key_db.len() == curves.xs.len()
            {
                let line = points(key_db);
                let end = line.last().copied();
                painter.extend(Painted::dashed_line(
                    &line,
                    Stroke::new(AXIS_STROKE, tokens.text_primary.gamma_multiply(0.7)),
                    5.0,
                    3.0,
                ));
                if let Some(end) = end {
                    painter.text(
                        side_chain_label_at(axes, end),
                        egui::Align2::RIGHT_BOTTOM,
                        "SC",
                        egui::FontId::proportional(11.0),
                        tokens.text_primary,
                    );
                }
            }
            // A freeze's peaks (plan §7.4): a marker and its frequency, strongest first, a label
            // that would overlap a stronger one's not drawn; the keyboard's peak ringed.
            if let Some(frozen) = &self.frozen {
                let font = egui::FontId::proportional(11.0);
                let labels: Vec<(Pos2, String, Rect)> = frozen
                    .peaks
                    .iter()
                    .map(|peak| {
                        let at = peak_marker(axes, peak);
                        let text = format_freq(peak.freq);
                        let size = painter
                            .layout_no_wrap(text.clone(), font.clone(), tokens.text_primary)
                            .size();
                        (at, text, peak_label(axes, at, size))
                    })
                    .collect();
                let rects: Vec<Rect> = labels.iter().map(|(_, _, rect)| *rect).collect();
                let focused = self.focus_peak.filter(|_| self.selected.is_none());
                let drawn = grab::unobstructed(&rects, focused);
                self.peak_labels = drawn
                    .iter()
                    .zip(&rects)
                    .enumerate()
                    .filter(|(_, (drawn, _))| **drawn)
                    .map(|(index, (_, rect))| (index, *rect))
                    .collect();
                for (index, ((at, text, rect), drawn)) in labels.into_iter().zip(drawn).enumerate()
                {
                    painter.circle_stroke(
                        at,
                        PEAK_MARKER_RADIUS,
                        Stroke::new(EMPHASIS_STROKE, tokens.text_primary),
                    );
                    if self.focus_peak == Some(index) && self.selected.is_none() {
                        painter.circle_stroke(at, 8.0, Stroke::new(EMPHASIS_STROKE, tokens.focus));
                    }
                    if drawn {
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            text,
                            font.clone(),
                            tokens.text_primary,
                        );
                    }
                }
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
        if curves.channels.is_empty() {
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
        } else {
            // With placed bands, each channel's own line (plan §7.3), labelled with its letter at
            // its end: identity never rests on hue. Mid and Side are dashed, so the two families
            // read apart where they cross.
            let letter_font = egui::FontId::proportional(11.0);
            let moving = !curves.moving_channels.is_empty();
            for (index, channel) in curves.channels.iter().enumerate() {
                let stroke = if moving {
                    Stroke::new(1.5, tokens.accent.gamma_multiply(0.4))
                } else {
                    Stroke::new(2.0, tokens.accent)
                };
                let points = line(&channel.db);
                let mid_side = matches!(channel.letter, "M" | "S");
                let end = points.last().copied();
                if mid_side {
                    painter.extend(Painted::dashed_line(&points, stroke, 6.0, 3.0));
                } else {
                    painter.add(Painted::line(points, stroke));
                }
                if let Some(heard) = curves.moving_channels.get(index) {
                    let heard = line(&heard.db);
                    let stroke = Stroke::new(2.0, tokens.accent);
                    if mid_side {
                        painter.extend(Painted::dashed_line(&heard, stroke, 6.0, 3.0));
                    } else {
                        painter.add(Painted::line(heard, stroke));
                    }
                }
                if let Some(end) = end {
                    painter.text(
                        end + vec2(-4.0, -4.0),
                        egui::Align2::RIGHT_BOTTOM,
                        channel.letter,
                        letter_font.clone(),
                        tokens.text_primary,
                    );
                }
            }
        }

        // Find's proposals (plan §7.5): the curve they would make, dashed over the EQ's, and a
        // downward triangle at each candidate with its frequency and depth, told apart by shape
        // and text; a label that would overlap a stronger one's is not drawn, the keyboard's
        // always is.
        if self.find && !self.proposals.is_empty() {
            let current = self
                .proposal_curve
                .as_ref()
                .is_some_and(|(version, freqs, _)| {
                    *version == self.proposals_version && *freqs == curves.freqs
                });
            if !current {
                let lines = self.proposal_lines(curves);
                self.proposal_curve = Some((self.proposals_version, curves.freqs.clone(), lines));
            }
            if let Some((_, _, lines)) = &self.proposal_curve {
                for dbs in lines {
                    painter.extend(Painted::dashed_line(
                        &line(dbs),
                        Stroke::new(1.5, tokens.text_primary.gamma_multiply(0.8)),
                        4.0,
                        3.0,
                    ));
                }
            }
            let font = egui::FontId::proportional(11.0);
            let spectrum = self.proposal_spectrum();
            let marks: Vec<(Pos2, String, Rect)> = self
                .proposals
                .iter()
                .map(|proposal| {
                    let tip = proposal_marker(axes, curves, spectrum, proposal.freq);
                    let text = proposal_text(proposal);
                    let size = painter
                        .layout_no_wrap(text.clone(), font.clone(), tokens.text_primary)
                        .size();
                    let top = tip - vec2(0.0, TRIANGLE_HEIGHT);
                    (tip, text, peak_label(axes, top, size))
                })
                .collect();
            let rects: Vec<Rect> = marks.iter().map(|(_, _, rect)| *rect).collect();
            let focused = self.focus_proposal.filter(|_| self.selected.is_none());
            let drawn = grab::unobstructed(&rects, focused);
            self.proposal_labels = drawn
                .iter()
                .zip(&rects)
                .enumerate()
                .filter(|(_, (drawn, _))| **drawn)
                .map(|(index, (_, rect))| (index, *rect))
                .collect();
            for (index, ((tip, text, rect), drawn)) in marks.into_iter().zip(drawn).enumerate() {
                painter.add(Painted::convex_polygon(
                    proposal_triangle(tip).to_vec(),
                    tokens.surface_1,
                    Stroke::new(EMPHASIS_STROKE, tokens.text_primary),
                ));
                if focused == Some(index) {
                    painter.circle_stroke(
                        tip - vec2(0.0, TRIANGLE_HEIGHT / 2.0),
                        TRIANGLE_HEIGHT,
                        Stroke::new(EMPHASIS_STROKE, tokens.focus),
                    );
                }
                if drawn {
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        text,
                        font.clone(),
                        tokens.text_primary,
                    );
                }
            }
        } else {
            self.proposal_labels.clear();
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
            // A placed band in mono is not heard (plan §7.3): every part of its point is faint.
            let heard = self.placement_heard(slot);
            let faint = |colour: Color32| {
                if heard {
                    colour
                } else {
                    colour.gamma_multiply(0.35)
                }
            };
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
                let ring = Stroke::new(AXIS_STROKE, faint(tokens.accent));
                if self.listens_to_key(slot) {
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
                    Stroke::new(EMPHASIS_STROKE, faint(tokens.text_primary)),
                );
            }
            let text_color = if band.bypass || !heard {
                painter.circle_stroke(
                    at,
                    POINT_RADIUS,
                    Stroke::new(EMPHASIS_STROKE, faint(tokens.accent)),
                );
                faint(tokens.text_primary)
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
            // Its placement's letter beside the number, as on its channel's line.
            let letter = match self.placement_now(slot) {
                Placement::Stereo => None,
                Placement::Left => Some("L"),
                Placement::Right => Some("R"),
                Placement::Mid => Some("M"),
                Placement::Side => Some("S"),
            };
            if let Some(letter) = letter {
                painter.text(
                    at + vec2(POINT_RADIUS + 2.0, -POINT_RADIUS - 2.0),
                    egui::Align2::LEFT_BOTTOM,
                    letter,
                    number_font.clone(),
                    faint(tokens.text_primary),
                );
            }
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
