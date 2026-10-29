//! What an EQ has learned of its own track for Match (plan §7.6, decision 31), and what it is
//! matched to: both host state, saved with the project, never a preset's.
//!
//! - **Each EQ learns only its own track**: its input before the EQ, which the match changes, and
//!   its output after it, the sound another EQ matches to. One toggle, Learn, learns both.
//! - **Every other EQ in the process reads it** through the registry (`crate::instances`), whose
//!   entry shares this field's storage, so an EQ is matched to another whether or not that one's
//!   window is open, and after a reload.
//! - **Cells**, a 48th of an octave apart, so it does not depend on the rate it was learned at; the
//!   editor's matcher makes and reads them (`editor::matcher`).

use std::sync::{Arc, PoisonError, RwLock};

use nice_plug::params::persist::PersistentField;
use serde::{Deserialize, Serialize};

/// What an EQ learned of its own track (plan §7.6).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Learned {
    /// The cells' spacing and first centre, so cells of another grid are never read as these.
    pub cells_per_octave: f32,
    pub lowest_hz: f32,
    /// Per cell: the input's covariance while it sounds, before the EQ, power per hertz: `|L|²`,
    /// `|R|²`, and `L·conj(R)`'s real and imaginary parts; `None` where nothing was read.
    pub input: Vec<Option<[f32; 4]>>,
    /// Per cell: the output's level while it sounds, after the EQ, dB per hertz: what another EQ
    /// is matched to.
    pub output: Vec<Option<f32>>,
    /// Per cell, how unsettled the input's and the output's smoothed levels are, in dB, `None`
    /// where it cannot be told; empty, as a state from before them, for settled throughout.
    #[serde(default)]
    pub input_spread: Vec<Option<f32>>,
    #[serde(default)]
    pub output_spread: Vec<Option<f32>>,
    /// The seconds of it heard.
    pub seconds: f64,
    /// The rate it was learned at.
    pub sample_rate: f32,
}

/// The storage the field and the instance's registry entry share.
pub type Shared = Arc<RwLock<Option<Arc<Learned>>>>;

/// What this EQ learned, held in the host's state beside the view settings.
#[derive(Default)]
pub struct LearnedField {
    inner: Shared,
}

impl LearnedField {
    /// The storage the instance's registry entry shares.
    pub fn shared(&self) -> Shared {
        self.inner.clone()
    }

    pub fn get(&self) -> Option<Arc<Learned>> {
        self.inner
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn store(&self, learned: Option<Arc<Learned>>) {
        *self.inner.write().unwrap_or_else(PoisonError::into_inner) = learned;
    }
}

impl<'a> PersistentField<'a, Option<Learned>> for LearnedField {
    /// A host's restore. One saved before a field it now has reads back with that field's default:
    /// told to the wrapper, which otherwise rolls back the whole restore.
    fn set(&self, new_value: Option<Learned>) {
        self.store(new_value.map(Arc::new));
        nice_plug::wrapper::accept_canonicalized_persistent_field(crate::params::LEARNED_KEY);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&Option<Learned>) -> R,
    {
        f(&self.get().map(|learned| (*learned).clone()))
    }
}

/// A band Match's Apply wrote (plan §7.6, decision 51), as written: a slot that still holds one
/// like it, static, unbypassed and Stereo, holds Match's band, which the next fit leaves out and
/// the next Apply replaces; one edited by hand since is the player's.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppliedBand {
    /// The shape as the `type` parameter holds it (`params::type_of`).
    pub kind: i32,
    pub freq: f32,
    pub gain_db: f32,
    pub q: f32,
}

/// How many bands Match keeps a record of, the newest: enough for Undo through a few Applies to
/// find the bands of the one it lands on.
pub const APPLIED_KEPT: usize = 96;

impl AppliedBand {
    /// `band` as Apply wrote it; `None` for an Off one.
    pub fn of(band: &mxm_fx_eq_n_dsp::Band) -> Option<Self> {
        band.shape.map(|shape| Self {
            kind: crate::params::type_of(Some(shape)),
            freq: band.freq,
            gain_db: band.gain_db,
            q: band.q,
        })
    }

    /// Whether `band` is this one as the host reads it back: the same shape, its values within what
    /// the parameters' rounding moves them.
    pub fn is(&self, band: &mxm_fx_eq_n_dsp::Band) -> bool {
        let close = |a: f32, b: f32| (a - b).abs() <= 1.0e-3 * a.abs().max(b.abs()).max(1.0);
        crate::params::type_of(band.shape) == self.kind
            && close(band.freq, self.freq)
            && close(band.gain_db, self.gain_db)
            && close(band.q, self.q)
    }
}

/// What this EQ is matched to (plan §7.6): nothing, another EQ by its instance's id (plan §7.8),
/// or a saved reference by name (decision 48).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchTo {
    #[default]
    Nothing,
    Track(u64),
    Saved(String),
}
