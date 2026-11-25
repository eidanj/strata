//! Reading what's in an instance: mods, resource packs and shader packs.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::instance::{Lockfile, Source};
use crate::paths::Paths;
use crate::versions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Mod,
    Resourcepack,
    Shader,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Mod, Kind::Resourcepack, Kind::Shader];

    pub fn folder(self) -> &'static str {
        match self {
            Kind::Mod => "mods",
            Kind::Resourcepack => "resourcepacks",
            Kind::Shader => "shaderpacks",
        }
    }

    /// Modrinth's `project_type` value.
    pub fn project_type(self) -> &'static str {
        match self {
            Kind::Mod => "mod",
            Kind::Resourcepack => "resourcepack",
            Kind::Shader => "shader",
        }
    }

    /// Modrinth loader tags to filter versions by.
    pub fn modrinth_loaders(self, instance_loader: &str) -> Vec<String> {
        match self {
            Kind::Mod => vec![instance_loader.to_string()],
            Kind::Resourcepack => vec!["minecraft".into()],
            Kind::Shader => vec!["iris".into()],
        }
    }
}

const DISABLED: &str = ".disabled";
const MAX_ICON_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Compat {
    Compatible,
    Incompatible,
    Unknown,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentItem {
    pub kind: Kind,
    /// Canonical name (no `.disabled`). Use this to refer to the item.
    pub file: String,
    pub enabled: bool,
    pub size: u64,
    pub is_directory: bool,
    pub source: Source,
    pub project_id: Option<String>,
    pub title: String,
    pub version: Option<String>,
    pub description: Option<String>,
    /// `data:` URL, ready for an `<img>`.
    pub icon: Option<String>,
    pub mod_id: Option<String>,
    /// Declared Minecraft range, e.g. `>=26.3`.
    pub minecraft: Option<String>,
    pub compatibility: Compat,
    /// Resource packs only: 0 = highest priority.
    pub priority: Option<usize>,
}

// ---------------------------------------------------------------------------
// Fabric mod metadata
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
pub struct ModMeta {
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub minecraft: Option<Value>,
    pub icon: Option<String>,
}

/// What kind of jar this is, judged from its contents.
pub enum JarKind {
    Fabric(ModMeta),
    /// Forge / NeoForge (has `META-INF/mods.toml` or `neoforge.mods.toml`).
    OtherLoader,
    NotAMod,
}

pub fn inspect_jar(path: &Path, want_icon: bool) -> Result<JarKind> {
    let file = fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let raw = match read_entry(&mut zip, "fabric.mod.json", 1024 * 1024)? {
        Some(bytes) => bytes,
        None => {
            // zip 9 yields each name as a Result.
            let other = zip.file_names().filter_map(|n| n.ok()).any(|n| {
                n == "META-INF/mods.toml" || n == "META-INF/neoforge.mods.toml"
            });
            return Ok(if other { JarKind::OtherLoader } else { JarKind::NotAMod });
        }
    };
    // Some mods ship control characters or a BOM; be forgiving.
    let text = String::from_utf8_lossy(&raw);
    let text = text.trim_start_matches('\u{feff}');
    let json: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Ok(JarKind::Fabric(ModMeta::default())),
    };
    let mut meta = ModMeta {
        id: json.get("id").and_then(Value::as_str).map(str::to_string),
        name: json.get("name").and_then(Value::as_str).map(str::to_string),
        version: json.get("version").and_then(Value::as_str).map(str::to_string),
        description: json.get("description").and_then(Value::as_str).map(str::to_string),
        minecraft: json.get("depends").and_then(|d| d.get("minecraft")).cloned(),
        icon: None,
    };
    if want_icon {
        if let Some(icon_path) = icon_path(json.get("icon")) {
            if let Some(bytes) = read_entry(&mut zip, &icon_path, MAX_ICON_BYTES)? {
                meta.icon = Some(png_data_url(&bytes));
            }
        }
    }
    Ok(JarKind::Fabric(meta))
}

/// `icon` is either a path or a `{ "16": "...", "64": "..." }` map; pick the
/// largest size up to 128.
fn icon_path(icon: Option<&Value>) -> Option<String> {
    match icon? {
        Value::String(s) => Some(s.clone()),
        Value::Object(map) => {
            let mut sizes: Vec<(u32, &str)> = map
                .iter()
                .filter_map(|(k, v)| Some((k.parse().ok()?, v.as_str()?)))
                .collect();
            sizes.sort_by_key(|(s, _)| *s);
            sizes.iter().rev().find(|(s, _)| *s <= 128).or(sizes.first()).map(|(_, p)| p.to_string())
        }
        _ => None,
    }
}

fn read_entry<R: Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
    max: u64,
) -> Result<Option<Vec<u8>>> {
    let name = name.trim_start_matches('/');
    let mut entry = match zip.by_name(name) {
        Ok(e) => e,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if entry.size() > max {
        return Ok(None);
    }
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf)?;
    Ok(Some(buf))
}

fn png_data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

// ---------------------------------------------------------------------------
// Resource / shader pack metadata
// ---------------------------------------------------------------------------

struct PackMeta {
    description: Option<String>,
    icon: Option<String>,
}

fn read_pack_meta(path: &Path) -> PackMeta {
    let (mcmeta, png) = if path.is_dir() {
        (fs::read(path.join("pack.mcmeta")).ok(), small_file(&path.join("pack.png")))
    } else {
        match fs::File::open(path).ok().and_then(|f| zip::ZipArchive::new(f).ok()) {
            Some(mut zip) => (
                read_entry(&mut zip, "pack.mcmeta", 256 * 1024).ok().flatten(),
                read_entry(&mut zip, "pack.png", MAX_ICON_BYTES).ok().flatten(),
            ),
            None => (None, None),
        }
    };
    let description = mcmeta
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.get("pack").and_then(|p| p.get("description")).map(flatten_text))
        .map(|s| strip_formatting(&s))
        .filter(|s| !s.trim().is_empty());
    PackMeta { description, icon: png.map(|b| png_data_url(&b)) }
}

fn small_file(path: &Path) -> Option<Vec<u8>> {
    let meta = fs::metadata(path).ok()?;
    (meta.len() <= MAX_ICON_BYTES).then(|| fs::read(path).ok()).flatten()
}

/// Pack descriptions can be a string, a text component, or an array of them.
fn flatten_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(flatten_text).collect(),
        Value::Object(map) => {
            let mut out = map.get("text").and_then(Value::as_str).unwrap_or_default().to_string();
            if let Some(extra) = map.get("extra") {
                out.push_str(&flatten_text(extra));
            }
            out
        }
        _ => String::new(),
    }
}

/// Remove `§x` colour codes.
fn strip_formatting(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

pub fn is_resource_pack(path: &Path) -> bool {
    if path.is_dir() {
        return path.join("pack.mcmeta").is_file();
    }
    zip_has(path, |n| n == "pack.mcmeta")
}

pub fn is_shader_pack(path: &Path) -> bool {
    if path.is_dir() {
        return path.join("shaders").is_dir();
    }
    zip_has(path, |n| n.starts_with("shaders/"))
}

fn zip_has(path: &Path, pred: impl Fn(&str) -> bool) -> bool {
    fs::File::open(path)
        .ok()
        .and_then(|f| zip::ZipArchive::new(f).ok())
        .map(|z| z.file_names().filter_map(|n| n.ok()).any(|n| pred(&n)))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// options.txt (resource pack order) and iris.properties (active shader)
// ---------------------------------------------------------------------------

pub struct OptionsTxt {
    path: PathBuf,
    lines: Vec<String>,
}

impl OptionsTxt {
    pub fn load(path: PathBuf) -> Result<Self> {
        let lines = match fs::read_to_string(&path) {
            Ok(text) => text.lines().map(str::to_string).collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self { path, lines })
    }

    pub fn list(&self, key: &str) -> Option<Vec<String>> {
        let prefix = format!("{key}:");
        let line = self.lines.iter().find(|l| l.starts_with(&prefix))?;
        serde_json::from_str(&line[prefix.len()..]).ok()
    }

    pub fn set_list(&mut self, key: &str, values: &[String]) -> Result<()> {
        let line = format!("{key}:{}", serde_json::to_string(values)?);
        let prefix = format!("{key}:");
        match self.lines.iter_mut().find(|l| l.starts_with(&prefix)) {
            Some(existing) => *existing = line,
            None => self.lines.push(line),
        }
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("txt.tmp");
        fs::write(&tmp, self.lines.join("\n") + "\n")?;
        fs::rename(tmp, &self.path)?;
        Ok(())
    }
}

/// Selected packs, highest priority first, as canonical file names.
/// (options.txt stores them lowest-first as `file/<name>`.)
fn selected_packs(options: &OptionsTxt) -> Vec<String> {
    options
        .list("resourcePacks")
        .unwrap_or_default()
        .iter()
        .rev()
        .filter_map(|p| p.strip_prefix("file/").map(str::to_string))
        .collect()
}

/// Write the enabled pack order (highest priority first). Built-in entries
/// such as `vanilla` and mod resources keep their place underneath.
pub fn set_pack_order(paths: &Paths, id: &str, enabled_top_first: &[String]) -> Result<()> {
    let mut options = OptionsTxt::load(paths.options_file(id))?;
    let existing = options.list("resourcePacks").unwrap_or_else(|| vec!["vanilla".into()]);
    let mut list: Vec<String> = existing.into_iter().filter(|p| !p.starts_with("file/")).collect();
    if list.is_empty() {
        list.push("vanilla".into());
    }
    list.extend(enabled_top_first.iter().rev().map(|f| format!("file/{f}")));
    options.set_list("resourcePacks", &list)?;
    options.save()
}

/// Iris stores its choice in `config/iris.properties`.
pub fn active_shader(paths: &Paths, id: &str) -> Option<String> {
    let text = fs::read_to_string(paths.iris_config(id)).ok()?;
    let mut pack = None;
    let mut enabled = true;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("shaderPack=") {
            pack = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("enableShaders=") {
            enabled = v.trim() == "true";
        }
    }
    pack.filter(|p| enabled && !p.is_empty())
}

pub fn set_active_shader(paths: &Paths, id: &str, pack: Option<&str>) -> Result<()> {
    let path = paths.iris_config(id);
    let text = fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| !l.starts_with("shaderPack=") && !l.starts_with("enableShaders="))
        .map(str::to_string)
        .collect();
    lines.push(format!("shaderPack={}", pack.unwrap_or("")));
    lines.push(format!("enableShaders={}", pack.is_some()));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Listing
// ---------------------------------------------------------------------------

pub fn canonical_name(file_name: &str) -> &str {
    file_name.strip_suffix(DISABLED).unwrap_or(file_name)
}

pub fn list(paths: &Paths, id: &str, kind: Kind, mc_version: &str) -> Result<Vec<ContentItem>> {
    let dir = paths.content_dir(id, kind);
    fs::create_dir_all(&dir)?;

    let mut lock = Lockfile::load(paths, id)?;
    if lock.prune(paths, id) {
        lock.save(paths, id)?;
    }
    let packs_on = match kind {
        Kind::Resourcepack => selected_packs(&OptionsTxt::load(paths.options_file(id))?),
        _ => vec![],
    };
    let shader_on = match kind {
        Kind::Shader => active_shader(paths, id),
        _ => None,
    };

    let mut items = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let raw_name = entry.file_name().to_string_lossy().to_string();
        if raw_name.starts_with('.') {
            continue; // .strata-tmp and hidden files
        }
        let path = entry.path();
        let meta = entry.metadata()?;
        let is_dir = meta.is_dir();
        let file = canonical_name(&raw_name).to_string();
        let disabled = raw_name.ends_with(DISABLED);

        let wanted = match kind {
            Kind::Mod => !is_dir && file.ends_with(".jar"),
            Kind::Resourcepack => is_dir || file.ends_with(".zip"),
            Kind::Shader => is_dir || file.ends_with(".zip"),
        };
        if !wanted {
            continue;
        }

        let locked = lock.by_file(kind, &file);
        let mut item = ContentItem {
            kind,
            file: file.clone(),
            enabled: !disabled,
            size: if is_dir { 0 } else { meta.len() },
            is_directory: is_dir,
            source: locked.map(|l| l.source).unwrap_or(Source::Local),
            project_id: locked.and_then(|l| l.project_id.clone()),
            title: locked
                .and_then(|l| l.title.clone())
                .unwrap_or_else(|| display_name(&file)),
            version: None,
            description: None,
            icon: None,
            mod_id: None,
            minecraft: None,
            compatibility: Compat::Unknown,
            priority: None,
        };

        match kind {
            Kind::Mod => {
                if let Ok(JarKind::Fabric(m)) = inspect_jar(&path, true) {
                    if let Some(name) = m.name {
                        item.title = name;
                    }
                    item.version = m.version;
                    item.description = m.description;
                    item.icon = m.icon;
                    item.mod_id = m.id;
                    if let Some(pred) = &m.minecraft {
                        item.minecraft = versions::describe(pred);
                        item.compatibility = match versions::satisfies(pred, mc_version) {
                            Some(true) => Compat::Compatible,
                            Some(false) => Compat::Incompatible,
                            None => Compat::Unknown,
                        };
                    } else {
                        // No minecraft dependency declared = loads on any version.
                        item.compatibility = Compat::Compatible;
                    }
                }
            }
            Kind::Resourcepack => {
                let pm = read_pack_meta(&path);
                item.description = pm.description;
                item.icon = pm.icon;
                item.priority = packs_on.iter().position(|p| p == &file);
                item.enabled = item.priority.is_some();
            }
            Kind::Shader => {
                item.enabled = shader_on.as_deref() == Some(file.as_str());
            }
        }
        items.push(item);
    }

    match kind {
        Kind::Resourcepack => items.sort_by(|a, b| match (a.priority, b.priority) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
        }),
        _ => items.sort_by_key(|i| i.title.to_lowercase()),
    }
    Ok(items)
}

fn display_name(file: &str) -> String {
    let stem = file.strip_suffix(".jar").or_else(|| file.strip_suffix(".zip")).unwrap_or(file);
    stem.replace(['_', '-'], " ")
}

/// Mods: rename `x.jar` <-> `x.jar.disabled`. Fabric ignores the latter.
pub fn set_mod_enabled(paths: &Paths, id: &str, file: &str, enabled: bool) -> Result<()> {
    let dir = paths.content_dir(id, Kind::Mod);
    let on = dir.join(file);
    let off = dir.join(format!("{file}{DISABLED}"));
    match (enabled, on.exists(), off.exists()) {
        (true, false, true) => fs::rename(off, on)?,
        (false, true, false) => fs::rename(on, off)?,
        (_, false, false) => return Err(Error::msg(format!("{file} is no longer in the mods folder"))),
        _ => {}
    }
    Ok(())
}

pub fn remove(paths: &Paths, id: &str, kind: Kind, file: &str) -> Result<()> {
    let dir = paths.content_dir(id, kind);
    for candidate in [dir.join(file), dir.join(format!("{file}{DISABLED}"))] {
        if candidate.is_dir() {
            fs::remove_dir_all(&candidate)?;
        } else if candidate.exists() {
            fs::remove_file(&candidate)?;
        }
    }
    let mut lock = Lockfile::load(paths, id)?;
    lock.remove_file(kind, file);
    lock.save(paths, id)?;
    match kind {
        Kind::Resourcepack => {
            let options = OptionsTxt::load(paths.options_file(id))?;
            let order: Vec<String> = selected_packs(&options).into_iter().filter(|p| p != file).collect();
            set_pack_order(paths, id, &order)?;
        }
        Kind::Shader if active_shader(paths, id).as_deref() == Some(file) => {
            set_active_shader(paths, id, None)?;
        }
        _ => {}
    }
    Ok(())
}

/// Mod ids of every jar in the folder (enabled or not), with file names.
pub fn installed_mod_ids(paths: &Paths, id: &str) -> Vec<(String, String)> {
    let dir = paths.content_dir(id, Kind::Mod);
    let Ok(entries) = fs::read_dir(dir) else { return vec![] };
    entries
        .flatten()
        .filter_map(|e| {
            let raw = e.file_name().to_string_lossy().to_string();
            let file = canonical_name(&raw).to_string();
            if !file.ends_with(".jar") {
                return None;
            }
            match inspect_jar(&e.path(), false) {
                Ok(JarKind::Fabric(m)) => Some((m.id?, file)),
                _ => None,
            }
        })
        .collect()
}
