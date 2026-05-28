// Copyright © 2026 Two Tech Studio
// Rust port of api/types.go

use serde::Deserialize;

/// API response for plugin searches.
#[derive(Debug, Clone, Deserialize)]
pub struct Response {
    pub success: bool,
    pub data: Data,
    pub pagination: Pagination,
}

/// Contains plugin search results.
#[derive(Debug, Clone, Deserialize)]
pub struct Data {
    pub plugins: Vec<Plugin>,
}

/// A single plugin in the registry.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub display_name: String,
    pub description: String,
    pub long_description: String,
    pub icon_url: String,
    pub repo_url: String,
    pub license: String,
    pub tags: Vec<String>,
    pub keywords: Vec<String>,
    pub plugin_type: String,
    pub downloads: u64,
    pub stars: u64,
    pub comment_count: u64,
    pub heat_score: u64,
    pub status: String,
    pub trust_score: u64,
    pub latest_version: String,
    pub created_at: String,
    pub updated_at: String,
    pub author: Author,
}

/// A plugin author.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub username: String,
    pub display_name: String,
    pub avatar_url: String,
}

/// The authenticated user.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    pub display_name: String,
}

/// Pagination information.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub page: u32,
    pub page_size: u32,
    pub total: u32,
    pub total_pages: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub username: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub username: String,
}

#[derive(Debug, Clone)]
pub struct DeviceAuthError {
    pub code: String,
}

impl std::fmt::Display for DeviceAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code)
    }
}

impl std::error::Error for DeviceAuthError {}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildResponse {
    pub success: bool,
    pub data: BuildData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildData {
    pub builds: Vec<Build>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Build {
    pub build_number: u32,
    pub commit_hash: String,
    pub status: String,
    pub artifact_url: String,
    pub artifact_url_win: String,
    pub artifact_url_linux: String,
}

impl Build {
    pub fn resolve_artifact_url(&self) -> &str {
        #[cfg(target_os = "windows")]
        if !self.artifact_url_win.is_empty() {
            return &self.artifact_url_win;
        }

        #[cfg(target_os = "linux")]
        if !self.artifact_url_linux.is_empty() {
            return &self.artifact_url_linux;
        }

        &self.artifact_url
    }
}