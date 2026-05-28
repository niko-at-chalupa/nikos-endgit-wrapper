// Copyright © 2026 Two Tech Studio
// Read-only EndGit API client.

use std::path::Path;

use reqwest::{Client as HttpClient, Response, StatusCode};
use serde::Deserialize;
use tokio::{fs, io::AsyncWriteExt};

use crate::types::{Build, BuildResponse, Plugin};
use crate::types::Response as SearchResponse;

const DEFAULT_BASE_URL: &str = "https://api.endgit.dev";
const MAX_RETRIES: u32 = 3;
const RETRY_BASE_MS: u64 = 500;

// ---------------------------------------------------------------------------
// Private envelope types — the API wraps every response in { success, data }.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// The server returned a non-2xx status with a message.
    #[error("HTTP {status}: {message}")]
    Api { status: StatusCode, message: String },

    /// A plugin slug/name was not found in the registry.
    #[error("plugin {0:?} not found")]
    NotFound(String),

    /// A named asset was absent from a GitHub release.
    #[error("asset {asset:?} not found in release {tag}")]
    AssetNotFound { asset: String, tag: String },

    /// All retry attempts were exhausted due to network errors.
    #[error("request failed after {attempts} attempts: {source}")]
    Retries {
        attempts: u32,
        #[source]
        source: reqwest::Error,
    },

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Read-only HTTP client for the EndGit plugin registry.
///
/// # Quick start
///
/// ```rust
/// let client = Client::new(None)?;
///
/// // Find plugins matching a query
/// let results = client.search_plugins("chat").await?;
///
/// // Get full details for one plugin
/// let plugin = client.get_plugin("endstone-mapdisplays").await?;
///
/// // Find out what builds are available and get the right download URL
/// let builds = client.get_plugin_builds("endstone-mapdisplays").await?;
/// let url = builds.data.builds[0].resolve_artifact_url();
///
/// // Download it
/// client.download_file(url, Path::new("./plugins"), |done, total| {
///     println!("{done}/{total}");
/// }).await?;
/// ```
pub struct Client {
    base_url: String,
    http: HttpClient,
}

impl Client {
    /// Creates a new client.
    ///
    /// `base_url` overrides the default `https://api.endgit.dev` — useful for
    /// testing against a local instance. Pass `None` in normal use.
    pub fn new(base_url: Option<&str>) -> Result<Self, ApiError> {
        let base = format!(
            "{}/api/v1",
            base_url.unwrap_or(DEFAULT_BASE_URL).trim_end_matches('/')
        );

        let http = HttpClient::builder()
            .user_agent("endgit-cli")
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self { base_url: base, http })
    }

    // -----------------------------------------------------------------------
    // Plugin registry
    // -----------------------------------------------------------------------

    /// Searches the plugin registry.
    ///
    /// Returns all plugins whose name, slug, description, or tags match
    /// `query`. Pass `""` to list everything.
    ///
    /// `GET /plugins?q={query}`
    pub async fn search_plugins(&self, query: &str) -> Result<SearchResponse, ApiError> {
        let url = reqwest::Url::parse_with_params(
            &format!("{}/plugins", self.base_url),
            &[("q", query)],
        )
        .expect("base_url is always valid");

        let resp = self.get_with_retry(url.as_str()).await?;

        if !resp.status().is_success() {
            return Err(Self::api_error(resp).await);
        }

        Ok(resp.json::<SearchResponse>().await?)
    }

    /// Fetches full details for a single plugin by slug or name.
    ///
    /// Returns `ApiError::NotFound` when the registry has no match.
    ///
    /// `GET /plugins/{name}`
    pub async fn get_plugin(&self, name: &str) -> Result<Plugin, ApiError> {
        let url = format!("{}/plugins/{}", self.base_url, urlencoding::encode(name));
        let resp = self.get_with_retry(&url).await?;

        if resp.status() == StatusCode::NOT_FOUND {
            return Err(ApiError::NotFound(name.to_owned()));
        }
        if !resp.status().is_success() {
            return Err(Self::api_error(resp).await);
        }

        Ok(resp.json::<Envelope<Plugin>>().await?.data)
    }

    /// Fetches the available builds for a plugin.
    ///
    /// Each [`Build`] carries platform-specific artifact URLs; call
    /// [`Build::resolve_artifact_url`] to get the right one for the current OS.
    ///
    /// `GET /builds/plugin/{plugin}`
    pub async fn get_plugin_builds(&self, plugin: &str) -> Result<BuildResponse, ApiError> {
        let url = format!(
            "{}/builds/plugin/{}",
            self.base_url,
            urlencoding::encode(plugin)
        );
        let resp = self.get_with_retry(&url).await?;

        if !resp.status().is_success() {
            return Err(Self::api_error(resp).await);
        }

        Ok(resp.json::<BuildResponse>().await?)
    }

    // -----------------------------------------------------------------------
    // File download
    // -----------------------------------------------------------------------

    /// Downloads a file into `dest_dir`, streaming it safely via a `.tmp`
    /// file that is atomically renamed on success — a failed download never
    /// leaves a partial file behind.
    ///
    /// The filename is taken from the `Content-Disposition` header when
    /// present, falling back to the URL path basename. The resolved filename
    /// is returned.
    ///
    /// `on_progress(bytes_written, total_bytes)` is called on every chunk.
    /// `total_bytes` is `-1` when the server omits `Content-Length`.
    ///
    /// The URL is typically obtained from [`Build::resolve_artifact_url`]:
    ///
    /// ```rust
    /// let builds = client.get_plugin_builds("my-plugin").await?;
    /// let url = builds.data.builds[0].resolve_artifact_url();
    /// let filename = client.download_file(url, Path::new("./plugins"), |_, _| {}).await?;
    /// ```
    pub async fn download_file(
        &self,
        url: &str,
        dest_dir: &Path,
        mut on_progress: impl FnMut(u64, i64),
    ) -> Result<String, ApiError> {
        let resp = self.http.get(url).send().await?;

        if !resp.status().is_success() {
            return Err(ApiError::Api {
                status: resp.status(),
                message: format!("download error: HTTP {}", resp.status()),
            });
        }

        let total = resp.content_length().map(|n| n as i64).unwrap_or(-1);
        let filename = resolve_filename(&resp, url);
        let dest_path = dest_dir.join(&filename);
        let tmp_path = dest_path.with_extension(
            dest_path
                .extension()
                .map(|e| format!("{}.tmp", e.to_string_lossy()))
                .unwrap_or_else(|| "tmp".into()),
        );

        let mut file = fs::File::create(&tmp_path).await?;
        let mut stream = resp;
        let mut written: u64 = 0;

        while let Some(chunk) = stream.chunk().await? {
            file.write_all(&chunk).await?;
            written += chunk.len() as u64;
            on_progress(written, total);
        }

        file.flush().await?;
        drop(file);

        fs::rename(&tmp_path, &dest_path).await.map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            e
        })?;

        Ok(filename)
    }

    // -----------------------------------------------------------------------
    // GitHub release helpers
    // -----------------------------------------------------------------------

    /// Returns the browser download URL for a named asset in the latest
    /// GitHub release of `repo` (e.g. `"two-tech-dev/endgit-cli"`).
    ///
    /// Returns `ApiError::AssetNotFound` when the release exists but the
    /// named asset is absent.
    pub async fn get_latest_release_asset_url(
        &self,
        repo: &str,
        asset_name: &str,
    ) -> Result<String, ApiError> {
        let release = self.fetch_github_release(repo).await?;

        release
            .assets
            .into_iter()
            .find(|a| a.name == asset_name)
            .map(|a| a.browser_download_url)
            .ok_or_else(|| ApiError::AssetNotFound {
                asset: asset_name.to_owned(),
                tag: release.tag_name,
            })
    }

    /// Returns the tag name of the latest GitHub release of `repo`
    /// (e.g. `"v0.2.1"`).
    pub async fn get_latest_release_tag(&self, repo: &str) -> Result<String, ApiError> {
        Ok(self.fetch_github_release(repo).await?.tag_name)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    async fn get_with_retry(&self, url: &str) -> Result<Response, ApiError> {
        enum Failure {
            Network(reqwest::Error),
            Server { status: StatusCode, body: String },
        }

        let mut last: Option<Failure> = None;

        for attempt in 0..MAX_RETRIES {
            match self.http.get(url).send().await {
                Err(e) => last = Some(Failure::Network(e)),
                Ok(resp) if resp.status().is_server_error() => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    last = Some(Failure::Server { status, body });
                }
                Ok(resp) => return Ok(resp),
            }

            if attempt < MAX_RETRIES - 1 {
                let wait = RETRY_BASE_MS * 2u64.pow(attempt);
                tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
            }
        }

        match last.expect("loop ran at least once") {
            Failure::Network(e) => Err(ApiError::Retries { attempts: MAX_RETRIES, source: e }),
            Failure::Server { status, body } => Err(ApiError::Api {
                status,
                message: truncate(&body, 200),
            }),
        }
    }

    async fn api_error(resp: Response) -> ApiError {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();

        #[derive(Deserialize)]
        struct ErrBody {
            message: Option<String>,
            error: Option<String>,
        }

        if let Ok(parsed) = serde_json::from_str::<ErrBody>(&body) {
            if let Some(msg) = parsed.message.filter(|s| !s.is_empty()) {
                return ApiError::Api { status, message: msg };
            }
            if let Some(msg) = parsed.error.filter(|s| !s.is_empty()) {
                return ApiError::Api { status, message: msg };
            }
        }

        ApiError::Api {
            status,
            message: if body.is_empty() { status.to_string() } else { truncate(&body, 200) },
        }
    }

    async fn fetch_github_release(&self, repo: &str) -> Result<GithubRelease, ApiError> {
        let url = format!("https://api.github.com/repos/{repo}/releases/latest");
        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(ApiError::Api {
                status: resp.status(),
                message: format!("GitHub API error: HTTP {}", resp.status()),
            });
        }

        Ok(resp.json::<GithubRelease>().await?)
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

fn resolve_filename(resp: &Response, download_url: &str) -> String {
    if let Some(cd) = resp.headers().get(reqwest::header::CONTENT_DISPOSITION) {
        if let Ok(cd_str) = cd.to_str() {
            for part in cd_str.split(';') {
                let part = part.trim();
                if let Some(val) = part
                    .strip_prefix("filename*=")
                    .or_else(|| part.strip_prefix("filename="))
                {
                    let name = val.trim_matches('"').trim();
                    if !name.is_empty() {
                        return name.to_owned();
                    }
                }
            }
        }
    }

    if let Ok(u) = reqwest::Url::parse(download_url) {
        if let Some(seg) = u.path_segments().and_then(|mut s| s.next_back()) {
            if !seg.is_empty() {
                return seg.to_owned();
            }
        }
    }

    "download".to_owned()
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_owned() } else { format!("{}...", &s[..n]) }
}