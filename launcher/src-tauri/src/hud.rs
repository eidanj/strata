//! The HUD layout file shared with the in-game mod:
//! `instances/<id>/minecraft/config/strata-hud.json`.
//! The default lives in `shared/` so the launcher and mod can't drift apart.

use serde_json::Value;

use crate::error::{Error, Result};
use crate::instance;
use crate::paths::{write_json_atomic, Paths};

pub const DEFAULT_CONFIG: &str = include_str!("../../../shared/strata-hud.default.json");

pub fn default_config() -> Value {
    serde_json::from_str(DEFAULT_CONFIG).expect("shared/strata-hud.default.json is valid JSON")
}

/// The saved file laid over the defaults, so keys added in newer versions
/// always show up.
pub fn load(paths: &Paths) -> Result<Value> {
    let id = instance::active_id(paths)?;
    let mut config = default_config();
    if let Ok(bytes) = std::fs::read(paths.hud_config(&id)) {
        if let Ok(saved) = serde_json::from_slice::<Value>(&bytes) {
            merge(&mut config, saved);
        }
    }
    Ok(config)
}

pub fn save(paths: &Paths, config: Value) -> Result<()> {
    if !config.is_object() || config.get("modules").is_none() || config.get("crosshair").is_none() {
        return Err(Error::msg("HUD config is missing modules or crosshair"));
    }
    let id = instance::active_id(paths)?;
    write_json_atomic(&paths.hud_config(&id), &config)
}

fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(existing) => merge(existing, v),
                    None => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (slot, v) => *slot = v,
    }
}
