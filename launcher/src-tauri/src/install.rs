//! Installing from Modrinth: resolve a compatible version, download it with a
//! SHA-512 check, pull in required dependencies, record it in the lock file.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha512};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

use crate::content::{self, JarKind, Kind};
use crate::error::{Error, Result};
use crate::instance::{self, LockEntry, Lockfile, Source};
use crate::modrinth::{pick_version, Modrinth, Version};
use crate::paths::{safe_file_name, Paths};
use crate::AppState;

pub const PROGRESS_EVENT: &str = "install-progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// The project the user clicked Install on; dependencies report under it.
    pub request_id: String,
    pub project_id: String,
    pub title: String,
    pub stage: Stage,
    pub received: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Resolving,
    Downloading,
    Done,
    Failed,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub installed: Vec<Installed>,
    /// Dependencies that were already present.
    pub satisfied: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub project_id: String,
    pub title: String,
    pub file: String,
    pub version_number: String,
    pub replaced: Option<String>,
    pub dependency: bool,
}

struct Job {
    project: String,
    version_id: Option<String>,
    dependency: bool,
}

pub async fn install(app: &AppHandle, state: &AppState, kind: Kind, project: &str) -> Result<InstallReport> {
    // One install at a time keeps the lock file consistent.
    let _guard = state.install_lock.lock().await;

    let id = instance::active_id(&state.paths)?;
    let inst = instance::load(&state.paths, &id)?;
    let api = Modrinth::new(&state.http);
    let loaders = kind.modrinth_loaders(&inst.loader);
    let dir = state.paths.content_dir(&id, kind);
    tokio::fs::create_dir_all(&dir).await?;

    let mut report = InstallReport::default();
    let mut seen: HashSet<String> = HashSet::new();
    let mut queue = vec![Job { project: project.to_string(), version_id: None, dependency: false }];

    let emit = |p: Progress| {
        let _ = app.emit(PROGRESS_EVENT, p);
    };

    while let Some(job) = queue.pop() {
        if !seen.insert(job.project.clone()) {
            continue;
        }

        emit(Progress {
            request_id: project.into(),
            project_id: job.project.clone(),
            title: job.project.clone(),
            stage: Stage::Resolving,
            received: 0,
            total: 0,
        });

        let version = match resolve(&api, &job, &loaders, &inst.minecraft_version).await {
            Ok(v) => v,
            Err(e) if job.dependency => {
                report.warnings.push(format!("Couldn't resolve required dependency {}: {e}", job.project));
                continue;
            }
            Err(e) => {
                emit(failed(project, &job.project));
                return Err(e);
            }
        };
        let project_meta = api.project(&version.project_id).await?;
        let title = project_meta.title.clone();

        // Skip dependencies that are already here, by Modrinth project or by
        // a local jar whose mod id matches the project slug (true for most
        // mods, e.g. fabric-api, sodium).
        if job.dependency {
            let lock = Lockfile::load(&state.paths, &id)?;
            let by_lock = lock.by_project(kind, &version.project_id).is_some();
            let by_mod_id = kind == Kind::Mod
                && content::installed_mod_ids(&state.paths, &id).iter().any(|(mid, _)| *mid == project_meta.slug);
            if by_lock || by_mod_id {
                report.satisfied.push(title);
                continue;
            }
        }

        let file = version
            .primary_file()
            .ok_or_else(|| Error::msg(format!("{title} {} has no files", version.version_number)))?
            .clone();
        let file_name = safe_file_name(&file.filename)?;

        let req_id = project.to_string();
        let pid = version.project_id.clone();
        let t = title.clone();
        let path = download_verified(&state.http, &file.url, &dir, &file_name, &file.hashes.sha512, file.size, |received, total| {
            emit(Progress {
                request_id: req_id.clone(),
                project_id: pid.clone(),
                title: t.clone(),
                stage: Stage::Downloading,
                received,
                total,
            });
        })
        .await
        .inspect_err(|_| emit(failed(project, &version.project_id)))?;

        // An update or a re-install of the same mod replaces the old jar.
        let replaced = if kind == Kind::Mod { replace_same_mod(&state.paths, &id, &path)? } else { None };

        let mut lock = Lockfile::load(&state.paths, &id)?;
        if let Some(old) = &replaced {
            lock.remove_file(kind, old);
        }
        lock.upsert(LockEntry {
            kind,
            file: file_name.clone(),
            source: Source::Modrinth,
            project_id: Some(version.project_id.clone()),
            version_id: Some(version.id.clone()),
            title: Some(title.clone()),
            sha512: Some(file.hashes.sha512.to_lowercase()),
        });
        lock.save(&state.paths, &id)?;

        emit(Progress {
            request_id: project.into(),
            project_id: version.project_id.clone(),
            title: title.clone(),
            stage: Stage::Done,
            received: file.size,
            total: file.size,
        });

        for dep in &version.dependencies {
            match (dep.dependency_type.as_str(), &dep.project_id, &dep.version_id) {
                ("required", Some(pid), vid) => queue.push(Job {
                    project: pid.clone(),
                    version_id: vid.clone(),
                    dependency: true,
                }),
                ("required", None, Some(vid)) => {
                    // Some versions only name the dependency's version.
                    if let Ok(v) = api.version(vid).await {
                        queue.push(Job { project: v.project_id, version_id: Some(v.id), dependency: true });
                    }
                }
                ("incompatible", Some(pid), _) => {
                    let lock = Lockfile::load(&state.paths, &id)?;
                    if let Some(existing) = lock.by_project(kind, pid) {
                        report.warnings.push(format!(
                            "{title} is marked incompatible with {}. Consider disabling one of them.",
                            existing.title.clone().unwrap_or_else(|| existing.file.clone())
                        ));
                    }
                }
                _ => {}
            }
        }

        report.installed.push(Installed {
            project_id: version.project_id.clone(),
            title,
            file: file_name,
            version_number: version.version_number.clone(),
            replaced,
            dependency: job.dependency,
        });
    }

    Ok(report)
}

fn failed(request: &str, project: &str) -> Progress {
    Progress {
        request_id: request.into(),
        project_id: project.into(),
        title: project.into(),
        stage: Stage::Failed,
        received: 0,
        total: 0,
    }
}

async fn resolve(api: &Modrinth<'_>, job: &Job, loaders: &[String], game_version: &str) -> Result<Version> {
    if let Some(vid) = &job.version_id {
        // A dependency pinned to a version for another game version is
        // useless to us, so only trust the pin if the API agrees it fits.
        let pinned = api.version(vid).await?;
        let fits = api
            .versions(&pinned.project_id, loaders, game_version)
            .await?
            .iter()
            .any(|v| v.id == pinned.id);
        if fits {
            return Ok(pinned);
        }
    }
    let versions = api.versions(&job.project, loaders, game_version).await?;
    pick_version(versions).ok_or_else(|| {
        Error::msg(format!("No build of {} for {game_version} ({}) on Modrinth yet", job.project, loaders.join("/")))
    })
}

/// Stream to `<dir>/.strata-tmp/<name>.part`, hash while writing, and only
/// move into place if the SHA-512 matches.
pub async fn download_verified(
    http: &reqwest::Client,
    url: &str,
    dir: &Path,
    file_name: &str,
    expected_sha512: &str,
    size_hint: u64,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<PathBuf> {
    let tmp_dir = dir.join(".strata-tmp");
    tokio::fs::create_dir_all(&tmp_dir).await?;
    let tmp = tmp_dir.join(format!("{file_name}.part"));

    let resp = http.get(url).send().await?.error_for_status()?;
    let total = resp.content_length().unwrap_or(size_hint);
    let mut out = tokio::fs::File::create(&tmp).await?;
    let mut hasher = Sha512::new();
    let mut received = 0u64;
    let mut last = Instant::now();
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        out.write_all(&chunk).await?;
        received += chunk.len() as u64;
        if last.elapsed() > Duration::from_millis(80) {
            on_progress(received, total);
            last = Instant::now();
        }
    }
    out.flush().await?;
    drop(out);
    on_progress(received, total);

    let digest = hex::encode(hasher.finalize());
    if !digest.eq_ignore_ascii_case(expected_sha512) {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(Error::msg(format!(
            "{file_name} failed its checksum, so it wasn't installed. Try again; if it keeps failing the file on Modrinth may be corrupt."
        )));
    }
    let dest = dir.join(file_name);
    tokio::fs::rename(&tmp, &dest).await?;
    let _ = tokio::fs::remove_dir(&tmp_dir).await; // only succeeds when empty
    Ok(dest)
}

/// If another jar in the folder has the same Fabric mod id as `new_jar`,
/// delete it and return its canonical name.
pub fn replace_same_mod(paths: &Paths, id: &str, new_jar: &Path) -> Result<Option<String>> {
    let Ok(JarKind::Fabric(meta)) = content::inspect_jar(new_jar, false) else { return Ok(None) };
    let Some(mod_id) = meta.id else { return Ok(None) };
    let new_name = new_jar.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let dir = paths.content_dir(id, Kind::Mod);

    for (other_id, file) in content::installed_mod_ids(paths, id) {
        if other_id == mod_id && file != new_name {
            for candidate in [dir.join(&file), dir.join(format!("{file}.disabled"))] {
                if candidate.exists() {
                    std::fs::remove_file(candidate)?;
                }
            }
            return Ok(Some(file));
        }
    }
    Ok(None)
}
