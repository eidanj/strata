//! Drag-and-drop (and "Add files…") import into the active instance.
//!
//! The frontend sends the paths from Tauri's native drop event plus the tab
//! they were dropped on. Everything is validated here, never trusted from
//! the file extension alone.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha512};

use crate::content::{self, JarKind, Kind};
use crate::error::Result;
use crate::install::replace_same_mod;
use crate::instance::{self, LockEntry, Lockfile, Source};
use crate::paths::{safe_file_name, Paths};
use crate::versions;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added: Vec<Added>,
    pub rejected: Vec<Rejected>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Added {
    pub file: String,
    pub replaced: Option<String>,
    pub warning: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rejected {
    pub file: String,
    pub reason: String,
}

pub fn import(paths: &Paths, kind: Kind, sources: &[String]) -> Result<ImportReport> {
    let id = instance::active_id(paths)?;
    let inst = instance::load(paths, &id)?;
    let dir = paths.content_dir(&id, kind);
    fs::create_dir_all(&dir)?;

    let mut report = ImportReport::default();
    for src in sources {
        let src = PathBuf::from(src);
        let shown = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| src.display().to_string());
        match import_one(paths, &id, &inst.minecraft_version, kind, &src, &dir) {
            Ok(added) => report.added.push(added),
            Err(reason) => report.rejected.push(Rejected { file: shown, reason }),
        }
    }
    Ok(report)
}

/// `Err` carries a user-facing reason.
fn import_one(paths: &Paths, id: &str, mc: &str, kind: Kind, src: &Path, dir: &Path) -> std::result::Result<Added, String> {
    if !src.exists() {
        return Err("The file no longer exists.".into());
    }
    let name = src.file_name().and_then(|n| n.to_str()).ok_or("The file name isn't valid text.")?;
    let name = safe_file_name(name).map_err(|e| e.to_string())?;
    let lower = name.to_lowercase();

    let mut warning = None;
    match kind {
        Kind::Mod => {
            if src.is_dir() || !lower.ends_with(".jar") {
                return Err(hint_for_wrong_tab(src).unwrap_or_else(|| "Mods must be .jar files.".into()));
            }
            match content::inspect_jar(src, false).map_err(|e| e.to_string())? {
                JarKind::Fabric(meta) => {
                    if let Some(pred) = &meta.minecraft {
                        if versions::satisfies(pred, mc) == Some(false) {
                            warning = Some(format!(
                                "Built for Minecraft {}, not {mc}. It may not load.",
                                versions::describe(pred).unwrap_or_default()
                            ));
                        }
                    }
                }
                JarKind::OtherLoader => {
                    return Err("This is a Forge/NeoForge mod. Strata runs Fabric, so look for the Fabric build.".into())
                }
                JarKind::NotAMod => return Err("This jar has no fabric.mod.json, so it isn't a Fabric mod.".into()),
            }
        }
        Kind::Resourcepack => {
            if !src.is_dir() && !lower.ends_with(".zip") {
                return Err("Resource packs must be .zip files or folders.".into());
            }
            if !content::is_resource_pack(src) {
                return Err(hint_for_wrong_tab(src).unwrap_or_else(|| "No pack.mcmeta inside, so this isn't a resource pack.".into()));
            }
        }
        Kind::Shader => {
            if !src.is_dir() && !lower.ends_with(".zip") {
                return Err("Shader packs must be .zip files or folders.".into());
            }
            if !content::is_shader_pack(src) {
                return Err(hint_for_wrong_tab(src).unwrap_or_else(|| "No shaders/ folder inside, so this isn't a shader pack.".into()));
            }
        }
    }

    let mut dest = dir.join(&name);
    if dest.exists() || dir.join(format!("{name}.disabled")).exists() {
        if !src.is_dir() && same_file(src, &existing(dir, &name)) {
            return Err("Already installed.".into());
        }
        if kind == Kind::Mod {
            // Same file name, different contents: treat as an update.
            for p in [dir.join(&name), dir.join(format!("{name}.disabled"))] {
                let _ = fs::remove_file(p);
            }
        } else {
            dest = unique_name(dir, &name);
        }
    }

    if src.is_dir() {
        copy_dir(src, &dest).map_err(|e| format!("Couldn't copy the folder: {e}"))?;
    } else {
        fs::copy(src, &dest).map_err(|e| format!("Couldn't copy the file: {e}"))?;
    }

    let replaced = if kind == Kind::Mod { replace_same_mod(paths, id, &dest).map_err(|e| e.to_string())? } else { None };
    let final_name = dest.file_name().unwrap().to_string_lossy().to_string();

    let mut lock = Lockfile::load(paths, id).map_err(|e| e.to_string())?;
    if let Some(old) = &replaced {
        lock.remove_file(kind, old);
    }
    lock.upsert(LockEntry {
        kind,
        file: final_name.clone(),
        source: Source::Local,
        project_id: None,
        version_id: None,
        title: None,
        sha512: if dest.is_file() { sha512_file(&dest).ok() } else { None },
    });
    lock.save(paths, id).map_err(|e| e.to_string())?;

    Ok(Added { file: final_name, replaced, warning })
}

/// Tell people when they dropped something on the wrong tab.
fn hint_for_wrong_tab(src: &Path) -> Option<String> {
    let lower = src.to_string_lossy().to_lowercase();
    if lower.ends_with(".jar") {
        return Some("That's a mod. Drop it on the Mods tab.".into());
    }
    if content::is_shader_pack(src) {
        return Some("That's a shader pack. Drop it on the Shaders tab.".into());
    }
    if content::is_resource_pack(src) {
        return Some("That's a resource pack. Drop it on the Resource Packs tab.".into());
    }
    if lower.ends_with(".mrpack") {
        return Some("Modpacks (.mrpack) aren't supported yet.".into());
    }
    None
}

fn existing(dir: &Path, name: &str) -> PathBuf {
    let on = dir.join(name);
    if on.exists() { on } else { dir.join(format!("{name}.disabled")) }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(ma), Ok(mb)) if ma.len() == mb.len() => matches!((sha512_file(a), sha512_file(b)), (Ok(x), Ok(y)) if x == y),
        _ => false,
    }
}

fn unique_name(dir: &Path, name: &str) -> PathBuf {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (name.to_string(), String::new()),
    };
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("infinite iterator")
}

fn copy_dir(src: &Path, dest: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub fn sha512_file(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha512::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{self, Compat};
    use crate::paths::DEFAULT_INSTANCE;
    use std::io::Write;

    fn zip_with(path: &Path, files: &[(&str, &str)]) {
        let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, body) in files {
            w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }

    fn fabric_jar(path: &Path, id: &str, version: &str, mc: &str) {
        let json = format!(r#"{{"schemaVersion":1,"id":"{id}","version":"{version}","name":"{id}","depends":{{"minecraft":"{mc}"}}}}"#);
        zip_with(path, &[("fabric.mod.json", &json)]);
    }

    fn setup() -> (PathBuf, Paths) {
        let root = std::env::temp_dir().join(format!("strata-test-{}", std::process::id() as u64 * 1000 + rand_suffix()));
        let _ = fs::remove_dir_all(&root);
        let paths = Paths::at(root.join("data"));
        instance::bootstrap(&paths).unwrap();
        let drops = root.join("drops");
        fs::create_dir_all(&drops).unwrap();
        (drops, paths)
    }

    fn rand_suffix() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos() as u64 % 1000
    }

    #[test]
    fn drops_are_validated_and_updates_replace_old_jars() {
        let (drops, paths) = setup();
        let s = |p: &Path| p.display().to_string();

        let good = drops.join("sodium-0.8.0.jar");
        fabric_jar(&good, "sodium", "0.8.0", ">=26.3");
        let old_mc = drops.join("oldmod.jar");
        fabric_jar(&old_mc, "oldmod", "1.0", "~1.21.11");
        let forge = drops.join("forgemod.jar");
        zip_with(&forge, &[("META-INF/neoforge.mods.toml", "modLoader=\"javafml\"")]);
        let pack = drops.join("Faithful.zip");
        zip_with(&pack, &[("pack.mcmeta", r#"{"pack":{"description":"§aClean 32x","pack_format":97}}"#)]);

        let r = import(&paths, Kind::Mod, &[s(&good), s(&old_mc), s(&forge), s(&pack)]).unwrap();
        assert_eq!(r.added.len(), 2, "{r:?}");
        assert!(r.added.iter().any(|a| a.file == "oldmod.jar" && a.warning.is_some()));
        assert!(r.rejected.iter().any(|x| x.file == "forgemod.jar" && x.reason.contains("Forge")));
        assert!(r.rejected.iter().any(|x| x.file == "Faithful.zip" && x.reason.contains("Resource Packs")));

        // Dropping the same jar again is a no-op.
        let again = import(&paths, Kind::Mod, &[s(&good)]).unwrap();
        assert_eq!(again.rejected[0].reason, "Already installed.");

        // A newer sodium with a different file name replaces the old one.
        let newer = drops.join("sodium-0.8.1.jar");
        fabric_jar(&newer, "sodium", "0.8.1", ">=26.3");
        let up = import(&paths, Kind::Mod, &[s(&newer)]).unwrap();
        assert_eq!(up.added[0].replaced.as_deref(), Some("sodium-0.8.0.jar"));

        let mods = content::list(&paths, DEFAULT_INSTANCE, Kind::Mod, "26.3").unwrap();
        let files: Vec<_> = mods.iter().map(|m| m.file.as_str()).collect();
        assert_eq!(files, vec!["oldmod.jar", "sodium-0.8.1.jar"]);
        assert_eq!(mods[0].compatibility, Compat::Incompatible);
        assert_eq!(mods[1].compatibility, Compat::Compatible);

        // Disable / enable round trip.
        content::set_mod_enabled(&paths, DEFAULT_INSTANCE, "oldmod.jar", false).unwrap();
        let mods = content::list(&paths, DEFAULT_INSTANCE, Kind::Mod, "26.3").unwrap();
        assert!(!mods.iter().find(|m| m.file == "oldmod.jar").unwrap().enabled);
        content::set_mod_enabled(&paths, DEFAULT_INSTANCE, "oldmod.jar", true).unwrap();

        // Packs: import, enable, order is written to options.txt lowest-first.
        let pack2 = drops.join("Bare Bones.zip");
        zip_with(&pack2, &[("pack.mcmeta", r#"{"pack":{"description":{"text":"Simple"},"pack_format":97}}"#)]);
        let r = import(&paths, Kind::Resourcepack, &[s(&pack), s(&pack2)]).unwrap();
        assert_eq!(r.added.len(), 2);
        content::set_pack_order(&paths, DEFAULT_INSTANCE, &["Faithful.zip".into(), "Bare Bones.zip".into()]).unwrap();
        let options = fs::read_to_string(paths.options_file(DEFAULT_INSTANCE)).unwrap();
        assert!(options.contains(r#"resourcePacks:["vanilla","file/Bare Bones.zip","file/Faithful.zip"]"#), "{options}");
        let packs = content::list(&paths, DEFAULT_INSTANCE, Kind::Resourcepack, "26.3").unwrap();
        assert_eq!(packs[0].file, "Faithful.zip");
        assert_eq!(packs[0].description.as_deref(), Some("Clean 32x"));
        assert_eq!(packs[0].priority, Some(0));

        // Shaders: a resource pack dropped on the shader tab gets a hint.
        let r = import(&paths, Kind::Shader, &[s(&pack)]).unwrap();
        assert!(r.rejected[0].reason.contains("resource pack"));
        let shader = drops.join("Complementary.zip");
        zip_with(&shader, &[("shaders/composite.fsh", "void main(){}")]);
        assert_eq!(import(&paths, Kind::Shader, &[s(&shader)]).unwrap().added.len(), 1);
        content::set_active_shader(&paths, DEFAULT_INSTANCE, Some("Complementary.zip")).unwrap();
        assert_eq!(content::active_shader(&paths, DEFAULT_INSTANCE).as_deref(), Some("Complementary.zip"));
    }
}
