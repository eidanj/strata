//! Performance profiles: JVM flags, memory and video settings.
//! Phase 1 lists and selects them; Phase 3 applies them at launch
//! (JVM args to the command line, `options` to options.txt).

use serde_json::Value;

use crate::error::{Error, Result};
use crate::instance;
use crate::paths::{write_json_atomic, Paths};

const SEEDS: [(&str, &str); 4] = [
    ("max-fps.json", include_str!("../profiles/max-fps.json")),
    ("balanced.json", include_str!("../profiles/balanced.json")),
    ("competitive.json", include_str!("../profiles/competitive.json")),
    ("recording.json", include_str!("../profiles/recording.json")),
];

/// Write the built-in profiles if they're missing. Never overwrites edits.
pub fn seed(paths: &Paths) -> Result<()> {
    let dir = paths.profiles_dir();
    std::fs::create_dir_all(&dir)?;
    for (name, body) in SEEDS {
        let path = dir.join(name);
        if !path.exists() {
            let value: Value = serde_json::from_str(body)?;
            write_json_atomic(&path, &value)?;
        }
    }
    Ok(())
}

pub fn list(paths: &Paths) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(paths.profiles_dir())? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(value) = serde_json::from_slice::<Value>(&std::fs::read(&path)?) {
            out.push(value);
        }
    }
    out.sort_by_key(|p| p.get("order").and_then(Value::as_i64).unwrap_or(99));
    Ok(out)
}

pub fn set_active(paths: &Paths, profile_id: &str) -> Result<()> {
    let exists = list(paths)?.iter().any(|p| p.get("id").and_then(Value::as_str) == Some(profile_id));
    if !exists {
        return Err(Error::msg(format!("No profile named {profile_id}")));
    }
    let id = instance::active_id(paths)?;
    let mut inst = instance::load(paths, &id)?;
    inst.active_profile = profile_id.to_string();
    instance::save(paths, &inst)
}
