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
//!
//! **Linear phase** (plan §7.7; `crate::linear`) is the other way through: the bands' settings at
//! their targets make a zero-phase kernel, designed off the audio thread and faded in on its update
//! boundaries; values glide and structures fade through those crossfades, dynamic bands are held at
//! their drawn gain, solo plays on the input delayed by the latency, and the output gain ramps as
//! here.

use crate::design::{
    Cascade, Design, Shape, Slope, design, design_running, design_solo, max_frequency, solo_hears_q,
};
use crate::dynamics::{Ballistics, Dynamics, law};
use crate::filter::Filter;
use crate::linear::{Heard, Linear, Phase};
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

/// Where a band works in the stereo image (plan §7.3): both channels, one of them, or the mid or
/// the side of the pair, `M = (L + R)/2` and `S = (L − R)/2`. In mono only Stereo is heard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Placement {
    #[default]
    Stereo,
    Left,
    Right,
    Mid,
    Side,
}

impl Placement {
    pub const ALL: [Placement; 5] = [
        Placement::Stereo,
        Placement::Left,
        Placement::Right,
        Placement::Mid,
        Placement::Side,
    ];

    /// On the mid or the side: run after every Stereo, Left and Right band (decision 22).
    pub fn is_mid_side(self) -> bool {
        matches!(self, Placement::Mid | Placement::Side)
    }
}

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
            placement: Placement::Stereo,
        })
    }
}

/// One arm on the channels its placement works on, in a stage whose signal is `[L, R]` or
/// `[M, S]`: a Stereo arm on both, a placed arm on its one. In mono a placed arm does nothing.
#[inline]
fn run_arm<const C: usize>(running: &mut Running, input: [f64; C]) -> [f64; C] {
    let channel = match running.structure.placement {
        Placement::Stereo => return running.filter.process(input),
        Placement::Left | Placement::Mid => 0,
        Placement::Right | Placement::Side => 1,
    };
    if C < 2 {
        return input;
    }
    let mut output = input;
    output[channel] = running.filter.process([input[channel]])[0];
    output
}

/// A solo audition of `placement` on `source`: the band's channel where it sits in the image — a
/// Left band's region in the left, a Mid band's in the centre, a Side band's as side (plan §7.3).
#[inline]
fn audition<const C: usize>(
    filter: &mut Filter,
    placement: Placement,
    source: [f64; C],
) -> [f64; C] {
    if placement == Placement::Stereo {
        return filter.process(source);
    }
    let mut output = [0.0; C];
    if C < 2 {
        return output;
    }
    match placement {
        Placement::Left => output[0] = filter.process([source[0]])[0],
        Placement::Right => output[1] = filter.process([source[1]])[0],
        Placement::Mid => {
            let mid = filter.process([(source[0] + source[1]) * 0.5])[0];
            output[0] = mid;
            output[1] = mid;
        }
        Placement::Side | Placement::Stereo => {
            let side = filter.process([(source[0] - source[1]) * 0.5])[0];
            output[0] = side;
            output[1] = -side;
        }
    }
    output
}

/// What a band's filter is built for. Two bands with the same structure differ only in values,
/// which glide; a different structure fades.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Structure {
    shape: Shape,
    slope: Slope,
    /// Its place in the stereo image: another placement is another band, crossfaded or, across
    /// the family boundary, faded out and in at each family's stage (plan §7.3).
    placement: Placement,
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
    /// The dynamics changed since the last control step read them.
    dynamics_changed: bool,
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
    /// Whether the detector reads the side-chain input instead of the EQ's input (plan §7.2).
    side_chain: bool,
    /// Where the band works in the stereo image, as set (plan §7.3).
    placement: Placement,
    /// The loudest detector output this interval, across channels.
    peak: f64,
    /// Whether the band listens this interval: a dynamic shape, Range or a moving part not 0, and
    /// the gain scale not 0.
    listening: bool,
    /// The last control step moved the moving part.
    dynamics_moved: bool,
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
        dynamics_changed: false,
        range_db: Glide::at(0.0),
        // Set for the engine's interval in `Engine::new`.
        ballistics: Ballistics {
            attack_keep: 0.0,
            release_keep: 0.0,
        },
        moving_db: 0.0,
        detector: Filter::EMPTY,
        detector_for: None,
        side_chain: false,
        placement: Placement::Stereo,
        peak: 0.0,
        listening: false,
        dynamics_moved: false,
    };

    /// Whether this band does anything to the audio this sample.
    fn active(&self) -> bool {
        self.fade != Fade::None || (self.current.is_some() && !self.parked)
    }

    /// What the band should be built as, its placement included.
    fn wanted(&self) -> Option<Structure> {
        self.target.structure().map(|structure| Structure {
            placement: self.placement,
            ..structure
        })
    }

    /// The placement the audio is at or heading to: what the detector and solo follow, so a change
    /// set while another fade runs waits with the band's own transition (plan §7.3).
    fn effective_placement(&self) -> Placement {
        // With no destination the band is fading out: its outgoing arm is what is heard.
        self.destination
            .or(self.current.map(|running| running.structure))
            .map_or(Placement::Stereo, |structure| structure.placement)
    }

    /// Whether an arm of the band is Stereo: in mono only those are heard.
    fn has_stereo_arm(&self) -> bool {
        let stereo = |running: &Option<Running>| {
            running
                .as_ref()
                .is_some_and(|running| running.structure.placement == Placement::Stereo)
        };
        stereo(&self.current) || stereo(&self.previous)
    }

    /// Whether an arm of the band runs on the mid or the side, so the mid/side stage must run.
    fn has_mid_side_arm(&self) -> bool {
        let mid_side = |running: &Option<Running>| {
            running
                .as_ref()
                .is_some_and(|running| running.structure.placement.is_mid_side())
        };
        (mid_side(&self.current) && !self.parked) || mid_side(&self.previous)
    }

    /// The detector hears its placement's channel of `source`, the input or the key: a Stereo band
    /// the linked peak of both (F1), a placed band its own channel (decision 20), a placed band in
    /// mono nothing.
    fn detect<const C: usize>(&mut self, source: [f64; C]) {
        let peak = match self.effective_placement() {
            Placement::Stereo => self
                .detector
                .process(source)
                .into_iter()
                .fold(0.0, |peak: f64, value| peak.max(value.abs())),
            _ if C < 2 => return,
            Placement::Left => self.detector.process([source[0]])[0].abs(),
            Placement::Right => self.detector.process([source[1]])[0].abs(),
            Placement::Mid => self.detector.process([(source[0] + source[1]) * 0.5])[0].abs(),
            Placement::Side => self.detector.process([(source[0] - source[1]) * 0.5])[0].abs(),
        };
        self.peak = self.peak.max(peak);
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
        let placement = |structure: Option<Structure>| structure.map(|s| s.placement);
        if placement(wanted) != placement(self.destination) {
            // The detector moves to its new channel with the band's own transition.
            self.detector_for = None;
            self.peak = 0.0;
        }
        self.destination = wanted;
        if old.is_none() {
            // Out of silence the band starts where it is drawn, not gliding from the values the
            // slot last held: its Range too, as configured now, and no moving part.
            self.snap();
            self.stop_listening();
            let dynamic = wanted.is_some_and(|structure| structure.shape.is_dynamic());
            self.range_db = Glide::at(if dynamic {
                f64::from(self.dynamics.range_db)
            } else {
                0.0
            });
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

    /// The band in one stage of the cascade (plan §7.3): `mid_side` false is the channel stage,
    /// its signal `[L, R]` (or the one channel in mono), true the mid/side stage, `[M, S]`. Each arm
    /// runs in its own family's stage: a crossfade within a family mixes its arms where they both
    /// run, as v1's did; across the boundary the old arm fades out to identity in its stage while
    /// the new one fades in from identity in its own. The fade's countdown is
    /// [`advance`](Self::advance)'s, once a sample.
    #[inline]
    fn tick<const C: usize>(&mut self, input: [f64; C], mid_side: bool, fade: u32) -> [f64; C] {
        match self.fade {
            Fade::None => match &mut self.current {
                Some(running)
                    if !self.parked && running.structure.placement.is_mid_side() == mid_side =>
                {
                    run_arm(running, input)
                }
                _ => input,
            },
            Fade::In | Fade::Out => match &mut self.current {
                Some(running) if running.structure.placement.is_mid_side() == mid_side => {
                    run_arm(running, input)
                }
                _ => input,
            },
            Fade::Cross => {
                // A dry arm runs with the other, as the identity it is.
                let family = |running: &Option<Running>| {
                    running.map(|r| r.structure.placement.is_mid_side())
                };
                let (old_family, new_family) = (family(&self.previous), family(&self.current));
                let old_stage = old_family.or(new_family).unwrap_or(false);
                let new_stage = new_family.or(old_family).unwrap_or(false);
                let t = 1.0 - f64::from(self.left) / f64::from(fade);
                if old_stage == new_stage {
                    if old_stage != mid_side {
                        return input;
                    }
                    let old = match &mut self.previous {
                        Some(running) => run_arm(running, input),
                        None => input,
                    };
                    let new = match &mut self.current {
                        Some(running) => run_arm(running, input),
                        None => input,
                    };
                    let mut mixed = old;
                    for (out, new) in mixed.iter_mut().zip(new) {
                        *out += (new - *out) * t;
                    }
                    mixed
                } else {
                    let (arm, weight) = if old_stage == mid_side {
                        (&mut self.previous, 1.0 - t)
                    } else {
                        (&mut self.current, t)
                    };
                    let Some(running) = arm else {
                        return input;
                    };
                    let filtered = run_arm(running, input);
                    let mut mixed = input;
                    for (out, filtered) in mixed.iter_mut().zip(filtered) {
                        *out += (filtered - *out) * weight;
                    }
                    mixed
                }
            }
        }
    }

    /// `frames` of the fade in progress at once, for frames no stage ran: a placed band in mono.
    fn advance_by(&mut self, frames: usize) {
        // Its coefficient ramps too, or a band edited while unheard would never settle.
        for running in [&mut self.current, &mut self.previous]
            .into_iter()
            .flatten()
        {
            running.filter.skip(frames);
        }
        self.detector.skip(frames);
        if self.fade != Fade::None {
            self.left = self
                .left
                .saturating_sub(u32::try_from(frames).unwrap_or(u32::MAX));
            if self.left == 0 {
                self.finish();
            }
        }
    }

    /// One sample of the fade in progress, after every stage has run.
    #[inline]
    fn advance(&mut self) {
        if self.fade != Fade::None {
            self.left = self.left.saturating_sub(1);
            if self.left == 0 {
                self.finish();
            }
        }
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

    /// Whether the next control step will move the moving part: something audible waiting to
    /// happen (`Engine::is_settled`). Once the interval is `complete` its peak is final, and any
    /// move it asks for counts, attack or release. Partway through, only an attack can be known:
    /// the peak so far already asks for more action than the band takes, while the interval's last
    /// samples may still bring a falling peak back.
    fn pending_move(&self, complete: bool) -> bool {
        if !self.listening {
            return false;
        }
        let level_db = if self.peak > 0.0 {
            20.0 * self.peak.log10()
        } else {
            f64::NEG_INFINITY
        };
        let target = law(
            level_db,
            f64::from(self.dynamics.threshold_db),
            self.range_db.value(),
        );
        if complete {
            return self.ballistics.step(self.moving_db, target) != self.moving_db;
        }
        target != 0.0
            && (self.moving_db == 0.0 || self.moving_db.signum() == target.signum())
            && target.abs() > self.moving_db.abs() + crate::dynamics::LAND_DB
    }

    /// No moving part and no detector: what a band that stops listening keeps.
    fn stop_listening(&mut self) {
        self.listening = false;
        self.dynamics_moved = self.moving_db != 0.0;
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
    /// Whether the audition built plays the side chain: what its band listens to (plan §7.2). A
    /// band that changes source is another audition, as another structure is.
    keyed: bool,
    /// The placement of its band when it was built: the audition plays the band's channel, where it
    /// sits in the image (plan §7.3). Another placement is another audition.
    placement: Placement,
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
        keyed: false,
        placement: Placement::Stereo,
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
    /// The layout the block calls run, mono or stereo: an engine is built for one, so this is
    /// known from the first call. In mono a band placed off Stereo is not in the sound (§7.3).
    mono: bool,
    /// Linear phase, when the engine was built for it (plan §7.7): the convolver and its design
    /// thread; what the kernel should hear, made when a setting it hears moves and asked for at
    /// the next update boundary; and the requests' sequence.
    linear: Option<Box<Linear>>,
    linear_dirty: bool,
    linear_wanted: Option<Heard>,
    linear_seq: u64,
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
            mono: false,
            linear: None,
            linear_dirty: false,
            linear_wanted: None,
            linear_seq: 0,
        }
    }

    /// An engine for `sample_rate` in `phase` (plan §7.7), for `channels` (1 or 2): Zero latency is
    /// [`Engine::new`]; Linear phase allocates its convolver and starts its design thread here, so
    /// build it off the audio thread. If the thread cannot start, the engine is Zero latency, and
    /// [`Engine::latency`] says so.
    pub fn with_phase(sample_rate: f64, phase: Phase, channels: usize) -> Engine {
        let mut engine = Engine::new(sample_rate);
        engine.mono = channels == 1;
        if let Phase::Linear(resolution) = phase {
            engine.linear = Linear::new(engine.sample_rate, resolution, channels).map(Box::new);
            engine.linear_dirty = true;
        }
        engine
    }

    /// The phase the engine runs.
    pub fn phase(&self) -> Phase {
        self.linear.as_ref().map_or(Phase::ZeroLatency, |linear| {
            Phase::Linear(linear.resolution())
        })
    }

    /// The latency in samples: 0 in Zero latency, the kernel's centre and a partition in Linear
    /// phase.
    pub fn latency(&self) -> u32 {
        self.linear
            .as_ref()
            .map_or(0, |linear| linear.latency() as u32)
    }

    /// Offline rendering (plan §7.7): in Linear phase a kernel asked for is waited for at the
    /// boundary after, so a bounce is the same every time.
    pub fn set_offline(&mut self, offline: bool) {
        if let Some(linear) = &mut self.linear {
            linear.set_offline(offline);
        }
    }

    /// In Linear phase, the kernel for the settings as they stand designed now and heard from the
    /// next sample: for activation, after the settings are set, off the audio thread.
    pub fn prime(&mut self) {
        if self.linear.is_none() {
            return;
        }
        self.retarget_all();
        let heard = self.linear_heard();
        self.linear_dirty = false;
        self.linear_wanted = None;
        if let Some(linear) = &mut self.linear {
            linear.prime(&heard);
        }
    }

    /// Every band's targets read now, as an interval's start reads them.
    fn retarget_all(&mut self) {
        let sample_rate = self.sample_rate;
        for band in &mut self.bands {
            if band.changed {
                band.retarget(sample_rate);
                band.changed = false;
            }
        }
    }

    /// What Linear phase's kernel hears (plan §7.7): every band the settings put in the sound, at
    /// its targets — the gain held inside ±30 dB, then under the gain scale's target — a dynamic
    /// band at its drawn gain, and in mono only Stereo bands; with the next sequence number.
    fn linear_heard(&mut self) -> Heard {
        self.linear_seq += 1;
        let scale = self.gain_scale.target;
        let mut heard = Heard::EMPTY;
        heard.seq = self.linear_seq;
        for (slot, band) in self.bands.iter().enumerate() {
            let Some(structure) = band.wanted() else {
                continue;
            };
            if self.mono && structure.placement != Placement::Stereo {
                continue;
            }
            let inputs = Design {
                shape: structure.shape,
                slope: structure.slope,
                freq: band.log_freq.target.exp(),
                gain_db: if structure.shape.has_gain() {
                    band.gain_db.target.clamp(-GAIN_RANGE, GAIN_RANGE) * scale
                } else {
                    0.0
                },
                q: band.log_q.target.exp(),
            };
            heard.bands[slot] = Some((design(inputs, self.sample_rate), structure.placement));
        }
        heard
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
                self.linear_dirty = true;
            }
        }
    }

    pub fn band(&self, slot: usize) -> Option<Band> {
        self.bands.get(slot).map(|engine| engine.target)
    }

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
                band.dynamics_changed = true;
            }
        }
    }

    /// Whether a slot's dynamics listen to the side chain rather than the EQ's input (plan §7.2).
    /// A switch starts the detector fresh on its new source, as a change of shape does; the moving
    /// part carries on through the ballistics, so the gain never steps.
    pub fn set_side_chain(&mut self, slot: usize, keyed: bool) {
        if let Some(band) = self.bands.get_mut(slot) {
            if band.side_chain != keyed {
                band.side_chain = keyed;
                band.detector_for = None;
                band.peak = 0.0;
                band.dynamics_changed = true;
            }
        }
    }

    /// Where a slot works in the stereo image (plan §7.3). A change is a change of structure,
    /// faded as one; the detector and solo move to the new channel when that transition starts.
    pub fn set_placement(&mut self, slot: usize, placement: Placement) {
        if let Some(band) = self.bands.get_mut(slot) {
            if band.placement != placement {
                band.placement = placement;
                self.linear_dirty = true;
            }
        }
    }

    /// A slot's placement, as last set.
    pub fn placement(&self, slot: usize) -> Option<Placement> {
        self.bands.get(slot).map(|band| band.placement)
    }

    /// Whether a slot listens to the side chain, as last set.
    pub fn side_chain(&self, slot: usize) -> Option<bool> {
        self.bands.get(slot).map(|band| band.side_chain)
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

    /// The output gain as a linear factor, held inside ±24 dB. It ramps over a fade.
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
        if scale != self.gain_scale.target {
            self.linear_dirty = true;
        }
        self.gain_scale.retarget(scale);
    }

    /// The gain scale where it has glided to: what [`gains_db`](Self::gains_db)'s gains are heard
    /// under now, for the editor.
    pub fn gain_scale(&self) -> f64 {
        self.gain_scale.value()
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
            let wanted = band.wanted();
            band.destination = wanted;
            band.previous = None;
            band.fade = Fade::None;
            band.left = 0;
            band.parked = false;
            // The dynamic state first: no moving part, Range as configured, so the filter below
            // is built from the drawn gain alone.
            band.stop_listening();
            band.dynamics_changed = false;
            band.dynamics_moved = false;
            let range = band.dynamics.range_db;
            let dynamic = wanted.is_some_and(|structure| structure.shape.is_dynamic());
            band.range_db = Glide::at(if dynamic { f64::from(range) } else { 0.0 });
            band.current = wanted.map(|structure| Running {
                structure,
                filter: Filter::fresh(&design_running(
                    band.design_inputs(structure, scale),
                    sample_rate,
                )),
            });
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
        if let Some(linear) = &mut self.linear {
            linear.clear();
        }
    }

    /// Nothing to do: every band parked or dry with no fade, no audition, the output at exactly
    /// 0 dB. The buffer is then left untouched.
    pub fn is_dry(&self) -> bool {
        // Linear phase always delays, even an empty EQ.
        if self.linear.is_some() {
            return false;
        }
        // In mono a band with no Stereo arm is not in the sound, so it renders nothing (§7.3).
        self.bands
            .iter()
            .all(|band| !band.active() || (self.mono && !band.has_stereo_arm()))
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
            || self
                .linear
                .as_ref()
                .is_some_and(|linear| linear.is_ringing())
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
        // The interval in progress has run its course: its peaks are final.
        let complete = self.countdown == 0;
        let band_settled = |band: &BandEngine| {
            let gliding = band.current.as_ref().is_some_and(|running| {
                let Structure { shape, slope, .. } = running.structure;
                band.log_freq.is_moving()
                    || (shape.has_gain() && band.gain_db.is_moving())
                    || (shape.has_q(slope) && band.log_q.is_moving())
            });
            !band.changed
                && !band.dynamics_changed
                && band.wanted() == band.destination
                && band.fade == Fade::None
                && !gliding
                && !band.dynamics_moved
                && !band.range_db.is_moving()
                && !band.pending_move(complete)
                && still(&band.current)
                && still(&band.previous)
        };
        // The scale is heard only through a gain shape with a gain to scale.
        let scale_heard = self.bands.iter().any(|band| {
            band.destination
                .is_some_and(|structure| structure.shape.has_gain())
                && (band.gain_db.value() != 0.0
                    || band.gain_db.target != 0.0
                    || band.moving_db != 0.0
                    // A band with Range hears the scale too: at 0 % it stops listening and parks.
                    || band.range_db.value() != 0.0
                    || band.range_db.target != 0.0)
        });
        let solo = &self.solo;
        let solo_mix_target = if solo.heard { 1.0 } else { 0.0 };
        let linear_settled = self.linear.as_ref().is_none_or(|linear| {
            !self.linear_dirty && self.linear_wanted.is_none() && linear.is_settled()
        });
        linear_settled
            && self.bands.iter().all(band_settled)
            && !(scale_heard && self.gain_scale.is_moving())
            && solo.slot == self.solo_wanted()
            && solo.mix == solo_mix_target
            && !solo.filter.is_ramping()
            && self.output_left == 0
    }

    /// The band whose audition should be built: the one asked for, if it is live and offers solo.
    fn solo_wanted(&self) -> Option<usize> {
        self.solo.wanted.filter(|&slot| {
            self.bands[slot].destination.is_some_and(|structure| {
                structure.shape.offers_solo()
                    && (!self.mono || structure.placement == Placement::Stereo)
            })
        })
    }

    /// Processes a stereo block in place — a Stereo band alike on both channels, a placed band on
    /// its own channel or the mid or side (plan §7.3) — with no side chain: a keyed
    /// band hears silence.
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.process_stereo_keyed(left, right, None);
    }

    /// Processes a mono block in place, with no side chain.
    pub fn process_mono(&mut self, buffer: &mut [f32]) {
        self.process_mono_keyed(buffer, None);
    }

    /// Processes a stereo block in place with the side chain's two channels, `key`, which only a
    /// keyed band's detector and solo hear (plan §7.2). A key shorter than the block is silence
    /// past its end.
    pub fn process_stereo_keyed(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        key: Option<[&[f32]; 2]>,
    ) {
        let frames = left.len().min(right.len());
        self.run::<2>([&mut left[..frames], &mut right[..frames]], key);
    }

    /// Processes a mono block in place with the side chain's one channel.
    pub fn process_mono_keyed(&mut self, buffer: &mut [f32], key: Option<&[f32]>) {
        self.run::<1>([buffer], key.map(|key| [key]));
    }

    /// Runs the block, starting a control interval wherever the one in progress ends.
    fn run<const C: usize>(&mut self, mut channels: [&mut [f32]; C], key: Option<[&[f32]; C]>) {
        self.mono = C == 1;
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
            if self.linear.is_some() {
                self.render_linear::<C>(&mut channels, start, end);
            } else if !self.dry {
                self.render::<C>(&mut channels, key, start, end);
            } else if self.mono {
                // A placed band in mono is not heard, but its fades still run their length, or a
                // change waiting on them would never start (plan §7.3).
                for band in &mut self.bands {
                    band.advance_by(end - start);
                }
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
        // Linear phase holds a dynamic band at its drawn gain (decision 37).
        let linear = self.linear.is_some();
        let scale_moved = self.gain_scale.step(keep, 1.0e-7);
        let scale = self.gain_scale.value();
        let (sample_rate, fade) = (self.sample_rate, self.fade);

        for band in &mut self.bands {
            if band.changed {
                band.retarget(sample_rate);
                band.changed = false;
            }
            band.dynamics_changed = false;
            let wanted = band.wanted();
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
            let dynamic_shape = structure.shape.is_dynamic() && !linear;
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
            band.dynamics_moved = dynamics_moved;

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
        // What the kernel should hear, once a setting it hears has moved; asked for at the next
        // update boundary.
        if linear && self.linear_dirty {
            self.linear_dirty = false;
            let heard = self.linear_heard();
            self.linear_wanted = Some(heard);
        }
    }

    fn control_solo(&mut self, scale: f64, frames: usize) {
        let valid = self.solo_wanted();
        // Linear phase holds dynamics, so no audition plays the key there.
        let linear = self.linear.is_some();
        let bands = &self.bands;
        let solo = &mut self.solo;
        // Only a dynamic shape listens to the key; any other plays its region of the input, the
        // switch it may have kept from an earlier shape aside (plan §7.2).
        let keyed = |band: &BandEngine, structure: Option<Structure>| {
            !linear
                && band.side_chain
                && structure.is_some_and(|structure| structure.shape.is_dynamic())
        };
        let matches = |slot: usize, solo: &Solo| {
            bands[slot].destination == solo.structure
                && keyed(&bands[slot], solo.structure) == solo.keyed
                && bands[slot].effective_placement() == solo.placement
        };
        let current = solo.slot.is_some()
            && solo.slot == valid
            && solo.slot.is_some_and(|slot| matches(slot, solo));
        if !current && solo.mix <= 0.0 && (solo.slot.is_some() || valid.is_some()) {
            // Faded out: build the audition now wanted, from silence.
            solo.slot = None;
            solo.structure = None;
            solo.keyed = false;
            solo.filter = Filter::EMPTY;
            if let Some(slot) = valid {
                let band = &bands[slot];
                if let Some(structure) = band.destination {
                    if let Some(cascade) =
                        design_solo(band.design_inputs(structure, scale), self.sample_rate)
                    {
                        solo.slot = Some(slot);
                        solo.structure = Some(structure);
                        solo.keyed = keyed(band, Some(structure));
                        solo.placement = band.effective_placement();
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
            && solo.slot.is_some_and(|slot| matches(slot, solo));
    }

    /// Frames `start..end` of the interval in progress, in place.
    fn render<const C: usize>(
        &mut self,
        channels: &mut [&mut [f32]; C],
        key: Option<[&[f32]; C]>,
        start: usize,
        end: usize,
    ) {
        let solo_target = if self.solo.heard { 1.0 } else { 0.0 };
        let solo_step = 1.0 / f64::from(self.fade);
        let fade = self.fade;
        // The key is read only when something hears it, and then counts as input: a key that plays
        // while the EQ's input rests is not the end of a tail.
        let keyed = self
            .bands
            .iter()
            .any(|band| band.listening && band.side_chain)
            || (self.solo.keyed && self.solo.slot.is_some());
        let key = key.filter(|_| keyed);
        // Two stages (plan §7.3, decision 22): every Stereo, Left and Right band, then every Mid
        // and Side band on the encoded pair. Arms change only at a control step or a fade's end, so
        // this is decided once for the frames here; with no mid/side arm the second stage never
        // runs, and an EQ of Stereo bands is v1's, bit for bit.
        let mid_side = C == 2 && self.bands.iter().any(BandEngine::has_mid_side_arm);
        let mut heard_input = false;
        let finite = |sample: f32| {
            if sample.is_finite() {
                f64::from(sample)
            } else {
                0.0
            }
        };
        for index in start..end {
            let mut dry = [0.0f64; C];
            for (value, channel) in dry.iter_mut().zip(channels.iter()) {
                *value = finite(channel[index]);
                heard_input |= *value != 0.0;
            }
            let mut keyed_in = [0.0f64; C];
            if let Some(key) = &key {
                for (value, channel) in keyed_in.iter_mut().zip(key.iter()) {
                    *value = channel.get(index).copied().map_or(0.0, finite);
                    heard_input |= *value != 0.0;
                }
            }

            let mut wet = dry;
            for band in &mut self.bands {
                if band.listening {
                    band.detect(if band.side_chain { keyed_in } else { dry });
                }
                if band.active() {
                    wet = band.tick(wet, false, fade);
                    if !mid_side {
                        band.advance();
                    }
                }
            }
            if mid_side {
                let mut pair = wet;
                pair[0] = (wet[0] + wet[1]) * 0.5;
                pair[1] = (wet[0] - wet[1]) * 0.5;
                for band in &mut self.bands {
                    if band.active() {
                        pair = band.tick(pair, true, fade);
                        band.advance();
                    }
                }
                wet[0] = pair[0] + pair[1];
                wet[1] = pair[0] - pair[1];
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
                let source = if solo.keyed { keyed_in } else { dry };
                let audition = audition(&mut solo.filter, solo.placement, source);
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

    /// Frames `start..end` in Linear phase (plan §7.7): through the kernel, solo on the input
    /// delayed by the latency, the output gain after; a kernel asked for at each update boundary
    /// when the settings it hears have moved.
    fn render_linear<const C: usize>(
        &mut self,
        channels: &mut [&mut [f32]; C],
        start: usize,
        end: usize,
    ) {
        let solo_target = if self.solo.heard { 1.0 } else { 0.0 };
        let solo_step = 1.0 / f64::from(self.fade);
        let mut heard_input = false;
        let finite = |sample: f32| {
            if sample.is_finite() {
                f64::from(sample)
            } else {
                0.0
            }
        };
        for index in start..end {
            let mut dry = [0.0f64; C];
            for (value, channel) in dry.iter_mut().zip(channels.iter()) {
                *value = finite(channel[index]);
                heard_input |= *value != 0.0;
            }
            let Some(linear) = self.linear.as_mut() else {
                return;
            };
            let (mut wet, delayed) = linear.tick(dry);
            if linear.take_boundary() {
                if let Some(heard) = self.linear_wanted.take() {
                    linear.ask(&heard);
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
                let audition = audition(&mut solo.filter, solo.placement, delayed);
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
                let sample = sample.clamp(-OUTPUT_BOUND, OUTPUT_BOUND) as f32;
                channel[index] = if sample.abs() < f32::MIN_POSITIVE {
                    0.0
                } else {
                    sample
                };
            }
            if blown {
                self.clear_states();
                if let Some(linear) = &mut self.linear {
                    linear.clear();
                }
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
