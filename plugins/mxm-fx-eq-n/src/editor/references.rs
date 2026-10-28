//! Saved references (plan §7.6, decision 48): what an EQ learned of its track's output, kept by
//! name, so any track in any project can be matched to it.
//!
//! - **A file per reference**, in a `references` folder beside the EQ's user presets (mxm-kit's
//!   `mxm_preset::library` layout), so an install never touches them. **A folder, not a dialog**,
//!   as the kit's banks are: Folder shows it, and a reference is renamed or deleted there.
//! - **Not a preset**: a preset holds parameter values only, and a reference is a measurement.
//! - **The file** is JSON: a format name and version, the name, the seconds heard, the rate it was
//!   learned at, the cells' spacing and first centre, the level per cell in dB, `null` where
//!   nothing was read, and how unsettled each is, a file without it taken as settled. One that does not parse, or of a newer version, is listed with the reason
//!   and cannot be chosen.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::finder::LOWEST_HZ;
use super::matcher::{CELLS, CELLS_PER_OCTAVE, Reference};

/// What a file says it is.
pub const FORMAT: &str = "mxm-fx-eq-n reference";
/// The version written; a newer one is listed and not read.
pub const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct File {
    format: String,
    version: u32,
    name: String,
    seconds: f64,
    sample_rate: f32,
    cells_per_octave: f32,
    lowest_hz: f32,
    db: Vec<Option<f32>>,
    #[serde(default)]
    spread: Vec<Option<f32>>,
}

/// Where the references live, beside the EQ's user presets; `None` on a system with no config
/// directory.
pub fn root() -> Option<PathBuf> {
    mxm_preset::library::user_root(crate::CLAP_ID)?
        .parent()
        .map(|dir| dir.join("references"))
}

/// A saved reference as listed: its name, its file, and what was read from it, or why nothing
/// could be.
#[derive(Clone, Debug)]
pub struct Listed {
    pub name: String,
    pub path: PathBuf,
    pub read: Result<Reference, String>,
}

/// The file a reference called `name` takes in `dir`: its name sanitised as a preset's is.
pub fn path_for(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{}.json", mxm_preset::library::dir_name(name)))
}

/// Every reference in `dir`, by name, case aside; nothing where the folder is not there.
pub fn list(dir: &Path) -> Vec<Listed> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut listed: Vec<Listed> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .map(|path| {
            let stem = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            match read(&path) {
                Ok((name, reference)) => Listed {
                    name,
                    path,
                    read: Ok(reference),
                },
                Err(why) => Listed {
                    name: stem,
                    path,
                    read: Err(why),
                },
            }
        })
        .collect();
    listed.sort_by_key(|listed| listed.name.to_lowercase());
    listed
}

/// A file's name and reference, or why it cannot be read.
pub fn read(path: &Path) -> Result<(String, Reference), String> {
    let text = std::fs::read_to_string(path).map_err(|why| format!("it cannot be read: {why}"))?;
    let file: File =
        serde_json::from_str(&text).map_err(|why| format!("it is not a reference: {why}"))?;
    if file.format != FORMAT {
        return Err("it is not a reference".to_owned());
    }
    if file.version > VERSION {
        return Err(format!(
            "it was saved by a newer version of the EQ (version {})",
            file.version
        ));
    }
    if file.cells_per_octave != CELLS_PER_OCTAVE || file.lowest_hz != LOWEST_HZ {
        return Err("its levels are on other frequencies".to_owned());
    }
    let mut cells = file.db;
    cells.resize(CELLS, None);
    let mut spread = file.spread;
    if !spread.is_empty() {
        spread.resize(CELLS, None);
    }
    Ok((
        file.name,
        Reference {
            cells,
            spread,
            seconds: file.seconds,
            sample_rate: file.sample_rate,
        },
    ))
}

/// Saves `reference` as `name` in `dir`, made if it is not there, replacing a file of that name.
/// The text goes to a file beside it first and is moved over it, so a failed save leaves the old
/// one whole.
pub fn save(dir: &Path, name: &str, reference: &Reference) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir)
        .map_err(|why| format!("{} cannot be made: {why}", dir.display()))?;
    let file = File {
        format: FORMAT.to_owned(),
        version: VERSION,
        name: name.to_owned(),
        seconds: reference.seconds,
        sample_rate: reference.sample_rate,
        cells_per_octave: CELLS_PER_OCTAVE,
        lowest_hz: LOWEST_HZ,
        db: reference.cells.clone(),
        spread: reference.spread.clone(),
    };
    let text = serde_json::to_string(&file).map_err(|why| why.to_string())?;
    let path = path_for(dir, name);
    let temp = path.with_extension("json.saving");
    std::fs::write(&temp, text).map_err(|why| {
        let _ = std::fs::remove_file(&temp);
        format!("{} cannot be written: {why}", temp.display())
    })?;
    std::fs::rename(&temp, &path).map_err(|why| {
        let _ = std::fs::remove_file(&temp);
        format!("{} cannot be replaced: {why}", path.display())
    })?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("mxm-fx-eq-n-references")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn reference() -> Reference {
        Reference {
            cells: (0..CELLS)
                .map(|cell| (cell % 7 != 0).then_some(-40.0 - cell as f32 * 0.1))
                .collect(),
            spread: (0..CELLS)
                .map(|cell| (cell % 5 != 0).then_some(0.25 + cell as f32 * 0.001))
                .collect(),
            seconds: 31.5,
            sample_rate: 44_100.0,
        }
    }

    /// **Saved and read back whole, listed by name** (plan §7.6): every cell, the unread ones too,
    /// the seconds and the rate; a name sanitised to a file and kept as written; the list by name.
    #[test]
    fn a_reference_is_saved_and_read_back_whole() {
        let dir = sandbox("whole");
        let saved = reference();
        let path = save(&dir, "Studio VO / mic 2", &saved).expect("saved");
        assert_eq!(path, path_for(&dir, "Studio VO / mic 2"));
        let (name, back) = read(&path).expect("read");
        assert_eq!(name, "Studio VO / mic 2");
        assert_eq!(back, saved);
        save(&dir, "another", &saved).expect("saved");
        let names: Vec<String> = list(&dir).into_iter().map(|listed| listed.name).collect();
        assert_eq!(names, ["another", "Studio VO / mic 2"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A file it cannot read is listed with the reason** (plan §7.6): one that does not parse,
    /// one of a newer version.
    #[test]
    fn an_unreadable_or_newer_file_is_listed_with_why() {
        let dir = sandbox("unreadable");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("broken.json"), "{ not json").unwrap();
        save(&dir, "newer", &reference()).unwrap();
        let newer = path_for(&dir, "newer");
        let text = std::fs::read_to_string(&newer).unwrap();
        std::fs::write(&newer, text.replace("\"version\":1", "\"version\":2")).unwrap();
        let listed = list(&dir);
        assert_eq!(listed.len(), 2);
        let why = |name: &str| {
            listed
                .iter()
                .find(|listed| listed.name == name)
                .and_then(|listed| listed.read.as_ref().err().cloned())
                .unwrap_or_default()
        };
        assert!(why("broken").contains("not a reference"));
        assert!(why("newer").contains("newer version"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
