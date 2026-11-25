//! Where everything lives on disk. See the README for the full layout.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::content::Kind;
use crate::error::Result;

pub const DEFAULT_INSTANCE: &str = "default-26.3";

#[derive(Clone)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    /// `<OS data dir>/Strata`, or `$STRATA_HOME` if set (handy for testing).
    pub fn resolve(app: &AppHandle) -> Result<Self> {
        let root = match std::env::var_os("STRATA_HOME") {
            Some(dir) => PathBuf::from(dir),
            None => app.path().data_dir()?.join("Strata"),
        };
        Ok(Self { root })
    }

    #[cfg(test)]
    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("strata.json")
    }
    pub fn meta_dir(&self) -> PathBuf {
        self.root.join("meta")
    }
    pub fn core_dir(&self) -> PathBuf {
        self.root.join("core")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }
    pub fn skins_dir(&self) -> PathBuf {
        self.root.join("accounts").join("skins")
    }
    pub fn versions_dir(&self) -> PathBuf {
        self.meta_dir().join("versions")
    }
    pub fn libraries_dir(&self) -> PathBuf {
        self.meta_dir().join("libraries")
    }
    pub fn assets_dir(&self) -> PathBuf {
        self.meta_dir().join("assets")
    }
    pub fn natives_dir(&self, version: &str) -> PathBuf {
        self.meta_dir().join("natives").join(version)
    }
    pub fn loaders_dir(&self) -> PathBuf {
        self.meta_dir().join("loaders")
    }
    pub fn profiles_dir(&self) -> PathBuf {
        self.root.join("profiles")
    }
    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.root.join("instances").join(id)
    }
    pub fn instance_file(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("instance.json")
    }
    pub fn lock_file(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("content.lock.json")
    }
    /// The isolated `.minecraft` equivalent passed to the game as `--gameDir`.
    pub fn game_dir(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("minecraft")
    }
    pub fn content_dir(&self, id: &str, kind: Kind) -> PathBuf {
        self.game_dir(id).join(kind.folder())
    }
    pub fn options_file(&self, id: &str) -> PathBuf {
        self.game_dir(id).join("options.txt")
    }
    pub fn config_dir(&self, id: &str) -> PathBuf {
        self.game_dir(id).join("config")
    }
    pub fn hud_config(&self, id: &str) -> PathBuf {
        self.config_dir(id).join("strata-hud.json")
    }
    pub fn iris_config(&self, id: &str) -> PathBuf {
        self.config_dir(id).join("iris.properties")
    }
}

/// Write JSON via a temp file + rename so a crash never leaves half a file.
pub fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Keep only the final path component and refuse anything that could escape
/// the target folder.
pub fn safe_file_name(name: &str) -> Result<String> {
    let base = Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    if base.is_empty() || base == "." || base == ".." || base.starts_with(".strata-") {
        return Err(crate::error::Error::msg(format!("Refusing unsafe file name: {name}")));
    }
    Ok(base)
}
