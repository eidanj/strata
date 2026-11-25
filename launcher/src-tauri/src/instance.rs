//! Instances (isolated game folders) and the per-instance content lock file.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::content::Kind;
use crate::error::Result;
use crate::paths::{write_json_atomic, Paths, DEFAULT_INSTANCE};

pub const MC_VERSION: &str = "26.3";
pub const LOADER: &str = "fabric";
pub const LOADER_VERSION: &str = "0.20.1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    pub loader: String,
    pub loader_version: String,
    pub active_profile: String,
    pub memory_mb: u32,
    #[serde(default)]
    pub extra_jvm_args: Vec<String>,
}

impl Instance {
    fn default_for(id: &str) -> Self {
        Self {
            id: id.to_string(),
            name: "Default".into(),
            minecraft_version: MC_VERSION.into(),
            loader: LOADER.into(),
            loader_version: LOADER_VERSION.into(),
            active_profile: "balanced".into(),
            memory_mb: 3072,
            extra_jvm_args: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub active_instance: String,
    pub close_on_launch: bool,
    #[serde(default)]
    pub java_path: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { active_instance: DEFAULT_INSTANCE.into(), close_on_launch: true, java_path: None }
    }
}

/// Create the folder layout and default files on first run. Safe to call on
/// every start: nothing that already exists is touched.
pub fn bootstrap(paths: &Paths) -> Result<()> {
    for dir in [paths.root().to_path_buf(), paths.meta_dir(), paths.core_dir(), paths.profiles_dir()] {
        std::fs::create_dir_all(dir)?;
    }
    if !paths.settings_file().exists() {
        write_json_atomic(&paths.settings_file(), &Settings::default())?;
    }
    let id = DEFAULT_INSTANCE;
    for kind in Kind::ALL {
        std::fs::create_dir_all(paths.content_dir(id, kind))?;
    }
    std::fs::create_dir_all(paths.config_dir(id))?;
    if !paths.instance_file(id).exists() {
        write_json_atomic(&paths.instance_file(id), &Instance::default_for(id))?;
    }
    crate::profiles::seed(paths)?;
    Ok(())
}

pub fn settings(paths: &Paths) -> Result<Settings> {
    Ok(read_json_or_default(&paths.settings_file())?)
}

pub fn active_id(paths: &Paths) -> Result<String> {
    Ok(settings(paths)?.active_instance)
}

pub fn load(paths: &Paths, id: &str) -> Result<Instance> {
    let file = paths.instance_file(id);
    if !file.exists() {
        return Ok(Instance::default_for(id));
    }
    Ok(serde_json::from_slice(&std::fs::read(file)?)?)
}

pub fn save(paths: &Paths, instance: &Instance) -> Result<()> {
    write_json_atomic(&paths.instance_file(&instance.id), instance)
}

fn read_json_or_default<T: serde::de::DeserializeOwned + Default>(path: &Path) -> Result<T> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------
// Lock file: who installed what. The folder is the truth for *what exists*;
// the lock adds *where it came from* (core bundle, Modrinth, or dropped in).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Core,
    Modrinth,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockEntry {
    pub kind: Kind,
    /// Canonical file name, always without a `.disabled` suffix.
    pub file: String,
    pub source: Source,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub sha512: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Lockfile {
    #[serde(default = "lock_version")]
    pub version: u32,
    #[serde(default)]
    pub entries: Vec<LockEntry>,
}

fn lock_version() -> u32 {
    1
}

impl Lockfile {
    pub fn load(paths: &Paths, id: &str) -> Result<Self> {
        read_json_or_default(&paths.lock_file(id))
    }

    pub fn save(&self, paths: &Paths, id: &str) -> Result<()> {
        write_json_atomic(&paths.lock_file(id), self)
    }

    pub fn by_file(&self, kind: Kind, file: &str) -> Option<&LockEntry> {
        self.entries.iter().find(|e| e.kind == kind && e.file == file)
    }

    pub fn by_project(&self, kind: Kind, project_id: &str) -> Option<&LockEntry> {
        self.entries
            .iter()
            .find(|e| e.kind == kind && e.project_id.as_deref() == Some(project_id))
    }

    /// Insert or replace. An entry for the same Modrinth project (an update)
    /// or the same file replaces the old one.
    pub fn upsert(&mut self, entry: LockEntry) {
        self.entries.retain(|e| {
            !(e.kind == entry.kind
                && (e.file == entry.file
                    || (entry.project_id.is_some() && e.project_id == entry.project_id)))
        });
        self.entries.push(entry);
    }

    pub fn remove_file(&mut self, kind: Kind, file: &str) {
        self.entries.retain(|e| !(e.kind == kind && e.file == file));
    }

    /// Drop entries whose files were deleted outside the launcher.
    pub fn prune(&mut self, paths: &Paths, id: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| {
            let dir = paths.content_dir(id, e.kind);
            dir.join(&e.file).exists() || dir.join(format!("{}.disabled", e.file)).exists()
        });
        before != self.entries.len()
    }
}
