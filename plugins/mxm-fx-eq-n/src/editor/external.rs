//! The external spectrum and collisions (plan §7.8, decision 43): one spectrum besides this
//! track's, the side chain's or another instance's post-EQ one, and where it may collide with this
//! track's sound.
//!
//! **Another track** is read through the registry (`crate::instances`): a watch while it is
//! chosen and the window open, its blocks through a second analyser of this editor's own, at its
//! settings but post only. **Collisions** are an indication, never a measurement: only while both
//! analysers hold sound, both levels as drawn and smoothed to a third of an octave in power, full
//! where the other is within 6 dB of this track or above it while this track is within 24 dB of
//! its peak.

use std::sync::{Arc, Weak};

use crate::instances::{self, Instance, Watch};
use crate::view::{Show, ViewSettings};

use super::spectrum::Spectrum;

/// Above this as drawn, in dB, a level counts towards a collision.
pub const COLLISION_FLOOR_DB: f32 = -80.0;
/// The smoothing's half-width either side of each drawn frequency, in octaves: a third of an
/// octave in all.
const SMOOTHING_OCTAVES: f32 = 1.0 / 6.0;
/// The longest name the selector and the line's label show; longer ones end in "…".
const NAME_CHARS: usize = 18;

/// What the External selector offers, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    Off,
    SideChain,
    Track(u64),
    /// A track chosen, saved with the project, that is not in this session.
    Missing(u64),
}

/// The selector's choices and their names: Off, the side chain on a layout with the port, every
/// other instance alphabetically, and a chosen track that is not found.
pub fn choices(own: &Instance, port: bool, settings: &ViewSettings) -> Vec<(Choice, String)> {
    let mut all = vec![(Choice::Off, "Off".to_owned())];
    if port {
        all.push((Choice::SideChain, "Side chain".to_owned()));
    }
    let others = instances::others(own);
    let chosen = settings.external;
    let found = others.iter().any(|listed| listed.id == chosen);
    all.extend(
        others
            .into_iter()
            .map(|listed| (Choice::Track(listed.id), short(&listed.name))),
    );
    if chosen != 0 && !found {
        all.push((Choice::Missing(chosen), "Track not found".to_owned()));
    }
    all
}

/// The choice the settings hold.
pub fn chosen(settings: &ViewSettings) -> Choice {
    if settings.external != 0 {
        Choice::Track(settings.external)
    } else if settings.side_chain {
        Choice::SideChain
    } else {
        Choice::Off
    }
}

/// The settings with `choice` made: one external spectrum at a time.
pub fn choose(settings: ViewSettings, choice: &Choice) -> ViewSettings {
    let (side_chain, external) = match *choice {
        Choice::Off => (false, 0),
        Choice::SideChain => (true, 0),
        Choice::Track(id) | Choice::Missing(id) => (false, id),
    };
    ViewSettings {
        side_chain,
        external,
        ..settings
    }
}

/// `name` held to [`NAME_CHARS`] characters.
pub fn short(name: &str) -> String {
    if name.chars().count() <= NAME_CHARS {
        name.to_owned()
    } else {
        let kept: String = name.chars().take(NAME_CHARS - 1).collect();
        format!("{kept}…")
    }
}

/// Another track's spectrum, while it is chosen and the window open.
pub struct Other {
    pub id: u64,
    entry: Weak<Instance>,
    _watch: Watch,
    pub spectrum: Spectrum,
    after: Option<u64>,
}

impl Other {
    pub fn new(entry: &Arc<Instance>, sample_rate: f32) -> Other {
        Other {
            id: entry.id(),
            entry: Arc::downgrade(entry),
            _watch: entry.watch(),
            spectrum: Spectrum::new(sample_rate),
            after: None,
        }
    }

    /// The frame's blocks into the analyser, after it has observed the frame's time; false once
    /// the track is gone.
    pub fn feed(&mut self, now: f64, settings: &ViewSettings) -> bool {
        let Some(entry) = self.entry.upgrade() else {
            return false;
        };
        self.spectrum.set_resolution(settings.resolution);
        self.spectrum.observe(now);
        let spectrum = &mut self.spectrum;
        entry.read(&mut self.after, |block| spectrum.ingest(block));
        true
    }

    /// The settings it is analysed at: this editor's, post only.
    pub fn settings(settings: &ViewSettings) -> ViewSettings {
        ViewSettings {
            show: Show::Post,
            side_chain: false,
            ..*settings
        }
    }
}

/// Each drawn frequency's collision strength, 0 to 1 (plan §7.8): `own` and `external` as drawn
/// at `freqs` (ascending), each smoothed to a third of an octave in power.
pub fn collisions(freqs: &[f32], own: &[f32], external: &[f32]) -> Vec<f32> {
    if freqs.len() != own.len() || freqs.len() != external.len() {
        return vec![0.0; freqs.len()];
    }
    let own = smoothed(freqs, own);
    let external = smoothed(freqs, external);
    let peak = own.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    own.iter()
        .zip(&external)
        .map(|(&o, &e)| {
            if o <= COLLISION_FLOOR_DB || e <= COLLISION_FLOOR_DB {
                return 0.0;
            }
            ((e - o + 12.0) / 6.0).clamp(0.0, 1.0) * ((o - (peak - 30.0)) / 6.0).clamp(0.0, 1.0)
        })
        .collect()
}

/// At each drawn frequency, the mean power of the drawn points within [`SMOOTHING_OCTAVES`] either
/// side, the window cut short by the ends, in dB.
fn smoothed(freqs: &[f32], db: &[f32]) -> Vec<f32> {
    let power: Vec<f64> = db
        .iter()
        .map(|db| 10f64.powf(f64::from(*db) / 10.0))
        .collect();
    let ratio = 2f32.powf(SMOOTHING_OCTAVES);
    let (mut low, mut high) = (0, 0);
    let mut sum = 0.0;
    freqs
        .iter()
        .map(|&freq| {
            while high < freqs.len() && freqs[high] <= freq * ratio {
                sum += power[high];
                high += 1;
            }
            while freqs[low] < freq / ratio {
                sum -= power[low];
                low += 1;
            }
            let mean = sum.max(0.0) / (high - low) as f64;
            (10.0 * mean.max(1.0e-30).log10()) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis() -> Vec<f32> {
        (0..400)
            .map(|i| 20.0 * 2f32.powf(i as f32 / 40.0))
            .collect()
    }

    /// **The strength at its edges** (plan §7.8): full where the other is within 6 dB of this
    /// track or above it and this track within 24 dB of its peak; nothing 12 dB under, nothing 30 dB
    /// under the peak, nothing at or below the floor.
    #[test]
    fn the_strength_has_its_edges() {
        let freqs = axis();
        let flat = |db: f32| vec![db; freqs.len()];
        let at = |own: f32, external: f32| collisions(&freqs, &flat(own), &flat(external))[200];
        assert!(
            (at(-20.0, -26.0) - 1.0).abs() < 1e-3,
            "{}",
            at(-20.0, -26.0)
        );
        assert!((at(-20.0, -10.0) - 1.0).abs() < 1e-3);
        assert!(at(-20.0, -32.0).abs() < 1e-3);
        assert!((at(-20.0, -29.0) - 0.5).abs() < 1e-3);
        assert_eq!(at(-90.0, -20.0), 0.0);
        assert_eq!(at(-20.0, -85.0), 0.0);
        // This track 30 dB under its own peak: nothing.
        let mut own = flat(-60.0);
        own[0..40].fill(-20.0);
        let strength = collisions(&freqs, &own, &flat(-20.0));
        assert!(strength[300].abs() < 1e-3, "{}", strength[300]);
        assert!(strength[10] > 0.99);
    }

    /// The smoothing is of power: a 20 dB spike one point wide comes out lower than 20 dB over its
    /// neighbours, the mean of the points within a sixth of an octave either side.
    #[test]
    fn the_smoothing_averages_power() {
        let freqs = axis();
        let mut db = vec![-40.0; freqs.len()];
        db[200] = -20.0;
        let smoothed = smoothed(&freqs, &db);
        // A third of an octave on 40 points an octave is about 14 points.
        assert!(
            smoothed[200] < -28.0 && smoothed[200] > -32.0,
            "{}",
            smoothed[200]
        );
        assert!((smoothed[100] + 40.0).abs() < 1e-3);
    }

    /// The selector's choices map to the settings and back, one external spectrum at a time.
    #[test]
    fn a_choice_is_one_external_spectrum() {
        let settings = ViewSettings::default();
        for choice in [Choice::Off, Choice::SideChain, Choice::Track(7)] {
            assert_eq!(chosen(&choose(settings, &choice)), choice);
        }
        let picked = choose(choose(settings, &Choice::SideChain), &Choice::Track(9));
        assert!(!picked.side_chain && picked.external == 9);
        assert_eq!(short("A very long track name indeed"), "A very long track…");
    }
}
