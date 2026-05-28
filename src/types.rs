// Copyright © 2026 Two Tech Studio
// Rust port of api/types.go

use serde::Deserialize;

/// API response for plugin searches.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Response {
    pub success: bool,
    pub data: Data,
    pub pagination: Pagination,
}

/// Contains plugin search results.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Data {
    pub plugins: Vec<Plugin>,
}

/// A single plugin in the registry.
#[derive(Debug, Clone, Deserialize, PartialEq)]
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
    #[serde(default)]
    pub status_reason: Option<String>,
    #[serde(default)]
    pub stability_score: u64,
    pub trust_score: u64,
    #[serde(default)]
    pub quality_badge: String,
    #[serde(default)]
    pub is_verified: bool,
    #[serde(default)]
    pub is_featured: bool,
    #[serde(default)]
    pub review_build_id: Option<String>,
    #[serde(default)]
    pub webhook_id: Option<String>,
    #[serde(default)]
    pub author_id: String,
    pub latest_version: String,
    #[serde(default)]
    pub is_pre_release: bool,
    pub created_at: String,
    pub updated_at: String,
    pub author: Author,
}

/// A plugin author.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub username: String,
    pub display_name: String,
    pub avatar_url: String,
}

/// Pagination information.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub page: u32,
    pub page_size: u32,
    pub total: u32,
    pub total_pages: u32,
}

/// API response for build queries.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct BuildResponse {
    pub success: bool,
    pub data: BuildData,
}

/// Inner data wrapper for [`BuildResponse`].
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct BuildData {
    pub builds: Vec<Build>,
}

/// A single plugin build.
#[derive(Debug, Clone, Deserialize, PartialEq)]
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
    /// Returns the appropriate artifact URL for the current OS.
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