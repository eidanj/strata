//! Launching Minecraft with Fabric.
//!
//! 1. Mojang's version manifest -> the version JSON (cached, SHA-1 checked)
//! 2. Fabric's profile JSON from meta.fabricmc.net (main class + libraries)
//! 3. Download whatever is missing: client jar, libraries for this OS/CPU,
//!    asset index + objects, logging config. Parallel, SHA-1 verified.
//! 4. Find a Java that matches the version's `javaVersion` (25 for 26.x)
//! 5. Build the command line from the JSON's argument lists and run it
//!
//! Natives: since 1.19 they're ordinary libraries with a platform classifier;
//! LWJGL extracts them itself into `-Dorg.lwjgl.system.SharedLibraryExtractPath`,
//! so they only need to be on the classpath.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use futures_util::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;

use crate::auth::{self, Identity};
use crate::error::{Error, Result};
use crate::instance::{self, Instance};
use crate::paths::Paths;
use crate::AppState;

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const ASSET_BASE: &str = "https://resources.download.minecraft.net";
const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
const PARALLEL_DOWNLOADS: usize = 16;
const LOG_LINES_KEPT: usize = 1500;

pub const PROGRESS_EVENT: &str = "launch-progress";
pub const LOG_EVENT: &str = "game-log";
pub const EXIT_EVENT: &str = "game-exit";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProgress {
    /// account | metadata | downloading | java | starting | running
    pub stage: &'static str,
    pub label: String,
    pub done: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameExit {
    pub code: Option<i32>,
    pub crashed: bool,
    pub seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStatus {
    pub running: bool,
    pub launching: bool,
    pub log: Vec<String>,
}

pub struct GameHandle {
    kill: Option<oneshot::Sender<()>>,
}

// ------------------------------------------------------- JSON formats

#[derive(Deserialize)]
struct Manifest {
    versions: Vec<ManifestEntry>,
}

#[derive(Deserialize, Clone)]
struct ManifestEntry {
    id: String,
    url: String,
    sha1: String,
}

#[derive(Deserialize, Default, Clone)]
pub struct Arguments {
    #[serde(default)]
    game: Vec<Value>,
    #[serde(default)]
    jvm: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    pub main_class: String,
    #[serde(default)]
    pub arguments: Arguments,
    pub asset_index: AssetIndexRef,
    pub downloads: Downloads,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub java_version: Option<JavaVersion>,
    #[serde(default)]
    pub logging: Option<Logging>,
    #[serde(rename = "type", default)]
    pub kind: String,
}

#[derive(Deserialize)]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    pub sha1: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Deserialize)]
pub struct Downloads {
    pub client: Artifact,
}

#[derive(Deserialize, Clone)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    pub url: String,
    pub sha1: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Deserialize, Clone)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Option<Vec<Rule>>,
    /// Fabric-style libraries: maven base URL + optional checksum.
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Deserialize, Clone)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub major_version: u32,
}

#[derive(Deserialize)]
pub struct Logging {
    #[serde(default)]
    pub client: Option<LoggingClient>,
}

#[derive(Deserialize)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LogFile,
}

#[derive(Deserialize)]
pub struct LogFile {
    pub id: String,
    pub url: String,
    pub sha1: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricProfile {
    pub id: String,
    pub main_class: String,
    #[serde(default)]
    pub arguments: Arguments,
    #[serde(default)]
    pub libraries: Vec<Library>,
}

// ------------------------------------------------------------- platform

#[derive(Debug, Clone, Copy)]
pub struct Platform {
    /// Mojang's names: osx | windows | linux
    pub os: &'static str,
    /// x86_64 | arm64 | x86 (32-bit)
    pub arch: &'static str,
}

impl Platform {
    pub fn current() -> Self {
        Self {
            os: match std::env::consts::OS {
                "macos" => "osx",
                "windows" => "windows",
                _ => "linux",
            },
            arch: match std::env::consts::ARCH {
                "aarch64" => "arm64",
                "x86" => "x86",
                _ => "x86_64",
            },
        }
    }

    fn classpath_separator(&self) -> &'static str {
        if self.os == "windows" { ";" } else { ":" }
    }
}

/// Mojang rule semantics: no rules = allowed; otherwise the last matching
/// rule decides, and nothing matching means disallowed.
pub fn rules_allow(rules: &[Rule], p: &Platform, features: &HashMap<&str, bool>) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        let os_ok = rule.os.as_ref().is_none_or(|os| {
            os.name.as_deref().is_none_or(|n| n == p.os) && os.arch.as_deref().is_none_or(|a| a == p.arch)
        });
        let features_ok = rule
            .features
            .as_ref()
            .is_none_or(|f| f.iter().all(|(k, want)| features.get(k.as_str()).copied().unwrap_or(false) == *want));
        if os_ok && features_ok {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

/// OS rules don't separate CPU types (natives-macos vs natives-macos-arm64
/// are both "osx"), so pick by the classifier too.
pub fn classifier_fits(name: &str, p: &Platform) -> bool {
    let Some(classifier) = name.split(':').nth(3) else { return true };
    let c = classifier.to_ascii_lowercase();
    let platform_specific = ["natives", "linux", "osx", "macos", "windows"].iter().any(|t| c.contains(t));
    if !platform_specific {
        return true;
    }
    let arm = c.contains("arm64") || c.contains("aarch_64") || c.contains("aarch64");
    let x86_32 = c.ends_with("-x86") || c.contains("x86_32");
    match p.arch {
        "arm64" => arm,
        "x86" => x86_32,
        _ => !arm && !x86_32,
    }
}

/// `group:artifact:version[:classifier][@ext]` -> relative Maven path.
pub fn maven_path(name: &str) -> Option<String> {
    let (coords, ext) = name.split_once('@').unwrap_or((name, "jar"));
    let parts: Vec<&str> = coords.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let (group, artifact, version) = (parts[0], parts[1], parts[2]);
    let file = match parts.get(3) {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    Some(format!("{}/{artifact}/{version}/{file}", group.replace('.', "/")))
}

/// `group:artifact[:classifier]`, used to avoid two copies of one library.
fn library_key(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    match parts.len() {
        0..=2 => name.to_string(),
        3 => format!("{}:{}", parts[0], parts[1]),
        _ => format!("{}:{}:{}", parts[0], parts[1], parts[3]),
    }
}

/// Flatten an argument list, keeping only entries whose rules match.
pub fn collect_args(values: &[Value], p: &Platform, features: &HashMap<&str, bool>) -> Vec<String> {
    let mut out = Vec::new();
    for v in values {
        match v {
            Value::String(s) => out.push(s.clone()),
            Value::Object(obj) => {
                let rules: Vec<Rule> = obj
                    .get("rules")
                    .and_then(|r| serde_json::from_value(r.clone()).ok())
                    .unwrap_or_default();
                if !rules_allow(&rules, p, features) {
                    continue;
                }
                match obj.get("value") {
                    Some(Value::String(s)) => out.push(s.clone()),
                    Some(Value::Array(items)) => out.extend(items.iter().filter_map(|i| i.as_str().map(str::to_string))),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

/// Replace `${name}` placeholders. Unknown ones are left as they are.
pub fn substitute(arg: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(arg.len());
    let mut rest = arg;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        match rest[start..].find('}') {
            Some(end_rel) => {
                let key = &rest[start + 2..start + end_rel];
                match vars.get(key) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(&rest[start..=start + end_rel]),
                }
                rest = &rest[start + end_rel + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

// ------------------------------------------------------------ downloads

#[derive(Debug, Clone)]
struct Download {
    url: String,
    path: PathBuf,
    sha1: Option<String>,
    size: u64,
}

impl Download {
    /// Existence + size is enough to skip a file on launch; hashing every
    /// asset on each start would cost seconds. Downloads are always hashed.
    fn needed(&self) -> bool {
        match std::fs::metadata(&self.path) {
            Ok(m) => self.size > 0 && m.len() != self.size,
            Err(_) => true,
        }
    }
}

async fn fetch_file(http: &reqwest::Client, d: &Download) -> Result<()> {
    if let Some(parent) = d.path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = d.path.with_extension("strata-part");
    let mut last_err = None;
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(400 * attempt)).await;
        }
        match try_fetch(http, d, &tmp).await {
            Ok(()) => {
                tokio::fs::rename(&tmp, &d.path).await?;
                return Ok(());
            }
            Err(e) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| Error::msg("download failed")))
}

async fn try_fetch(http: &reqwest::Client, d: &Download, tmp: &Path) -> Result<()> {
    let resp = http.get(&d.url).send().await?.error_for_status()?;
    let mut out = tokio::fs::File::create(tmp).await?;
    let mut hasher = Sha1::new();
    let mut body = resp.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        out.write_all(&chunk).await?;
    }
    out.flush().await?;
    if let Some(expected) = &d.sha1 {
        let got = hex::encode(hasher.finalize());
        if !got.eq_ignore_ascii_case(expected) {
            return Err(Error::msg(format!("{} failed its checksum", d.url)));
        }
    }
    Ok(())
}

async fn download_all(app: &AppHandle, http: &reqwest::Client, items: Vec<Download>) -> Result<()> {
    let todo: Vec<Download> = items.into_iter().filter(Download::needed).collect();
    if todo.is_empty() {
        return Ok(());
    }
    let total_bytes: u64 = todo.iter().map(|d| d.size).sum();
    let total = todo.len() as u64;
    let mut done = 0u64;
    let mut bytes = 0u64;
    let mut last = Instant::now() - Duration::from_secs(1);

    let mut results = stream::iter(todo)
        .map(|d| async move {
            let size = d.size;
            fetch_file(http, &d).await.map(|_| size)
        })
        .buffer_unordered(PARALLEL_DOWNLOADS);

    while let Some(result) = results.next().await {
        bytes += result?;
        done += 1;
        if last.elapsed() > Duration::from_millis(100) || done == total {
            last = Instant::now();
            let label = if total_bytes > 0 {
                format!("Downloading game files ({:.0} of {:.0} MB)", bytes as f64 / 1e6, total_bytes as f64 / 1e6)
            } else {
                format!("Downloading game files ({done} of {total})")
            };
            progress(app, "downloading", label, done, total);
        }
    }
    Ok(())
}

async fn fetch_json<T: serde::de::DeserializeOwned>(http: &reqwest::Client, url: &str, cache: &Path) -> Result<T> {
    let fresh = async {
        let text = http.get(url).send().await?.error_for_status()?.text().await?;
        Ok::<String, Error>(text)
    }
    .await;
    let text = match fresh {
        Ok(text) => {
            if let Some(parent) = cache.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(cache, &text).await?;
            text
        }
        // Offline? Use the last copy we saved.
        Err(e) => tokio::fs::read_to_string(cache).await.map_err(|_| e)?,
    };
    Ok(serde_json::from_str(&text)?)
}

fn progress(app: &AppHandle, stage: &'static str, label: impl Into<String>, done: u64, total: u64) {
    let _ = app.emit(PROGRESS_EVENT, LaunchProgress { stage, label: label.into(), done, total });
}

// -------------------------------------------------------- version files

async fn version_json(app: &AppHandle, state: &AppState, mc: &str) -> Result<VersionJson> {
    let dir = state.paths.versions_dir().join(mc);
    let path = dir.join(format!("{mc}.json"));
    let manifest: Result<Manifest> = fetch_json(&state.http, MANIFEST_URL, &state.paths.meta_dir().join("version_manifest_v2.json")).await;
    if let Ok(manifest) = manifest {
        let entry = manifest
            .versions
            .iter()
            .find(|v| v.id == mc)
            .cloned()
            .ok_or_else(|| Error::msg(format!("Mojang doesn't list Minecraft {mc}.")))?;
        let d = Download { url: entry.url, path: path.clone(), sha1: Some(entry.sha1.clone()), size: 0 };
        let current = std::fs::read(&path).map(|b| hex::encode(Sha1::digest(&b))).unwrap_or_default();
        if !current.eq_ignore_ascii_case(&entry.sha1) {
            progress(app, "metadata", format!("Getting Minecraft {mc}"), 0, 0);
            fetch_file(&state.http, &d).await?;
        }
    } else if !path.exists() {
        return Err(Error::msg("Couldn't reach Mojang to download the game. Check your connection."));
    }
    Ok(serde_json::from_slice(&tokio::fs::read(&path).await?)?)
}

async fn fabric_profile(state: &AppState, inst: &Instance) -> Result<FabricProfile> {
    let url = format!(
        "{FABRIC_META}/versions/loader/{}/{}/profile/json",
        inst.minecraft_version, inst.loader_version
    );
    let cache = state
        .paths
        .loaders_dir()
        .join("fabric")
        .join(format!("fabric-loader-{}-{}.json", inst.loader_version, inst.minecraft_version));
    fetch_json(&state.http, &url, &cache).await
}

/// Everything the game needs on disk, plus the ordered classpath.
fn plan_files(paths: &Paths, version: &VersionJson, fabric: &FabricProfile, p: &Platform) -> (Vec<Download>, Vec<PathBuf>) {
    let libs = paths.libraries_dir();
    let no_features = HashMap::new();
    let mut downloads = Vec::new();
    let mut classpath = Vec::new();
    let mut seen = HashSet::new();

    // Fabric first: its copies win when both list a library (e.g. ASM).
    for lib in &fabric.libraries {
        let Some(rel) = maven_path(&lib.name) else { continue };
        if !seen.insert(library_key(&lib.name)) {
            continue;
        }
        let base = lib.url.as_deref().unwrap_or("https://maven.fabricmc.net/");
        let path = libs.join(&rel);
        downloads.push(Download {
            url: format!("{}/{rel}", base.trim_end_matches('/')),
            path: path.clone(),
            sha1: lib.sha1.clone(),
            size: lib.size.unwrap_or(0),
        });
        classpath.push(path);
    }

    for lib in &version.libraries {
        if !rules_allow(lib.rules.as_deref().unwrap_or(&[]), p, &no_features) || !classifier_fits(&lib.name, p) {
            continue;
        }
        let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.clone()) else { continue };
        if !seen.insert(library_key(&lib.name)) {
            continue;
        }
        let rel = artifact.path.clone().or_else(|| maven_path(&lib.name)).unwrap_or_default();
        let path = libs.join(rel);
        downloads.push(Download { url: artifact.url, path: path.clone(), sha1: Some(artifact.sha1), size: artifact.size });
        classpath.push(path);
    }

    let client = paths.versions_dir().join(&version.id).join(format!("{}.jar", version.id));
    downloads.push(Download {
        url: version.downloads.client.url.clone(),
        path: client.clone(),
        sha1: Some(version.downloads.client.sha1.clone()),
        size: version.downloads.client.size,
    });
    classpath.push(client);

    if let Some(log) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        downloads.push(Download {
            url: log.file.url.clone(),
            path: paths.assets_dir().join("log_configs").join(&log.file.id),
            sha1: Some(log.file.sha1.clone()),
            size: log.file.size,
        });
    }
    (downloads, classpath)
}

async fn asset_downloads(state: &AppState, version: &VersionJson) -> Result<Vec<Download>> {
    let index_path = state.paths.assets_dir().join("indexes").join(format!("{}.json", version.asset_index.id));
    let index = Download {
        url: version.asset_index.url.clone(),
        path: index_path.clone(),
        sha1: Some(version.asset_index.sha1.clone()),
        size: version.asset_index.size,
    };
    let current = std::fs::read(&index_path).map(|b| hex::encode(Sha1::digest(&b))).unwrap_or_default();
    if !current.eq_ignore_ascii_case(&version.asset_index.sha1) {
        fetch_file(&state.http, &index).await?;
    }
    let parsed: Value = serde_json::from_slice(&tokio::fs::read(&index_path).await?)?;
    let objects = parsed.get("objects").and_then(Value::as_object).cloned().unwrap_or_default();
    let dir = state.paths.assets_dir().join("objects");
    let mut out = Vec::with_capacity(objects.len());
    let mut seen = HashSet::new();
    for obj in objects.values() {
        let Some(hash) = obj.get("hash").and_then(Value::as_str) else { continue };
        if hash.len() < 2 || !seen.insert(hash.to_string()) {
            continue;
        }
        let size = obj.get("size").and_then(Value::as_u64).unwrap_or(0);
        out.push(Download {
            url: format!("{ASSET_BASE}/{}/{hash}", &hash[..2]),
            path: dir.join(&hash[..2]).join(hash),
            sha1: Some(hash.to_string()),
            size,
        });
    }
    Ok(out)
}

// ------------------------------------------------------------------ java

/// Find a Java whose major version is at least `major`, preferring an exact
/// match. Looks at the Strata setting, JAVA_HOME, the OS's usual install
/// folders, then PATH.
pub async fn find_java(paths: &Paths, major: u32) -> Result<PathBuf> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(p) = instance::settings(paths).ok().and_then(|s| s.java_path) {
        candidates.push(PathBuf::from(p));
    }
    #[cfg(target_os = "macos")]
    if let Ok(out) = tokio::process::Command::new("/usr/libexec/java_home").args(["-v", &major.to_string()]).output().await {
        let home = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && !home.is_empty() {
            candidates.push(Path::new(&home).join("bin").join(exe));
        }
    }
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        candidates.push(Path::new(&home).join("bin").join(exe));
    }
    for root in java_roots() {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for e in entries.flatten() {
                let base = e.path();
                for bin in [base.join("bin").join(exe), base.join("Contents/Home/bin").join(exe)] {
                    if bin.exists() {
                        candidates.push(bin);
                    }
                }
            }
        }
    }
    candidates.push(PathBuf::from(exe));

    let mut found: Vec<(PathBuf, u32)> = Vec::new();
    for c in candidates {
        if let Some(v) = java_major(&c).await {
            if v == major {
                return Ok(prefer_windowless(c));
            }
            found.push((c, v));
        }
    }
    if let Some((c, _)) = found.into_iter().filter(|(_, v)| *v > major).min_by_key(|(_, v)| *v) {
        return Ok(prefer_windowless(c));
    }
    Err(Error::msg(format!(
        "Minecraft needs Java {major}, and Strata couldn't find it. Install a JDK {major} (adoptium.net), or set its path in strata.json as javaPath."
    )))
}

fn java_roots() -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        vec!["/Library/Java/JavaVirtualMachines".into()]
    } else if cfg!(windows) {
        let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
        ["Eclipse Adoptium", "Java", "Microsoft", "Zulu", "BellSoft", "Amazon Corretto"]
            .iter()
            .map(|v| Path::new(&pf).join(v))
            .collect()
    } else {
        vec!["/usr/lib/jvm".into(), "/opt/java".into()]
    }
}

async fn java_major(java: &Path) -> Option<u32> {
    let out = tokio::process::Command::new(java).arg("-version").output().await.ok()?;
    let text = String::from_utf8_lossy(&out.stderr);
    parse_java_major(&text)
}

/// `openjdk version "25.0.1" 2025-10-21` -> 25; `"1.8.0_402"` -> 8.
pub fn parse_java_major(text: &str) -> Option<u32> {
    let start = text.find("version \"")? + 9;
    let version = &text[start..];
    let version = &version[..version.find('"')?];
    let mut parts = version.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 { parts.next()?.parse().ok() } else { Some(first) }
}

/// On Windows, javaw.exe runs without a console window.
fn prefer_windowless(java: PathBuf) -> PathBuf {
    if cfg!(windows) {
        let w = java.with_file_name("javaw.exe");
        if w.exists() {
            return w;
        }
    }
    java
}

// ------------------------------------------------------- command line

pub struct CommandParts<'a> {
    pub version: &'a VersionJson,
    pub fabric: &'a FabricProfile,
    pub identity: &'a Identity,
    pub game_dir: &'a Path,
    pub assets_dir: &'a Path,
    pub libraries_dir: &'a Path,
    pub natives_dir: &'a Path,
    pub classpath: &'a [PathBuf],
    pub log_config: Option<PathBuf>,
    pub memory_mb: u32,
    pub profile_args: Vec<String>,
    pub extra_args: Vec<String>,
}

pub fn build_command(c: &CommandParts, p: &Platform) -> Vec<String> {
    let features: HashMap<&str, bool> = HashMap::new(); // demo, custom resolution, quick play: all off
    let sep = p.classpath_separator();
    let classpath = c.classpath.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(sep);

    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", c.identity.name.clone()),
        ("version_name", c.fabric.id.clone()),
        ("game_directory", c.game_dir.display().to_string()),
        ("assets_root", c.assets_dir.display().to_string()),
        ("assets_index_name", c.version.asset_index.id.clone()),
        ("auth_uuid", c.identity.uuid.clone()),
        ("auth_access_token", c.identity.access_token.clone()),
        ("auth_xuid", c.identity.xuid.clone()),
        ("clientid", "strata-launcher".to_string()),
        ("user_type", c.identity.user_type.to_string()),
        ("version_type", if c.version.kind.is_empty() { "release".into() } else { c.version.kind.clone() }),
        ("natives_directory", c.natives_dir.display().to_string()),
        ("library_directory", c.libraries_dir.display().to_string()),
        ("launcher_name", "strata".to_string()),
        ("launcher_version", env!("CARGO_PKG_VERSION").to_string()),
        ("classpath", classpath),
        ("classpath_separator", sep.to_string()),
    ]);

    let mut args = vec![format!("-Xms{}m", c.memory_mb), format!("-Xmx{}m", c.memory_mb)];
    args.extend(c.profile_args.iter().cloned());
    args.extend(c.extra_args.iter().cloned());
    args.extend(collect_args(&c.fabric.arguments.jvm, p, &features));
    args.extend(collect_args(&c.version.arguments.jvm, p, &features).iter().map(|a| substitute(a, &vars)));
    if let (Some(log), Some(path)) = (c.version.logging.as_ref().and_then(|l| l.client.as_ref()), &c.log_config) {
        let vars = HashMap::from([("path", path.display().to_string())]);
        args.push(substitute(&log.argument, &vars));
    }
    // Fabric's Knot launcher wraps the vanilla main class.
    let main = if c.fabric.main_class.is_empty() { &c.version.main_class } else { &c.fabric.main_class };
    args.push(main.clone());
    args.extend(collect_args(&c.version.arguments.game, p, &features).iter().map(|a| substitute(a, &vars)));
    args.extend(collect_args(&c.fabric.arguments.game, p, &features).iter().map(|a| substitute(a, &vars)));
    args
}

// --------------------------------------------------------------- launch

struct LaunchingFlag<'a>(&'a AppState);
impl Drop for LaunchingFlag<'_> {
    fn drop(&mut self) {
        self.0.launching.store(false, Ordering::SeqCst);
    }
}

pub async fn launch(app: AppHandle, state: &AppState) -> Result<()> {
    if state.game.lock().await.is_some() {
        return Err(Error::msg("The game is already running."));
    }
    if state.launching.swap(true, Ordering::SeqCst) {
        return Err(Error::msg("Already starting the game."));
    }
    let _flag = LaunchingFlag(state);
    let p = Platform::current();

    progress(&app, "account", "Checking your account", 0, 0);
    let identity = auth::identity(state).await?;

    let id = instance::active_id(&state.paths)?;
    let inst = instance::load(&state.paths, &id)?;
    progress(&app, "metadata", format!("Preparing Minecraft {}", inst.minecraft_version), 0, 0);
    let version = version_json(&app, state, &inst.minecraft_version).await?;
    let fabric = fabric_profile(state, &inst).await?;

    let (mut files, classpath) = plan_files(&state.paths, &version, &fabric, &p);
    files.extend(asset_downloads(state, &version).await?);
    download_all(&app, &state.http, files).await?;

    let major = version.java_version.as_ref().map(|j| j.major_version).unwrap_or(25);
    progress(&app, "java", format!("Finding Java {major}"), 0, 0);
    let java = find_java(&state.paths, major).await?;

    let natives = state.paths.natives_dir(&version.id);
    for sub in ["", "java", "jna", "lwjgl", "netty"] {
        tokio::fs::create_dir_all(natives.join(sub)).await?;
    }
    let game_dir = state.paths.game_dir(&id);
    tokio::fs::create_dir_all(&game_dir).await?;

    let profile = crate::profiles::list(&state.paths)?
        .into_iter()
        .find(|p| p.get("id").and_then(Value::as_str) == Some(inst.active_profile.as_str()));
    let memory_mb = profile.as_ref().and_then(|p| p.get("memoryMb")).and_then(Value::as_u64).map(|m| m as u32).unwrap_or(inst.memory_mb);
    let profile_args: Vec<String> = profile
        .as_ref()
        .and_then(|p| p.get("jvmArgs"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    let log_config = version
        .logging
        .as_ref()
        .and_then(|l| l.client.as_ref())
        .map(|l| state.paths.assets_dir().join("log_configs").join(&l.file.id));

    let args = build_command(
        &CommandParts {
            version: &version,
            fabric: &fabric,
            identity: &identity,
            game_dir: &game_dir,
            assets_dir: &state.paths.assets_dir(),
            libraries_dir: &state.paths.libraries_dir(),
            natives_dir: &natives,
            classpath: &classpath,
            log_config,
            memory_mb,
            profile_args,
            extra_args: inst.extra_jvm_args.clone(),
        },
        &p,
    );

    progress(&app, "starting", format!("Starting Minecraft {} with Fabric {}", inst.minecraft_version, inst.loader_version), 0, 0);
    let mut cmd = tokio::process::Command::new(&java);
    cmd.args(&args).current_dir(&game_dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut child = cmd.spawn().map_err(|e| Error::msg(format!("Couldn't start Java at {}: {e}", java.display())))?;

    {
        let mut log = state.game_log.lock().unwrap();
        log.clear();
        log.push_back(format!("[Strata] Java: {}", java.display()));
        log.push_back(format!("[Strata] Minecraft {} with {}, profile {}", version.id, fabric.id, inst.active_profile));
    }
    if let Some(out) = child.stdout.take() {
        pipe_log(app.clone(), out);
    }
    if let Some(err) = child.stderr.take() {
        pipe_log(app.clone(), err);
    }

    let (kill_tx, kill_rx) = oneshot::channel();
    *state.game.lock().await = Some(GameHandle { kill: Some(kill_tx) });
    progress(&app, "running", "Playing", 0, 0);

    let hide = instance::settings(&state.paths).map(|s| s.close_on_launch).unwrap_or(true);
    if hide {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
    }

    let started = Instant::now();
    let waiter_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let status = tokio::select! {
            s = child.wait() => s.ok(),
            _ = kill_rx => {
                let _ = child.kill().await;
                child.wait().await.ok()
            }
        };
        let state = waiter_app.state::<AppState>();
        *state.game.lock().await = None;
        let code = status.and_then(|s| s.code());
        let _ = waiter_app.emit(
            EXIT_EVENT,
            GameExit { code, crashed: code.is_some_and(|c| c != 0), seconds: started.elapsed().as_secs() },
        );
        if let Some(w) = waiter_app.get_webview_window("main") {
            let _ = w.show();
            let _ = w.set_focus();
        }
    });
    Ok(())
}

fn pipe_log<R: tokio::io::AsyncRead + Unpin + Send + 'static>(app: AppHandle, reader: R) {
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            {
                let state = app.state::<AppState>();
                let mut log = state.game_log.lock().unwrap();
                if log.len() >= LOG_LINES_KEPT {
                    log.pop_front();
                }
                log.push_back(line.clone());
            }
            let _ = app.emit(LOG_EVENT, line);
        }
    });
}

pub async fn stop(state: &AppState) -> Result<()> {
    let mut game = state.game.lock().await;
    match game.as_mut().and_then(|g| g.kill.take()) {
        Some(tx) => {
            let _ = tx.send(());
            Ok(())
        }
        None => Err(Error::msg("The game isn't running.")),
    }
}

pub async fn status(state: &AppState) -> GameStatus {
    GameStatus {
        running: state.game.lock().await.is_some(),
        launching: state.launching.load(Ordering::SeqCst),
        log: state.game_log.lock().unwrap().iter().cloned().collect(),
    }
}

pub fn new_log() -> std::sync::Mutex<VecDeque<String>> {
    std::sync::Mutex::new(VecDeque::new())
}

// --------------------------------------------------------- version list

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionOption {
    pub id: String,
    pub installed: bool,
}

/// Releases Fabric supports, from 26.1 (the first unobfuscated version) on.
pub async fn versions(state: &AppState) -> Result<Vec<VersionOption>> {
    let cache = state.paths.meta_dir().join("fabric-game-versions.json");
    let list: Vec<Value> = fetch_json(&state.http, &format!("{FABRIC_META}/versions/game"), &cache).await?;
    Ok(supported_versions(&list)
        .into_iter()
        .map(|id| VersionOption {
            installed: state.paths.versions_dir().join(&id).join(format!("{id}.json")).exists(),
            id,
        })
        .collect())
}

pub fn supported_versions(list: &[Value]) -> Vec<String> {
    list.iter()
        .filter(|v| v.get("stable").and_then(Value::as_bool) == Some(true))
        .filter_map(|v| v.get("version").and_then(Value::as_str))
        .filter(|id| id.split('.').next().and_then(|m| m.parse::<u32>().ok()).is_some_and(|m| m >= 26))
        .map(str::to_string)
        .collect()
}

/// Switch the instance's game version and pick the newest stable loader for it.
pub async fn set_version(state: &AppState, mc: &str) -> Result<Instance> {
    let cache = state.paths.loaders_dir().join("fabric").join(format!("loaders-{mc}.json"));
    let loaders: Vec<Value> = fetch_json(&state.http, &format!("{FABRIC_META}/versions/loader/{mc}"), &cache).await?;
    let loader = loaders
        .iter()
        .filter_map(|l| l.get("loader"))
        .find(|l| l.get("stable").and_then(Value::as_bool) == Some(true))
        .or_else(|| loaders.first().and_then(|l| l.get("loader")))
        .and_then(|l| l.get("version").and_then(Value::as_str))
        .ok_or_else(|| Error::msg(format!("Fabric doesn't support Minecraft {mc} yet.")))?
        .to_string();
    let id = instance::active_id(&state.paths)?;
    let mut inst = instance::load(&state.paths, &id)?;
    inst.minecraft_version = mc.to_string();
    inst.loader_version = loader;
    instance::save(&state.paths, &inst)?;
    Ok(inst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mac_arm() -> Platform {
        Platform { os: "osx", arch: "arm64" }
    }
    fn win_x64() -> Platform {
        Platform { os: "windows", arch: "x86_64" }
    }
    fn linux_x64() -> Platform {
        Platform { os: "linux", arch: "x86_64" }
    }

    /// Shaped like the real 26.3 JSON (arguments, natives, netty, logging).
    fn sample_version() -> VersionJson {
        serde_json::from_value(json!({
            "id": "26.3",
            "type": "release",
            "mainClass": "net.minecraft.client.main.Main",
            "javaVersion": { "component": "java-runtime-epsilon", "majorVersion": 25 },
            "assetIndex": { "id": "34", "url": "https://x/34.json", "sha1": "a", "size": 1 },
            "assets": "34",
            "downloads": { "client": { "url": "https://x/client.jar", "sha1": "b", "size": 41483720 } },
            "logging": { "client": { "argument": "-Dlog4j.configurationFile=${path}", "file": { "id": "client-1.21.2.xml", "url": "https://x/l.xml", "sha1": "c", "size": 1 }, "type": "log4j2-xml" } },
            "arguments": {
                "game": [
                    "--username", "${auth_player_name}", "--version", "${version_name}", "--gameDir", "${game_directory}",
                    "--assetsDir", "${assets_root}", "--assetIndex", "${assets_index_name}", "--uuid", "${auth_uuid}",
                    "--accessToken", "${auth_access_token}", "--clientId", "${clientid}", "--xuid", "${auth_xuid}",
                    "--versionType", "${version_type}",
                    { "rules": [{ "action": "allow", "features": { "is_demo_user": true } }], "value": "--demo" },
                    { "rules": [{ "action": "allow", "features": { "has_custom_resolution": true } }], "value": ["--width", "${resolution_width}", "--height", "${resolution_height}"] }
                ],
                "jvm": [
                    { "rules": [{ "action": "allow", "os": { "name": "osx" } }], "value": ["-XstartOnFirstThread"] },
                    { "rules": [{ "action": "allow", "os": { "name": "windows" } }], "value": "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump" },
                    { "rules": [{ "action": "allow", "os": { "arch": "x86" } }], "value": ["-Xss1M"] },
                    "-XX:StackShadowPages=32", "--enable-native-access=ALL-UNNAMED",
                    "-Djava.library.path=${natives_directory}/java",
                    "-Dorg.lwjgl.system.SharedLibraryExtractPath=${natives_directory}/lwjgl",
                    "-Dminecraft.launcher.brand=${launcher_name}", "-Dminecraft.launcher.version=${launcher_version}",
                    "-cp", "${classpath}"
                ]
            },
            "libraries": [
                { "name": "org.slf4j:slf4j-api:2.0.17", "downloads": { "artifact": { "path": "org/slf4j/slf4j-api/2.0.17/slf4j-api-2.0.17.jar", "url": "https://l/slf4j.jar", "sha1": "d", "size": 69908 } } },
                { "name": "org.ow2.asm:asm:9.6", "downloads": { "artifact": { "path": "org/ow2/asm/asm/9.6/asm-9.6.jar", "url": "https://l/asm-old.jar", "sha1": "e", "size": 1 } } },
                { "name": "org.lwjgl:lwjgl-sdl:3.4.3:natives-macos", "rules": [{ "action": "allow", "os": { "name": "osx" } }], "downloads": { "artifact": { "path": "org/lwjgl/lwjgl-sdl/3.4.3/lwjgl-sdl-3.4.3-natives-macos.jar", "url": "https://l/sdl-mac.jar", "sha1": "f", "size": 1 } } },
                { "name": "org.lwjgl:lwjgl-sdl:3.4.3:natives-macos-arm64", "rules": [{ "action": "allow", "os": { "name": "osx" } }], "downloads": { "artifact": { "path": "org/lwjgl/lwjgl-sdl/3.4.3/lwjgl-sdl-3.4.3-natives-macos-arm64.jar", "url": "https://l/sdl-mac-arm.jar", "sha1": "g", "size": 1 } } },
                { "name": "org.lwjgl:lwjgl-sdl:3.4.3:natives-windows", "rules": [{ "action": "allow", "os": { "name": "windows" } }], "downloads": { "artifact": { "path": "org/lwjgl/lwjgl-sdl/3.4.3/lwjgl-sdl-3.4.3-natives-windows.jar", "url": "https://l/sdl-win.jar", "sha1": "h", "size": 1 } } },
                { "name": "org.lwjgl:lwjgl-sdl:3.4.3:natives-windows-arm64", "rules": [{ "action": "allow", "os": { "name": "windows" } }], "downloads": { "artifact": { "path": "org/lwjgl/lwjgl-sdl/3.4.3/lwjgl-sdl-3.4.3-natives-windows-arm64.jar", "url": "https://l/sdl-win-arm.jar", "sha1": "i", "size": 1 } } },
                { "name": "org.lwjgl:lwjgl-sdl:3.4.3:natives-linux", "rules": [{ "action": "allow", "os": { "name": "linux" } }], "downloads": { "artifact": { "path": "org/lwjgl/lwjgl-sdl/3.4.3/lwjgl-sdl-3.4.3-natives-linux.jar", "url": "https://l/sdl-linux.jar", "sha1": "j", "size": 1 } } },
                { "name": "io.netty:netty-transport-native-epoll:4.2.16.Final:linux-x86_64", "rules": [{ "action": "allow", "os": { "name": "linux" } }], "downloads": { "artifact": { "path": "io/netty/epoll-x86.jar", "url": "https://l/epoll-x86.jar", "sha1": "k", "size": 1 } } },
                { "name": "io.netty:netty-transport-native-epoll:4.2.16.Final:linux-aarch_64", "rules": [{ "action": "allow", "os": { "name": "linux" } }], "downloads": { "artifact": { "path": "io/netty/epoll-arm.jar", "url": "https://l/epoll-arm.jar", "sha1": "l", "size": 1 } } }
            ]
        }))
        .unwrap()
    }

    fn sample_fabric() -> FabricProfile {
        serde_json::from_value(json!({
            "id": "fabric-loader-0.20.1-26.3",
            "inheritsFrom": "26.3",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "arguments": { "game": [], "jvm": ["-XX:+UnlockDiagnosticVMOptions", "-XX:-UseBASE64Intrinsics", "-DFabricMcEmu= net.minecraft.client.main.Main "] },
            "libraries": [
                { "name": "org.ow2.asm:asm:9.10.1", "url": "https://maven.fabricmc.net/", "sha1": "ada2", "size": 126151 },
                { "name": "net.fabricmc:fabric-loader:0.20.1", "url": "https://maven.fabricmc.net/" }
            ]
        }))
        .unwrap()
    }

    fn file_names(classpath: &[PathBuf]) -> Vec<String> {
        classpath.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect()
    }

    #[test]
    fn picks_libraries_for_each_platform() {
        let paths = Paths::at(PathBuf::from("/data"));
        let (v, f) = (sample_version(), sample_fabric());

        let (_, cp) = plan_files(&paths, &v, &f, &mac_arm());
        let names = file_names(&cp);
        assert!(names.contains(&"lwjgl-sdl-3.4.3-natives-macos-arm64.jar".to_string()));
        assert!(!names.contains(&"lwjgl-sdl-3.4.3-natives-macos.jar".to_string()));
        assert!(!names.iter().any(|n| n.contains("windows") || n.contains("linux") || n.contains("epoll")));
        // Fabric's ASM wins over the vanilla copy; the client jar is last.
        assert!(names.contains(&"asm-9.10.1.jar".to_string()));
        assert!(!names.contains(&"asm-9.6.jar".to_string()));
        assert_eq!(names.last().unwrap(), "26.3.jar");

        let (_, cp) = plan_files(&paths, &v, &f, &win_x64());
        let names = file_names(&cp);
        assert!(names.contains(&"lwjgl-sdl-3.4.3-natives-windows.jar".to_string()));
        assert!(!names.contains(&"lwjgl-sdl-3.4.3-natives-windows-arm64.jar".to_string()));

        let (downloads, cp) = plan_files(&paths, &v, &f, &linux_x64());
        let names = file_names(&cp);
        assert!(names.contains(&"epoll-x86.jar".to_string()));
        assert!(!names.contains(&"epoll-arm.jar".to_string()));
        // Fabric libraries come from Fabric's maven, logging config is downloaded too.
        assert!(downloads.iter().any(|d| d.url == "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar"));
        assert!(downloads.iter().any(|d| d.path.ends_with("log_configs/client-1.21.2.xml")));
    }

    #[test]
    fn builds_the_command_line() {
        let (v, f) = (sample_version(), sample_fabric());
        let id = Identity { name: "Eidan".into(), uuid: "u-u-i-d".into(), access_token: "TOKEN".into(), xuid: "123".into(), user_type: "msa" };
        let cp = vec![PathBuf::from("/l/a.jar"), PathBuf::from("/v/26.3.jar")];
        let parts = CommandParts {
            version: &v,
            fabric: &f,
            identity: &id,
            game_dir: Path::new("/inst/minecraft"),
            assets_dir: Path::new("/meta/assets"),
            libraries_dir: Path::new("/meta/libraries"),
            natives_dir: Path::new("/meta/natives/26.3"),
            classpath: &cp,
            log_config: Some(PathBuf::from("/meta/assets/log_configs/client-1.21.2.xml")),
            memory_mb: 4096,
            profile_args: vec!["-XX:+UseZGC".into()],
            extra_args: vec![],
        };

        let mac = build_command(&parts, &mac_arm());
        let main = mac.iter().position(|a| a == "net.fabricmc.loader.impl.launch.knot.KnotClient").unwrap();
        let (jvm, game) = (&mac[..main], &mac[main + 1..]);
        assert_eq!(&jvm[..3], &["-Xms4096m", "-Xmx4096m", "-XX:+UseZGC"]);
        assert!(jvm.contains(&"-XstartOnFirstThread".to_string()));
        assert!(jvm.contains(&"-DFabricMcEmu= net.minecraft.client.main.Main ".to_string()));
        assert!(jvm.contains(&"-Djava.library.path=/meta/natives/26.3/java".to_string()));
        assert!(jvm.contains(&"-Dminecraft.launcher.brand=strata".to_string()));
        assert!(jvm.contains(&"-Dlog4j.configurationFile=/meta/assets/log_configs/client-1.21.2.xml".to_string()));
        let cp_at = jvm.iter().position(|a| a == "-cp").unwrap();
        assert_eq!(jvm[cp_at + 1], "/l/a.jar:/v/26.3.jar");
        assert!(!jvm.iter().any(|a| a.contains("HeapDumpPath") || a == "-Xss1M"));

        let pair = |flag: &str| game.iter().position(|a| a == flag).map(|i| game[i + 1].clone());
        assert_eq!(pair("--username").as_deref(), Some("Eidan"));
        assert_eq!(pair("--version").as_deref(), Some("fabric-loader-0.20.1-26.3"));
        assert_eq!(pair("--accessToken").as_deref(), Some("TOKEN"));
        assert_eq!(pair("--assetIndex").as_deref(), Some("34"));
        assert_eq!(pair("--gameDir").as_deref(), Some("/inst/minecraft"));
        assert!(!game.contains(&"--demo".to_string()) && !game.contains(&"--width".to_string()));
        assert!(!mac.iter().any(|a| a.contains("${")), "unreplaced placeholder: {mac:?}");

        let win = build_command(&parts, &win_x64());
        assert!(win.iter().any(|a| a.contains("HeapDumpPath")));
        assert!(!win.contains(&"-XstartOnFirstThread".to_string()));
        assert!(win.contains(&"/l/a.jar;/v/26.3.jar".to_string()));
    }

    #[test]
    fn helpers() {
        assert_eq!(maven_path("net.fabricmc:fabric-loader:0.20.1").unwrap(), "net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar");
        assert_eq!(maven_path("org.lwjgl:lwjgl:3.4.3:natives-linux").unwrap(), "org/lwjgl/lwjgl/3.4.3/lwjgl-3.4.3-natives-linux.jar");
        assert_eq!(parse_java_major("openjdk version \"25.0.1\" 2025-10-21"), Some(25));
        assert_eq!(parse_java_major("java version \"1.8.0_402\""), Some(8));
        assert_eq!(parse_java_major("openjdk version \"21\" 2023-09-19"), Some(21));
        let vars = HashMap::from([("a", "1".to_string())]);
        assert_eq!(substitute("x${a}y${b}z${", &vars), "x1y${b}z${");
        let list = vec![
            json!({"version": "26.4-snapshot-8", "stable": false}),
            json!({"version": "26.3", "stable": true}),
            json!({"version": "26.1.2", "stable": true}),
            json!({"version": "1.21.11", "stable": true}),
        ];
        assert_eq!(supported_versions(&list), vec!["26.3", "26.1.2"]);
    }
}
