//! The whole EQ: up to 24 bands in series, their values' smoothing, structural transitions,
//! parking, band solo, the gain scale, the output gain and activity.
//!
//! **Values glide, structures fade** (plan §3.4):
//! - Frequency, gain and Q glide in the user's domain (log frequency, dB, log Q) with a one-pole
//!   carried as the remaining distance, and the band's sections are recomputed from them every
//!   control interval. Interpolating coefficients would bend the shape on the way.
//! - A change of shape, slope, bypass or Off is **structural**. A bell, shelf or tilt enters and
//!   leaves by ramping its gain from or to 0 dB, where it is the identity and its state warms up
//!   on the way in. Anything else crossfades the old filter into the new one. A structural change
//!   that arrives during a fade waits for it to end; the band reads its target again then, so the
//!   newest one wins and at most one fade is ever pending.
//! - **Doing nothing costs nothing.** An Off or bypassed band runs nothing once its fade is over,
//!   and so does a bell, shelf or tilt settled at exactly 0 dB, the gain scale's 0 % included.
//!   A parked band's state is cleared, and it comes back exactly as a new band does, through the
//!   entry fade from silence (plan §3.5). With every band parked, no solo and the output at
//!   exactly 0 dB, [`Engine::process_stereo`] leaves the buffer untouched: bit-exact dry.
//!
//! **A bell or shelf with Range is dynamic** (plan §7.1; `crate::dynamics`): its detector, the
//! band's region (`design_solo`) with its own state, hears the EQ's input — never the bands before
//! it — and its loudest sample each interval, across channels, is the level the law reads. The
//! ballistics move the signed moving part, the heard gain is the drawn gain plus it, held inside
//! ±30 dB, and Range glides as the gain does. A band that listens never parks; it parks again, as a
//! static band does, only when nothing it does can be heard (Range 0, or the gain scale at 0 %).

use crate::design::{
    Cascade, Design, Shape, Slope, design_running, design_solo, max_frequency, solo_hears_q,
};
use crate::dynamics::{Ballistics, Dynamics, law};
use crate::filter::Filter;
use crate::{
    FREQ_MIN, GAIN_RANGE, GAIN_SCALE_MAX, OUTPUT_GAIN_RANGE_DB, Q_MAX, Q_MIN, SLOTS,
    usable_sample_rate,
};

/// The time constant of a gliding value. A step settles in about 20 ms, inside the collection's
/// 5–20 ms smoothing window (`plugins/AGENTS.md`).
pub const SMOOTHING_SECONDS: f64 = 0.005;

/// The length of every fade: a band's entry, exit and crossfade, solo's, and the output gain's
/// ramp. `mxm-fx-curve`'s 20 ms is the precedent; the click measurements are in `AGENTS.md`.
pub const FADE_SECONDS: f64 = 0.020;

/// How often a moving band's sections are recomputed: every third of a millisecond, whatever
/// the sample rate, so the cost of a glide is the same at 44.1 and 192 kHz.
pub const CONTROL_SECONDS: f64 = 1.0 / 3_000.0;

/// The bounds on the control interval in frames.
const MIN_CONTROL_FRAMES: usize = 8;
const MAX_CONTROL_FRAMES: usize = 64;

/// With the input silent, a filter whose every state is below this is cleared, which ends its
/// tail (`plugins/AGENTS.md`: a host may sleep a plugin on real silence). A state is in signal
/// units, so −200 dB of it stays inaudible even through the largest mix a band designs.
pub const QUIET_FLOOR: f64 = 1.0e-10;

/// The output's safety bound, +24 dBFS. Valid settings can ask for far more — 24 coincident
/// +60 dB bells — which `f64` holds and `f32` would turn into infinity; the host always gets a
/// finite signal. Recovery, not character: no reasonable setting comes near it.
pub const OUTPUT_BOUND: f64 = 16.0;

/// One slot as the host and the editor set it: the plugin's parameters as plain values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// `None` is Off.
    pub shape: Option<Shape>,
    pub bypass: bool,
    pub freq: f32,
    pub gain_db: f32,
    pub q: f32,
    pub slope: Slope,
}

impl Band {
    pub const OFF: Band = Band {
        shape: None,
        bypass: false,
        freq: 1_000.0,
        gain_db: 0.0,
        q: 1.0,
        slope: Slope::Db12,
    };

    /// What the band should be built as: nothing when Off or bypassed.
    fn structure(&self) -> Option<Structure> {
        let shape = self.shape?;
        if self.bypass {
            return None;
        }
        Some(Structure {
            shape,
            // A slope only means something to a cut; elsewhere one value, so changing a hidden
            // slope is never a structural change.
            slope: if shape.has_slope() {
                self.slope
            } else {
                Slope::Db12
            },
        })
    }
}

/// What a band's filter is built for. Two bands with the same structure differ only in values,
/// which glide; a different structure fades.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Structure {
    shape: Shape,
    slope: Slope,
}

/// A value gliding toward its target, carried as the remaining distance so that it lands on the
/// target exactly (`crates/mxm-mono-01-dsp/AGENTS.md`, *A lag toward a target*).
#[derive(Clone, Copy, Debug)]
struct Glide {
    target: f64,
    distance: f64,
}

impl Glide {
    const fn at(value: f64) -> Glide {
        Glide {
            target: value,
            distance: 0.0,
        }
    }

    fn value(&self) -> f64 {
        self.target - self.distance
    }

    /// A new target, leaving the value where it is.
    fn retarget(&mut self, target: f64) {
        if target != self.target {
            self.distance += target - self.target;
            self.target = target;
        }
    }

    /// One step: `keep` of the distance is left. Whether the value moved.
    fn step(&mut self, keep: f64, epsilon: f64) -> bool {
        if self.distance == 0.0 {
            return false;
        }
        self.distance *= keep;
        if self.distance.abs() < epsilon || !self.distance.is_finite() {
            self.distance = 0.0;
        }
        true
    }

    fn snap(&mut self) {
        self.distance = 0.0;
    }

    fn is_moving(&self) -> bool {
        self.distance != 0.0
    }
}

/// A transition in progress, counted down a sample at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fade {
    None,
    /// A gain shape coming in from 0 dB.
    In,
    /// A gain shape going out to 0 dB, then dry.
    Out,
    /// `previous` fading into `current`; either may be dry.
    Cross,
}

#[derive(Clone, Copy)]
struct Running {
    structure: Structure,
    filter: Filter,
}

#[derive(Clone, Copy)]
struct BandEngine {
    target: Band,
    /// The target changed since its values were last read into the glides. Reading takes two
    /// logarithms, which an unchanged band must not pay every interval.
    changed: bool,
    log_freq: Glide,
    gain_db: Glide,
    log_q: Glide,
    /// The structure the band is at or heading to.
    destination: Option<Structure>,
    current: Option<Running>,
    previous: Option<Running>,
    fade: Fade,
    left: u32,
    /// A gain shape at exactly 0 dB: identity, cleared, not processed.
    parked: bool,
    /// Frequency or Q moved in the last control step: what a solo audition is designed from.
    audition_moved: bool,
    /// The slot's dynamics as set (plan §7.1).
    dynamics: Dynamics,
    /// Range, gliding as the gain does, in dB; 0 for a shape that cannot be dynamic.
    range_db: Glide,
    /// One interval's factors from Attack and Release.
    ballistics: Ballistics,
    /// The moving part of the gain, signed, in dB; exactly 0 at rest.
    moving_db: f64,
    /// The band's region with its own state: what the dynamics hear, from the EQ's input.
    detector: Filter,
    /// The shape the detector is designed for, while it listens.
    detector_for: Option<Shape>,
    /// The loudest detector output this interval, across channels.
    peak: f64,
    /// Whether the band listens this interval: a dynamic shape, Range or a moving part not 0, and
    /// the gain scale not 0.
    listening: bool,
}

impl BandEngine {
    const NEW: BandEngine = BandEngine {
        target: Band::OFF,
        changed: false,
        log_freq: Glide::at(6.907_755_278_982_137), // ln 1000
        gain_db: Glide::at(0.0),
        log_q: Glide::at(0.0),
        destination: None,
        current: None,
        previous: None,
        fade: Fade::None,
        left: 0,
        parked: false,
        audition_moved: false,
        dynamics: Dynamics::STATIC,
        range_db: Glide::at(0.0),
        // Set for the engine's interval in `Engine::new`.
        ballistics: Ballistics {
            attack_keep: 0.0,
            release_keep: 0.0,
        },
        moving_db: 0.0,
        detector: Filter::EMPTY,
        detector_for: None,
        peak: 0.0,
        listening: false,
    };

    /// Whether this band does anything to the audio this sample.
    fn active(&self) -> bool {
        self.fade != Fade::None || (self.current.is_some() && !self.parked)
    }

    /// The targets in the gliding domains, held inside their ranges.
    fn targets(&self, sample_rate: f64) -> (f64, f64, f64) {
        let finite_or = |value: f32, fallback: f64| {
            let value = f64::from(value);
            if value.is_finite() { value } else { fallback }
        };
        (
            finite_or(self.target.freq, 1_000.0)
                .clamp(FREQ_MIN, max_frequency(sample_rate))
                .ln(),
            finite_or(self.target.gain_db, 0.0).clamp(-GAIN_RANGE, GAIN_RANGE),
            finite_or(self.target.q, 1.0).clamp(Q_MIN, Q_MAX).ln(),
        )
    }

    fn retarget(&mut self, sample_rate: f64) {
        let (log_freq, gain_db, log_q) = self.targets(sample_rate);
        self.log_freq.retarget(log_freq);
        self.gain_db.retarget(gain_db);
        self.log_q.retarget(log_q);
    }

    fn snap(&mut self) {
        self.log_freq.snap();
        self.gain_db.snap();
        self.log_q.snap();
    }

    /// The design of `structure` at the band's present values, its gain scaled by `scale`.
    fn design_inputs(&self, structure: Structure, scale: f64) -> Design {
        Design {
            shape: structure.shape,
            slope: structure.slope,
            freq: self.log_freq.value().exp(),
            gain_db: if structure.shape.has_gain() {
                self.heard_gain_db() * scale
            } else {
                0.0
            },
            q: self.log_q.value().exp(),
        }
    }

    /// The gain heard before the gain scale: the drawn gain where it has glided to, plus the moving
    /// part, held inside the band's ±30 dB at every interval (plan §7.1, *the bound*).
    fn heard_gain_db(&self) -> f64 {
        (self.gain_db.value() + self.moving_db).clamp(-GAIN_RANGE, GAIN_RANGE)
    }

    /// The detector for the band's region at its present frequency and Q.
    fn detector_design(&self, structure: Structure, sample_rate: f64) -> Option<Cascade> {
        design_solo(self.design_inputs(structure, 1.0), sample_rate)
    }

    /// The gain envelope of an entry or exit fade after `frames` more samples.
    fn envelope(&self, frames: usize, fade: u32) -> f64 {
        let left_after = f64::from(self.left.saturating_sub(frames as u32));
        match self.fade {
            Fade::In => 1.0 - left_after / f64::from(fade),
            Fade::Out => left_after / f64::from(fade),
            Fade::None | Fade::Cross => 1.0,
        }
    }

    /// Starts the move to `wanted`. Only called with no fade in progress.
    fn start(&mut self, wanted: Option<Structure>, gain_scale: f64, sample_rate: f64, fade: u32) {
        let old = self.current.take();
        let old_parked = self.parked;
        self.parked = false;
        self.destination = wanted;
        if old.is_none() {
            // Out of silence the band starts where it is drawn, not gliding from the values the
            // slot last held.
            self.snap();
        }
        let fresh = |band: &BandEngine, structure: Structure, scale: f64| Running {
            structure,
            filter: Filter::fresh(&design_running(
                band.design_inputs(structure, scale),
                sample_rate,
            )),
        };
        match (old, wanted) {
            (None, None) => {}
            (None, Some(new)) if new.shape.has_gain() => {
                // Built at 0 dB, the identity; the envelope brings the gain in.
                self.current = Some(fresh(self, new, 0.0));
                self.fade = Fade::In;
                self.left = fade;
            }
            (Some(_), None) if old_parked => {
                // Already the identity and silent: nothing to fade.
            }
            (Some(old), None) if old.structure.shape.has_gain() => {
                self.current = Some(old);
                self.fade = Fade::Out;
                self.left = fade;
            }
            (old, new) => {
                self.previous = if old_parked { None } else { old };
                self.current = new.map(|new| fresh(self, new, gain_scale));
                self.fade = Fade::Cross;
                self.left = fade;
            }
        }
    }

    /// Ends whatever fade has run out.
    fn finish(&mut self) {
        match self.fade {
            Fade::Out => self.current = None,
            Fade::Cross => self.previous = None,
            Fade::In | Fade::None => {}
        }
        self.fade = Fade::None;
    }

    #[inline]
    fn tick<const C: usize>(&mut self, input: [f64; C], fade: u32) -> [f64; C] {
        let output = match self.fade {
            Fade::None => {
                return match &mut self.current {
                    Some(running) if !self.parked => running.filter.process(input),
                    _ => input,
                };
            }
            Fade::In | Fade::Out => match &mut self.current {
                Some(running) => running.filter.process(input),
                None => input,
            },
            Fade::Cross => {
                let old = match &mut self.previous {
                    Some(running) => running.filter.process(input),
                    None => input,
                };
                let new = match &mut self.current {
                    Some(running) => running.filter.process(input),
                    None => input,
                };
                let t = 1.0 - f64::from(self.left) / f64::from(fade);
                let mut mixed = old;
                for (out, new) in mixed.iter_mut().zip(new) {
                    *out += (new - *out) * t;
                }
                mixed
            }
        };
        self.left = self.left.saturating_sub(1);
        if self.left == 0 {
            self.finish();
        }
        output
    }

    fn largest_state(&self) -> f64 {
        let state =
            |running: &Option<Running>| running.as_ref().map_or(0.0, |r| r.filter.largest_state());
        state(&self.current)
            .max(state(&self.previous))
            .max(self.detector.largest_state())
    }

    fn clear(&mut self) {
        for running in [&mut self.current, &mut self.previous]
            .into_iter()
            .flatten()
        {
            running.filter.clear();
        }
        self.detector.clear();
    }

    /// No moving part and no detector: what a band that stops listening keeps.
    fn stop_listening(&mut self) {
        self.listening = false;
        self.moving_db = 0.0;
        self.peak = 0.0;
        self.detector = Filter::EMPTY;
        self.detector_for = None;
    }
}

/// Band solo: an audition filter on the dry input, crossfaded over the EQ.
#[derive(Clone, Copy)]
struct Solo {
    /// The band the editor asked for.
    wanted: Option<usize>,
    /// The band the audition is built for, and that band's structure when it was built. Another
    /// structure is another audition: it fades out, is rebuilt from silence and fades in.
    slot: Option<usize>,
    structure: Option<Structure>,
    filter: Filter,
    /// 0 is the EQ, 1 the audition.
    mix: f64,
    /// Whether the audition built is the one wanted, so the mix heads to 1; decided each control
    /// interval.
    heard: bool,
}

impl Solo {
    const OFF: Solo = Solo {
        wanted: None,
        slot: None,
        structure: None,
        filter: Filter::EMPTY,
        mix: 0.0,
        heard: false,
    };
}

/// The whole EQ.
pub struct Engine {
    sample_rate: f64,
    fade: u32,
    interval: usize,
    /// One interval's glide factor, `e^{−interval/(τ·fs)}`.
    keep: f64,
    bands: [BandEngine; SLOTS],
    gain_scale: Glide,
    output: f64,
    output_target: f64,
    output_step: f64,
    output_left: u32,
    solo: Solo,
    /// Samples left in the control interval in progress. The interval runs on across calls, so
    /// the control schedule, and with it every sample, is the same however a host splits its
    /// blocks.
    countdown: usize,
    /// Whether the interval in progress is dry, decided at its start.
    dry: bool,
    /// Whether the interval in progress has heard a nonzero input sample.
    heard_input: bool,
}

impl Engine {
    /// An engine for `sample_rate`, every slot Off. A rate below [`crate::MIN_SAMPLE_RATE`] or not
    /// finite is built as that minimum ([`usable_sample_rate`]): the plugin refuses to activate
    /// there, and the engine stays finite.
    pub fn new(sample_rate: f64) -> Engine {
        let sample_rate = usable_sample_rate(sample_rate);
        let interval = ((sample_rate * CONTROL_SECONDS).round() as usize)
            .clamp(MIN_CONTROL_FRAMES, MAX_CONTROL_FRAMES);
        let mut bands = [BandEngine::NEW; SLOTS];
        for band in &mut bands {
            band.ballistics = Ballistics::new(Dynamics::STATIC, interval as f64 / sample_rate);
        }
        Engine {
            sample_rate,
            fade: ((sample_rate * FADE_SECONDS).round() as u32).max(1),
            interval,
            keep: (-(interval as f64) / (SMOOTHING_SECONDS * sample_rate)).exp(),
            bands,
            gain_scale: Glide::at(1.0),
            output: 1.0,
            output_target: 1.0,
            output_step: 0.0,
            output_left: 0,
            solo: Solo::OFF,
            countdown: 0,
            dry: true,
            heard_input: false,
        }
    }

    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// The control interval in frames: settings are read, and moving bands redesigned, once per
    /// interval (`CONTROL_SECONDS`).
    pub fn control_interval(&self) -> usize {
        self.interval
    }

    /// A fade's length in frames (`FADE_SECONDS`).
    pub fn fade_frames(&self) -> usize {
        self.fade as usize
    }

    /// A slot's settings, read at the next control interval's start: values glide toward them, a
    /// structural change starts its fade or waits for the one in progress. An out-of-range slot
    /// is ignored.
    pub fn set_band(&mut self, slot: usize, band: Band) {
        if let Some(engine) = self.bands.get_mut(slot) {
            // Bitwise, so a NaN setting compares unequal to nothing and is read once.
            let same = engine.target.shape == band.shape
                && engine.target.bypass == band.bypass
                && engine.target.slope == band.slope
                && engine.target.freq.to_bits() == band.freq.to_bits()
                && engine.target.gain_db.to_bits() == band.gain_db.to_bits()
                && engine.target.q.to_bits() == band.q.to_bits();
            if !same {
                engine.target = band;
                engine.changed = true;
            }
        }
    }

    pub fn band(&self, slot: usize) -> Option<Band> {
        self.bands.get(slot).map(|engine| engine.target)
    }

    /// The output gain as a linear factor, held inside ±24 dB. It ramps over a fade.
    /// A slot's dynamics (plan §7.1), held inside their ranges and read at the next control
    /// interval: Range glides there; Attack and Release take effect at once. An out-of-range slot
    /// is ignored.
    pub fn set_dynamics(&mut self, slot: usize, dynamics: Dynamics) {
        let interval_seconds = self.interval as f64 / self.sample_rate;
        if let Some(band) = self.bands.get_mut(slot) {
            let dynamics = dynamics.bounded();
            if band.dynamics != dynamics {
                band.dynamics = dynamics;
                band.ballistics = Ballistics::new(dynamics, interval_seconds);
            }
        }
    }

    /// A slot's dynamics as last set, held inside their ranges.
    pub fn dynamics(&self, slot: usize) -> Option<Dynamics> {
        self.bands.get(slot).map(|band| band.dynamics)
    }

    /// A slot's gains for the editor, before the gain scale (plan §7.1, telemetry): the drawn gain
    /// where it has glided to, and the gain heard, the drawn gain plus the moving part held inside
    /// ±30 dB. Their difference is the action heard. 0 dB for a shape without a gain.
    pub fn gains_db(&self, slot: usize) -> Option<(f64, f64)> {
        let band = self.bands.get(slot)?;
        let gain_shape = band
            .current
            .as_ref()
            .is_some_and(|running| running.structure.shape.has_gain());
        Some(if gain_shape {
            (band.gain_db.value(), band.heard_gain_db())
        } else {
            (0.0, 0.0)
        })
    }

    pub fn set_output_gain(&mut self, gain: f32) {
        let limit = 10f64.powf(OUTPUT_GAIN_RANGE_DB / 20.0);
        let gain = if gain.is_finite() {
            f64::from(gain).clamp(1.0 / limit, limit)
        } else {
            1.0
        };
        if gain != self.output_target {
            self.output_target = gain;
            self.output_left = self.fade;
            self.output_step = (gain - self.output) / f64::from(self.fade);
            // The ramp starts at this sample, so the interval in progress can no longer be left
            // untouched.
            self.dry = false;
        }
    }

    /// The gain scale as a factor, 0 to 2. It glides.
    pub fn set_gain_scale(&mut self, scale: f32) {
        let scale = if scale.is_finite() {
            f64::from(scale).clamp(0.0, GAIN_SCALE_MAX)
        } else {
            1.0
        };
        self.gain_scale.retarget(scale);
    }

    /// The band to audition, or none. Honoured only while that band is live and offers solo.
    pub fn set_solo(&mut self, slot: Option<usize>) {
        self.solo.wanted = slot.filter(|&slot| slot < SLOTS);
    }

    /// The band whose audition is heard, if any is.
    pub fn audition(&self) -> Option<usize> {
        self.solo.slot.filter(|_| self.solo.mix > 0.0)
    }

    /// Every transition finished at once and every state cleared: each band built as its settings
    /// say, values at their targets, the gain scale and output gain at theirs, solo ended. For
    /// activation and a host's reset, so a saved EQ is heard from its first sample.
    pub fn reset(&mut self) {
        self.gain_scale.snap();
        let scale = self.gain_scale.value();
        let sample_rate = self.sample_rate;
        for band in &mut self.bands {
            band.retarget(sample_rate);
            band.changed = false;
            band.snap();
            let wanted = band.target.structure();
            band.destination = wanted;
            band.previous = None;
            band.fade = Fade::None;
            band.left = 0;
            band.parked = false;
            band.current = wanted.map(|structure| Running {
                structure,
                filter: Filter::fresh(&design_running(
                    band.design_inputs(structure, scale),
                    sample_rate,
                )),
            });
            band.stop_listening();
            let range = band.dynamics.range_db;
            let dynamic = wanted.is_some_and(|structure| structure.shape.is_dynamic());
            band.range_db = Glide::at(if dynamic { f64::from(range) } else { 0.0 });
            // A band with Range listens from the first sample, so it never parks here.
            if dynamic && range != 0.0 && scale != 0.0 {
                band.listening = true;
            }
            band.parked = !band.listening
                && wanted.is_some_and(|structure| {
                    structure.shape.has_gain()
                        && band.design_inputs(structure, scale).gain_db == 0.0
                });
        }
        self.solo = Solo::OFF;
        self.output = self.output_target;
        self.output_left = 0;
        self.countdown = 0;
        self.heard_input = false;
    }

    /// Nothing to do: every band parked or dry with no fade, no audition, the output at exactly
    /// 0 dB. The buffer is then left untouched.
    pub fn is_dry(&self) -> bool {
        self.bands.iter().all(|band| !band.active())
            && self.solo.slot.is_none()
            && self.solo.mix == 0.0
            && self.output_left == 0
            && self.output == 1.0
    }

    /// Whether any filter holds state, so silence in is not yet silence out.
    pub fn is_ringing(&self) -> bool {
        self.bands
            .iter()
            .any(|band| band.largest_state() > 0.0 || band.moving_db != 0.0)
            || self.solo.filter.largest_state() > 0.0
    }

    /// Nothing audible is moving or waiting to: no setting unread, no structural change pending
    /// or fading, no audible value gliding, no section ramping, the gain scale still where any gain
    /// shape hears it, solo fully in or out of the band it should be on, and the output gain at
    /// its target. A settled engine runs every section on its precomputed coefficients. A value
    /// its shape ignores may still glide.
    pub fn is_settled(&self) -> bool {
        let still = |running: &Option<Running>| {
            running
                .as_ref()
                .is_none_or(|running| !running.filter.is_ramping())
        };
        let band_settled = |band: &BandEngine| {
            let gliding = band.current.as_ref().is_some_and(|running| {
                let Structure { shape, slope } = running.structure;
                band.log_freq.is_moving()
                    || (shape.has_gain() && band.gain_db.is_moving())
                    || (shape.has_q(slope) && band.log_q.is_moving())
            });
            !band.changed
                && band.target.structure() == band.destination
                && band.fade == Fade::None
                && !gliding
                && band.moving_db == 0.0
                && !band.range_db.is_moving()
                && still(&band.current)
                && still(&band.previous)
        };
        // The scale is heard only through a gain shape with a gain to scale.
        let scale_heard = self.bands.iter().any(|band| {
            band.destination
                .is_some_and(|structure| structure.shape.has_gain())
                && (band.gain_db.value() != 0.0
                    || band.gain_db.target != 0.0
                    || band.moving_db != 0.0)
        });
        let solo = &self.solo;
        let solo_mix_target = if solo.heard { 1.0 } else { 0.0 };
        self.bands.iter().all(band_settled)
            && !(scale_heard && self.gain_scale.is_moving())
            && solo.slot == self.solo_wanted()
            && solo.mix == solo_mix_target
            && !solo.filter.is_ramping()
            && self.output_left == 0
    }

    /// The band whose audition should be built: the one asked for, if it is live and offers solo.
    fn solo_wanted(&self) -> Option<usize> {
        self.solo.wanted.filter(|&slot| {
            self.bands[slot]
                .destination
                .is_some_and(|structure| structure.shape.offers_solo())
        })
    }

    /// Processes a stereo block in place, both channels identically.
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        self.run::<2>([&mut left[..frames], &mut right[..frames]]);
    }

    /// Processes a mono block in place.
    pub fn process_mono(&mut self, buffer: &mut [f32]) {
        self.run::<1>([buffer]);
    }

    /// Runs the block, starting a control interval wherever the one in progress ends.
    fn run<const C: usize>(&mut self, mut channels: [&mut [f32]; C]) {
        let frames = channels[0].len();
        let mut start = 0;
        while start < frames {
            if self.countdown == 0 {
                self.control();
                self.countdown = self.interval;
                self.heard_input = false;
                self.dry = self.is_dry();
            }
            let end = (start + self.countdown).min(frames);
            if !self.dry {
                self.render::<C>(&mut channels, start, end);
            }
            self.countdown -= end - start;
            start = end;
            if self.countdown == 0
                && !self.dry
                && !self.heard_input
                && self.largest_state() < QUIET_FLOOR
            {
                // A whole interval of silence in and nothing left worth hearing: the tail is over.
                self.clear_states();
            }
        }
    }

    /// The control half of an interval: structures, glides, parking, coefficients, solo.
    fn control(&mut self) {
        let frames = self.interval;
        let keep = self.keep;
        let scale_moved = self.gain_scale.step(keep, 1.0e-7);
        let scale = self.gain_scale.value();
        let (sample_rate, fade) = (self.sample_rate, self.fade);

        for band in &mut self.bands {
            if band.changed {
                band.retarget(sample_rate);
                band.changed = false;
            }
            let wanted = band.target.structure();
            if wanted != band.destination && band.fade == Fade::None {
                band.start(wanted, scale, sample_rate, fade);
            }
            let Some(running) = band.current else {
                // Dry: nothing is heard, so the values wait at their targets.
                band.snap();
                band.audition_moved = false;
                band.stop_listening();
                continue;
            };
            let freq_moved = band.log_freq.step(keep, 1.0e-6);
            let gain_moved = band.gain_db.step(keep, 1.0e-5);
            let q_moved = band.log_q.step(keep, 1.0e-6);
            let structure = running.structure;
            band.audition_moved = freq_moved || (q_moved && solo_hears_q(structure.shape));
            let gain_shape = structure.shape.has_gain();

            // Dynamics (plan §7.1): Range glides, the interval's level meets the law, and the
            // ballistics move the moving part toward what it asks.
            let dynamic_shape = structure.shape.is_dynamic();
            band.range_db.retarget(if dynamic_shape {
                f64::from(band.dynamics.range_db)
            } else {
                0.0
            });
            band.range_db.step(keep, 1.0e-5);
            let moving_before = band.moving_db;
            if dynamic_shape
                && scale != 0.0
                && (band.range_db.value() != 0.0 || band.moving_db != 0.0)
            {
                if !band.listening || band.detector_for != Some(structure.shape) {
                    band.detector = band
                        .detector_design(structure, sample_rate)
                        .map_or(Filter::EMPTY, |cascade| Filter::fresh(&cascade));
                    band.detector_for = Some(structure.shape);
                    band.peak = 0.0;
                } else if band.audition_moved {
                    if let Some(cascade) = band.detector_design(structure, sample_rate) {
                        band.detector.ramp_to(&cascade, frames);
                    }
                }
                let level_db = if band.peak > 0.0 {
                    20.0 * band.peak.log10()
                } else {
                    f64::NEG_INFINITY
                };
                let target = law(
                    level_db,
                    f64::from(band.dynamics.threshold_db),
                    band.range_db.value(),
                );
                band.moving_db = band.ballistics.step(band.moving_db, target);
                band.peak = 0.0;
                band.listening = true;
            } else if band.listening || band.moving_db != 0.0 {
                // Nothing it does can be heard any more: no moving part, no detector.
                band.stop_listening();
            }
            let dynamics_moved = band.moving_db != moving_before;

            // Only what the shape hears: a cut's stored gain or a tilt's Q may glide unheeded.
            let moved = freq_moved
                || ((gain_moved || dynamics_moved) && gain_shape)
                || (q_moved && structure.shape.has_q(structure.slope));
            let fading_gain = matches!(band.fade, Fade::In | Fade::Out);
            // A fade's last interval may hold fewer of its samples than the interval: the ramp
            // ends with the fade, so the band is never dropped with its coefficients on the way.
            let ramp = if fading_gain {
                frames.min(band.left as usize)
            } else {
                frames
            };
            let envelope = band.envelope(frames, fade);
            let inputs = {
                let mut inputs = band.design_inputs(structure, scale);
                inputs.gain_db *= envelope;
                inputs
            };

            // Parking: a gain shape settled at exactly 0 dB is the identity.
            if band.fade == Fade::None && gain_shape {
                let identity = inputs.gain_db == 0.0 && !band.listening;
                if identity && !band.parked {
                    band.parked = true;
                    if let Some(running) = &mut band.current {
                        running.filter.clear();
                    }
                    continue;
                }
                if band.parked {
                    if identity {
                        continue;
                    }
                    // Leaving it is creation again: from silence, through the entry fade.
                    band.parked = false;
                    band.fade = Fade::In;
                    band.left = fade;
                    let mut entering = inputs;
                    entering.gain_db *= band.envelope(frames, fade);
                    if let Some(running) = &mut band.current {
                        running.filter.ramp_to(
                            &design_running(entering, sample_rate),
                            frames.min(fade as usize),
                        );
                    }
                    continue;
                }
            }
            if moved || fading_gain || (scale_moved && gain_shape) {
                if let Some(running) = &mut band.current {
                    running
                        .filter
                        .ramp_to(&design_running(inputs, sample_rate), ramp);
                }
            }
        }

        self.control_solo(scale, frames);
    }

    fn control_solo(&mut self, scale: f64, frames: usize) {
        let valid = self.solo_wanted();
        let bands = &self.bands;
        let solo = &mut self.solo;
        let current = solo.slot.is_some()
            && solo.slot == valid
            && solo
                .slot
                .is_some_and(|slot| bands[slot].destination == solo.structure);
        if !current && solo.mix <= 0.0 && (solo.slot.is_some() || valid.is_some()) {
            // Faded out: build the audition now wanted, from silence.
            solo.slot = None;
            solo.structure = None;
            solo.filter = Filter::EMPTY;
            if let Some(slot) = valid {
                let band = &bands[slot];
                if let Some(structure) = band.destination {
                    if let Some(cascade) =
                        design_solo(band.design_inputs(structure, scale), self.sample_rate)
                    {
                        solo.slot = Some(slot);
                        solo.structure = Some(structure);
                        solo.filter = Filter::fresh(&cascade);
                    }
                }
            }
        } else if current {
            // While soloed the band stays editable, and the audition follows its frequency and Q
            // when they move; no solo design hears the gain.
            if let (Some(slot), Some(structure)) = (solo.slot, solo.structure) {
                if bands[slot].audition_moved {
                    let cascade: Option<Cascade> = design_solo(
                        bands[slot].design_inputs(structure, scale),
                        self.sample_rate,
                    );
                    if let Some(cascade) = cascade {
                        solo.filter.ramp_to(&cascade, frames);
                    }
                }
            }
        }
        // Decided after any rebuild, so an audition built this interval starts fading in now.
        solo.heard = solo.slot.is_some()
            && solo.slot == valid
            && solo
                .slot
                .is_some_and(|slot| bands[slot].destination == solo.structure);
    }

    /// Frames `start..end` of the interval in progress, in place.
    fn render<const C: usize>(&mut self, channels: &mut [&mut [f32]; C], start: usize, end: usize) {
        let solo_target = if self.solo.heard { 1.0 } else { 0.0 };
        let solo_step = 1.0 / f64::from(self.fade);
        let fade = self.fade;
        let mut heard_input = false;
        for index in start..end {
            let mut dry = [0.0f64; C];
            for (value, channel) in dry.iter_mut().zip(channels.iter()) {
                let sample = channel[index];
                *value = if sample.is_finite() {
                    f64::from(sample)
                } else {
                    0.0
                };
                heard_input |= *value != 0.0;
            }

            let mut wet = dry;
            for band in &mut self.bands {
                if band.listening {
                    for value in band.detector.process(dry) {
                        band.peak = band.peak.max(value.abs());
                    }
                }
                if band.active() {
                    wet = band.tick(wet, fade);
                }
            }

            let solo = &mut self.solo;
            if solo.mix != solo_target {
                solo.mix = if solo.mix < solo_target {
                    (solo.mix + solo_step).min(solo_target)
                } else {
                    (solo.mix - solo_step).max(solo_target)
                };
            }
            if solo.mix > 0.0 && solo.slot.is_some() {
                let audition = solo.filter.process(dry);
                for (out, audition) in wet.iter_mut().zip(audition) {
                    *out += (audition - *out) * solo.mix;
                }
            }

            if self.output_left > 0 {
                self.output += self.output_step;
                self.output_left -= 1;
                if self.output_left == 0 {
                    self.output = self.output_target;
                }
            }
            let mut blown = false;
            for (out, channel) in wet.iter().zip(channels.iter_mut()) {
                let mut sample = out * self.output;
                if !sample.is_finite() {
                    sample = 0.0;
                    blown = true;
                }
                // A value normal in `f64` can be subnormal in `f32`; a host is never handed one.
                let sample = sample.clamp(-OUTPUT_BOUND, OUTPUT_BOUND) as f32;
                channel[index] = if sample.abs() < f32::MIN_POSITIVE {
                    0.0
                } else {
                    sample
                };
            }
            if blown {
                // A state that overflowed never recovers on its own: every filter starts again
                // from silence, at this sample.
                self.clear_states();
            }
        }
        self.heard_input |= heard_input;
    }

    fn largest_state(&self) -> f64 {
        self.bands
            .iter()
            .map(BandEngine::largest_state)
            .fold(self.solo.filter.largest_state(), f64::max)
    }

    fn clear_states(&mut self) {
        for band in &mut self.bands {
            band.clear();
        }
        self.solo.filter.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    fn bell(freq: f32, gain_db: f32, q: f32) -> Band {
        Band {
            shape: Some(Shape::Bell),
            bypass: false,
            freq,
            gain_db,
            q,
            slope: Slope::Db12,
        }
    }

    #[test]
    fn values_glide_at_one_speed_whatever_the_hosts_buffers() {
        let glided = |sizes: &[usize]| {
            let mut engine = Engine::new(RATE);
            engine.set_band(0, bell(1_000.0, 0.0, 1.0));
            engine.reset();
            engine.set_band(0, bell(1_000.0, 12.0, 1.0));
            // 480 frames, 10 ms: two time constants in, nowhere near the end of the glide.
            let mut left = 480;
            for &size in sizes.iter().cycle() {
                if left == 0 {
                    break;
                }
                let size = size.min(left);
                let mut buffer = vec![0.0f32; size];
                engine.process_mono(&mut buffer);
                left -= size;
            }
            engine.bands[0].gain_db.value()
        };
        let whole = glided(&[480]);
        assert!(whole > 6.0 && whole < 11.5, "{whole} dB of 12 after 10 ms");
        for sizes in [&[1][..], &[7][..], &[3, 16, 5, 1, 11][..], &[64][..]] {
            let other = glided(sizes);
            assert!(
                (other - whole).abs() < 1.0e-9,
                "{sizes:?}: {other}, whole blocks {whole}"
            );
        }
    }

    #[test]
    fn a_glide_lands_exactly_on_its_target() {
        let mut engine = Engine::new(RATE);
        engine.set_band(0, bell(100.0, -3.0, 0.5));
        engine.reset();
        engine.set_band(0, bell(7_123.0, 17.25, 9.0));
        let mut buffer = vec![0.0f32; 48_000];
        engine.process_mono(&mut buffer);
        let band = &engine.bands[0];
        assert_eq!(band.gain_db.value(), 17.25);
        assert_eq!(band.log_freq.value(), 7_123f64.ln());
        assert_eq!(band.log_q.value(), 9f64.ln());
    }

    /// Band and solo designs this thread has made.
    fn designs() -> usize {
        crate::design::DESIGNS.with(std::cell::Cell::get) + solo_designs()
    }

    fn solo_designs() -> usize {
        crate::design::SOLO_DESIGNS.with(std::cell::Cell::get)
    }

    /// Processes `seconds` of a quiet tone, calling `each` before every 64-frame block.
    fn play(engine: &mut Engine, seconds: f64, mut each: impl FnMut(&mut Engine, usize)) {
        let blocks = (seconds * RATE / 64.0) as usize;
        for block in 0..blocks {
            each(engine, block);
            let mut left = [0.0f32; 64];
            let mut right = [0.0f32; 64];
            for (i, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
                *l = (0.01 * ((block * 64 + i) as f64 * 0.05).sin()) as f32;
                *r = -*l;
            }
            engine.process_stereo(&mut left, &mut right);
        }
    }

    #[test]
    fn automation_a_shape_ignores_designs_nothing_once_settled() {
        let mut engine = Engine::new(RATE);
        let shaped = |slot: usize, block: usize| {
            let wobble = (block % 2) as f32;
            match slot % 3 {
                // A cut has no gain, a tilt and a 6 dB/octave cut no Q.
                0 => Band {
                    shape: Some(Shape::HighCut),
                    bypass: false,
                    freq: 2_000.0 + 100.0 * slot as f32,
                    gain_db: 20.0 * wobble - 10.0,
                    q: 0.707,
                    slope: Slope::Db96,
                },
                1 => Band {
                    shape: Some(Shape::TiltShelf),
                    bypass: false,
                    freq: 500.0 + 50.0 * slot as f32,
                    gain_db: 3.0,
                    q: 0.5 + 9.0 * wobble,
                    slope: Slope::Db12,
                },
                _ => Band {
                    shape: Some(Shape::LowCut),
                    bypass: false,
                    freq: 40.0 + slot as f32,
                    gain_db: -7.0 + 14.0 * wobble,
                    q: 0.3 + 5.0 * wobble,
                    slope: Slope::Db6,
                },
            }
        };
        for slot in 0..SLOTS {
            engine.set_band(slot, shaped(slot, 0));
        }
        engine.reset();
        play(&mut engine, 0.1, |engine, block| {
            for slot in 0..SLOTS {
                engine.set_band(slot, shaped(slot, block));
            }
        });
        let before = designs();
        play(&mut engine, 0.5, |engine, block| {
            for slot in 0..SLOTS {
                engine.set_band(slot, shaped(slot, block));
            }
        });
        assert_eq!(designs() - before, 0, "ignored lanes redesigned a band");
        assert!(engine.is_settled());
    }

    #[test]
    fn a_static_solo_designs_nothing_once_it_is_in() {
        let mut engine = Engine::new(RATE);
        engine.set_band(
            7,
            Band {
                shape: Some(Shape::LowCut),
                bypass: false,
                freq: 3_000.0,
                gain_db: 0.0,
                q: 2.0,
                slope: Slope::Db96,
            },
        );
        engine.reset();
        engine.set_solo(Some(7));
        play(&mut engine, 0.1, |_, _| {});
        assert_eq!(engine.audition(), Some(7));
        let before = designs();
        play(&mut engine, 0.5, |_, _| {});
        assert_eq!(designs() - before, 0, "a static audition was redesigned");
        assert!(engine.is_settled());
        // Moving the band's frequency still moves the audition.
        let mut moved = engine.band(7).unwrap();
        moved.freq = 4_000.0;
        engine.set_band(7, moved);
        play(&mut engine, 0.01, |_, _| {});
        assert!(designs() > before);
    }

    #[test]
    fn automating_q_an_audition_ignores_designs_nothing() {
        for (shape, slope) in [
            (Shape::LowCut, Slope::Db6),
            (Shape::HighCut, Slope::Db48),
            (Shape::LowShelf, Slope::Db12),
        ] {
            let mut engine = Engine::new(RATE);
            let at = |q: f32| Band {
                shape: Some(shape),
                bypass: false,
                freq: 900.0,
                gain_db: 6.0,
                q,
                slope,
            };
            engine.set_band(0, at(0.7));
            engine.reset();
            engine.set_solo(Some(0));
            play(&mut engine, 0.1, |_, _| {});
            assert_eq!(engine.audition(), Some(0));
            let before = solo_designs();
            play(&mut engine, 0.3, |engine, block| {
                engine.set_band(0, at(if block % 2 == 0 { 0.7 } else { 0.71 }));
            });
            // The band itself may hear its Q; its audition, at Q 0.707, must not be redesigned.
            assert_eq!(solo_designs() - before, 0, "{shape:?} {slope:?}");
        }
    }

    #[test]
    fn settled_means_nothing_audible_moves_at_any_control_boundary() {
        let mut engine = Engine::new(RATE);
        engine.set_band(
            0,
            Band {
                shape: Some(Shape::Bell),
                bypass: false,
                freq: 1_000.0,
                gain_db: 0.5,
                q: 1.0,
                slope: Slope::Db12,
            },
        );
        engine.reset();
        assert!(engine.is_settled());
        let mut bell = engine.band(0).unwrap();
        bell.gain_db = 12.0;
        engine.set_band(0, bell);
        assert!(!engine.is_settled(), "a setting not yet read");
        let interval = engine.control_interval();
        let mut buffer = vec![0.001f32; interval];
        let mut boundaries = 0;
        while !engine.is_settled() {
            // Exactly one interval per call: the status at every boundary.
            engine.process_mono(&mut buffer);
            boundaries += 1;
            assert!(boundaries < 2_000, "never settles");
        }
        assert_eq!(
            engine.bands[0].gain_db.value(),
            12.0,
            "settled only once the glide landed"
        );
        // And it stays so with nothing changing.
        for _ in 0..50 {
            engine.process_mono(&mut buffer);
            assert!(engine.is_settled());
        }
        // A gain scale move is heard by the bell.
        engine.set_gain_scale(0.5);
        assert!(!engine.is_settled());

        // But not by a parked 0 dB bell: nothing moves, so nothing is unsettled.
        let mut identity = Engine::new(RATE);
        identity.set_band(
            0,
            Band {
                gain_db: 0.0,
                ..bell
            },
        );
        identity.reset();
        identity.set_gain_scale(2.0);
        assert!(identity.is_dry() && identity.is_settled());
    }

    #[test]
    fn a_solo_request_fades_in_from_the_interval_it_is_built_in() {
        let mut engine = Engine::new(RATE);
        engine.set_band(
            3,
            Band {
                shape: Some(Shape::Bell),
                bypass: false,
                freq: 800.0,
                gain_db: 4.0,
                q: 3.0,
                slope: Slope::Db12,
            },
        );
        engine.reset();
        engine.set_solo(Some(3));
        let interval = engine.control_interval();
        let mut buffer = vec![0.01f32; interval];
        engine.process_mono(&mut buffer);
        assert_eq!(
            engine.audition(),
            Some(3),
            "fading in after its first interval"
        );
        assert!(!engine.is_settled());
        let mut boundaries = 1;
        while !engine.is_settled() {
            engine.process_mono(&mut buffer);
            boundaries += 1;
            assert!(boundaries < 1_000);
        }
        // In exactly one fade, rounded up to whole intervals.
        assert_eq!(boundaries, engine.fade_frames().div_ceil(interval));
        assert_eq!(engine.solo.mix, 1.0);
    }
}
