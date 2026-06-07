use anyhow::{anyhow, Result};
use client::http::HttpClient;
use serde::Deserialize;
use std::sync::Arc;

pub struct GitHubLspBinaryVersion {
    pub name: String,
    pub url: String,
}

#[derive(Deserialize)]
pub(crate) struct GithubRelease {
    pub name: String,
    pub assets: Vec<GithubReleaseAsset>,
}

#[derive(Deserialize)]
pub(crate) struct GithubReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

pub(crate) async fn latest_github_release(
    _repo_name_with_owner: &str,
    _http: Arc<dyn HttpClient>,
) -> Result<GithubRelease, anyhow::Error> {
    Err(anyhow!(
        "automatic language server release lookup is disabled in this build"
    ))
}
