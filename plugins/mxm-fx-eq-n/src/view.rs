//! The analyser's settings: view state, saved with the project (plan §4).
//!
//! They are not parameters, so they have no ids and no automation, and presets, Init and undo leave
//! them alone. An editor change marks the project dirty through a parameter-free GUI transaction
//! (`vendor/nice-plug/PATCHES.md`, *GUI-authored state dirty*; `mxm-fx-curve`'s precedent).
//!
//! **A restore is counted.** The wrapper calls [`ViewField`]'s `set` only when a host restores
//! the plugin's state, so the count tells the editor that the whole patch was replaced behind it,
//! which starts a new undo epoch (plan §5.4).

use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use nice_plug::params::persist::PersistentField;
use serde::{Deserialize, Serialize};

/// Which spectra the graph draws behind the curve.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Show {
    Pre,
    Post,
    #[default]
    Both,
}

impl Show {
    pub const ALL: [Show; 3] = [Show::Pre, Show::Post, Show::Both];

    pub fn label(self) -> &'static str {
        match self {
            Show::Pre => "Pre",
            Show::Post => "Post",
            Show::Both => "Both",
        }
    }
}

/// How finely the lows are resolved, which is also how long they take to settle: a low peak needs
/// a long window (research §8.6). The highs always use the short window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    /// The short window alone, about 85 ms.
    Low,
    /// The lows from a window of about 0.34 s.
    #[default]
    Medium,
    /// The lows from a window of about 1.4 s, enough to tell a room mode's width.
    High,
}

impl Resolution {
    pub const ALL: [Resolution; 3] = [Resolution::Low, Resolution::Medium, Resolution::High];

    pub fn label(self) -> &'static str {
        match self {
            Resolution::Low => "Low",
            Resolution::Medium => "Medium",
            Resolution::High => "High",
        }
    }
}

/// How fast a fallen level is let down; a rise is always shown at once.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Speed {
    Slow,
    #[default]
    Medium,
    Fast,
}

impl Speed {
    pub const ALL: [Speed; 3] = [Speed::Slow, Speed::Medium, Speed::Fast];

    pub fn label(self) -> &'static str {
        match self {
            Speed::Slow => "Slow",
            Speed::Medium => "Medium",
            Speed::Fast => "Fast",
        }
    }

    /// The fall, in dB per second.
    pub fn release_db_per_second(self) -> f32 {
        match self {
            Speed::Slow => 12.0,
            Speed::Medium => 36.0,
            Speed::Fast => 96.0,
        }
    }
}

/// The display's tilt about 1 kHz, so typical music reads level. The display reads band power, as
/// a real-time analyser does, where the research's corpus slope (Elowsson's −4.53 dB/octave per FFT
/// bin) is about −1.5 dB/octave (`research:effects/parametric-equalizers.md` §10.6, *Derived*), so
/// the default is 1.5. Pro-Q's 4.5 is the same correction on a per-bin display.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tilt {
    Flat,
    #[default]
    OneAndAHalf,
    Three,
    FourAndAHalf,
}

impl Tilt {
    pub const ALL: [Tilt; 4] = [
        Tilt::Flat,
        Tilt::OneAndAHalf,
        Tilt::Three,
        Tilt::FourAndAHalf,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tilt::Flat => "0",
            Tilt::OneAndAHalf => "1.5",
            Tilt::Three => "3",
            Tilt::FourAndAHalf => "4.5",
        }
    }

    pub fn db_per_octave(self) -> f32 {
        match self {
            Tilt::Flat => 0.0,
            Tilt::OneAndAHalf => 1.5,
            Tilt::Three => 3.0,
            Tilt::FourAndAHalf => 4.5,
        }
    }
}

/// The settings. Every field has a default, so a project saved before a field existed loads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewSettings {
    pub analyser: bool,
    pub show: Show,
    pub resolution: Resolution,
    pub speed: Speed,
    pub tilt: Tilt,
    /// The side chain's spectrum as a third line, the reference overlay (plan §7.4): offered only
    /// on a layout with the port.
    pub side_chain: bool,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            analyser: true,
            show: Show::default(),
            resolution: Resolution::default(),
            speed: Speed::default(),
            tilt: Tilt::default(),
            side_chain: false,
        }
    }
}

/// The persisted field: the settings, and how many times a host has restored them.
#[derive(Default)]
pub struct ViewField {
    settings: RwLock<ViewSettings>,
    restores: AtomicU64,
}

impl ViewField {
    pub fn get(&self) -> ViewSettings {
        *self
            .settings
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// An editor's change. Not a restore: it is not counted.
    pub fn edit(&self, settings: ViewSettings) {
        *self
            .settings
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = settings;
    }

    /// How many times a host has restored the plugin's state.
    pub fn restores(&self) -> u64 {
        self.restores.load(Ordering::Acquire)
    }
}

impl<'a> PersistentField<'a, ViewSettings> for ViewField {
    /// A host's restore: the wrapper calls this only when it deserialises saved state.
    fn set(&self, new_value: ViewSettings) {
        self.edit(new_value);
        self.restores.fetch_add(1, Ordering::AcqRel);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&ViewSettings) -> R,
    {
        f(&self.get())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edit_is_not_a_restore_and_a_restore_is_counted() {
        let field = ViewField::default();
        assert_eq!(field.get(), ViewSettings::default());
        let quiet = ViewSettings {
            analyser: false,
            ..ViewSettings::default()
        };
        field.edit(quiet);
        assert_eq!((field.get(), field.restores()), (quiet, 0));
        PersistentField::set(&field, ViewSettings::default());
        assert_eq!(field.restores(), 1);
        assert!(field.get().analyser);
    }

    #[test]
    fn a_project_from_before_a_setting_existed_loads_with_its_default() {
        let settings: ViewSettings = serde_json::from_str(r#"{"analyser":false}"#).unwrap();
        assert_eq!(
            settings,
            ViewSettings {
                analyser: false,
                ..ViewSettings::default()
            }
        );
        let round: ViewSettings =
            serde_json::from_str(&serde_json::to_string(&ViewSettings::default()).unwrap())
                .unwrap();
        assert_eq!(round, ViewSettings::default());
    }
}
