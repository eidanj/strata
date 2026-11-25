//! Minimal Modrinth v2 client: search, versions, projects.
//! Docs: https://docs.modrinth.com/api/  — 300 requests/minute per IP, and a
//! unique User-Agent is required.

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::content::Kind;
use crate::error::{Error, Result};

pub const API: &str = "https://api.modrinth.com/v2";

/// Modrinth asks for a User-Agent that identifies the app and a contact.
/// Change the contact before you ship.
pub const USER_AGENT: &str =
    concat!("eidanj/strata-launcher/", env!("CARGO_PKG_VERSION"), " (github.com/eidanj)");

pub fn http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .build()?)
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    #[default]
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

impl Sort {
    fn as_str(self) -> &'static str {
        match self {
            Sort::Relevance => "relevance",
            Sort::Downloads => "downloads",
            Sort::Follows => "follows",
            Sort::Newest => "newest",
            Sort::Updated => "updated",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub kind: Kind,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub sort: Sort,
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub follows: u64,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub date_modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub version_number: String,
    pub version_type: String,
    pub date_published: String,
    #[serde(default)]
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub size: u64,
    pub hashes: Hashes,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Hashes {
    pub sha512: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Dependency {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub dependency_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub slug: String,
    pub title: String,
}

impl Version {
    /// The file to install: the one marked primary, else the first.
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }
}

pub struct Modrinth<'a> {
    http: &'a reqwest::Client,
}

impl<'a> Modrinth<'a> {
    pub fn new(http: &'a reqwest::Client) -> Self {
        Self { http }
    }

    /// GET with one polite retry when rate-limited.
    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T> {
        let url = format!("{API}{path}");
        for attempt in 0..2 {
            let resp = self.http.get(&url).query(query).send().await?;
            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt == 0 {
                let wait = resp
                    .headers()
                    .get("x-ratelimit-reset")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(5)
                    .min(60);
                tokio::time::sleep(Duration::from_secs(wait)).await;
                continue;
            }
            if resp.status() == reqwest::StatusCode::NOT_FOUND {
                return Err(Error::msg(format!("Modrinth has nothing at {path}")));
            }
            return Ok(resp.error_for_status()?.json().await?);
        }
        Err(Error::msg("Modrinth is rate-limiting requests. Try again in a minute."))
    }

    pub async fn search(&self, q: &SearchQuery, game_version: &str, loader: &str) -> Result<SearchResults> {
        let mut facets = vec![
            vec![format!("project_type:{}", q.kind.project_type())],
            vec![format!("versions:{game_version}")],
        ];
        match q.kind {
            Kind::Mod => facets.push(vec![format!("categories:{loader}")]),
            Kind::Shader => facets.push(vec!["categories:iris".to_string()]),
            Kind::Resourcepack => {}
        }
        let query = vec![
            ("query", q.query.clone()),
            ("facets", serde_json::to_string(&facets)?),
            ("index", q.sort.as_str().to_string()),
            ("offset", q.offset.to_string()),
            ("limit", q.limit.clamp(1, 100).to_string()),
        ];
        self.get("/search", &query).await
    }

    /// Versions of a project that fit the loader(s) and game version,
    /// newest first.
    pub async fn versions(&self, id_or_slug: &str, loaders: &[String], game_version: &str) -> Result<Vec<Version>> {
        let query = vec![
            ("loaders", serde_json::to_string(loaders)?),
            ("game_versions", serde_json::to_string(&[game_version])?),
            ("include_changelog", "false".to_string()),
        ];
        let mut list: Vec<Version> = self.get(&format!("/project/{id_or_slug}/version"), &query).await?;
        list.sort_by(|a, b| b.date_published.cmp(&a.date_published));
        Ok(list)
    }

    pub async fn version(&self, version_id: &str) -> Result<Version> {
        self.get(&format!("/version/{version_id}"), &[]).await
    }

    pub async fn project(&self, id_or_slug: &str) -> Result<Project> {
        self.get(&format!("/project/{id_or_slug}"), &[]).await
    }
}

/// Prefer the newest release; fall back to the newest beta/alpha.
pub fn pick_version(versions: Vec<Version>) -> Option<Version> {
    let release = versions.iter().position(|v| v.version_type == "release");
    let index = release.or(if versions.is_empty() { None } else { Some(0) })?;
    versions.into_iter().nth(index)
}
