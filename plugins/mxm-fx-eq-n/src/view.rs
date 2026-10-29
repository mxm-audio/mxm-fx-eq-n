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
    /// on a layout with the port. One external spectrum at a time: never with `external`.
    pub side_chain: bool,
    /// Another track's spectrum instead (plan §7.8): that instance's id, 0 for none.
    pub external: u64,
    /// Marks where this track's sound and the external spectrum may collide (plan §7.8).
    pub collisions: bool,
    /// The resonance finder's Amount (plan §7.5), in whole percent: the share of each resonance's
    /// prominence a proposed cut takes. Not part of what the display shows, so a change ends no
    /// freeze ([`ViewSettings::display`]).
    pub amount: u8,
    /// The EQ match's Amount (plan §7.6), in whole percent: the share of the difference from the
    /// reference the match takes, up to `matcher::MAX_AMOUNT`.
    pub match_amount: u8,
    /// The EQ match's Detail (plan §7.6): at most how many bands it proposes, 1 to 24.
    pub match_detail: u8,
}

impl ViewSettings {
    /// The settings that decide what the display shows: every one but the Amounts and Detail,
    /// another track and Collisions. A freeze is held on these (plan §7.4, §7.8).
    pub fn display(self) -> ViewSettings {
        let defaults = ViewSettings::default();
        ViewSettings {
            amount: defaults.amount,
            match_amount: defaults.match_amount,
            match_detail: defaults.match_detail,
            external: defaults.external,
            collisions: defaults.collisions,
            ..self
        }
    }

    /// Amount as a share, 0 to 1.
    pub fn amount_share(self) -> f32 {
        f32::from(self.amount.min(100)) / 100.0
    }

    /// The match's Amount as a share, 0 to 2.
    pub fn match_amount_share(self) -> f32 {
        f32::from(self.match_amount.min(crate::editor::matcher::MAX_AMOUNT)) / 100.0
    }

    /// The match's Detail, held inside 1 to 24.
    pub fn match_detail(self) -> u8 {
        self.match_detail
            .clamp(1, crate::editor::matcher::MAX_DETAIL)
    }
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
            external: 0,
            collisions: false,
            amount: crate::editor::finder::DEFAULT_AMOUNT,
            match_amount: crate::editor::matcher::DEFAULT_AMOUNT,
            match_detail: crate::editor::matcher::DEFAULT_DETAIL,
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
    /// A host's restore: the wrapper calls this only when it deserialises saved state. Settings
    /// saved before one existed read back with its default: told to the wrapper, which otherwise
    /// rolls back the whole restore.
    fn set(&self, new_value: ViewSettings) {
        self.edit(new_value);
        self.restores.fetch_add(1, Ordering::AcqRel);
        nice_plug::wrapper::accept_canonicalized_persistent_field(crate::params::VIEW_KEY);
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
        // F4's Amount (plan §7.5): a project from before it loads at 20 %.
        let before_f4: ViewSettings =
            serde_json::from_str(r#"{"analyser":true,"show":"Both","side_chain":true}"#).unwrap();
        assert_eq!(before_f4.amount, 20);
        assert!(before_f4.side_chain);
        // F5's Amount and Detail (plan §7.6): a project from before them loads 100 % and 8.
        assert_eq!((before_f4.match_amount, before_f4.match_detail), (100, 8));
        let changed = ViewSettings {
            match_amount: 70,
            match_detail: 3,
            amount: 50,
            ..ViewSettings::default()
        };
        assert_eq!(
            changed.display(),
            ViewSettings::default().display(),
            "no Amount or Detail is part of what the display shows"
        );
        let round: ViewSettings =
            serde_json::from_str(&serde_json::to_string(&ViewSettings::default()).unwrap())
                .unwrap();
        assert_eq!(round, ViewSettings::default());
    }
}
