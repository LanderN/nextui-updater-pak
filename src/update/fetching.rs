use std::io::{Read, Write};
use std::sync::OnceLock;

use bytes::Bytes;
use const_format::concatcp;
use reqwest::blocking::Client;
use reqwest::IntoUrl;

use crate::github::{Release, Tag};
use crate::Result;

const USER_AGENT: &str = concatcp!("NextUIUpdater/", env!("CARGO_PKG_VERSION"));

static CLIENT_CELL: OnceLock<Client> = OnceLock::new();

fn get_client() -> &'static Client {
    CLIENT_CELL.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .timeout(None)
            .build()
            .expect("Failed to create HTTP client")
    })
}

pub fn fetch_latest_release(repo: &str) -> Result<Release> {
    let response = get_client()
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .header("User-Agent", USER_AGENT)
        .send()?;

    if !response.status().is_success() {
        return Err(format!("GitHub API request failed: {}", response.status()).into());
    }

    Ok(response.json()?)
}

pub fn fetch_releases(repo: &str) -> Result<Vec<Release>> {
    let response = get_client()
        .get(format!(
            "https://api.github.com/repos/{repo}/releases?per_page=100"
        ))
        .header("User-Agent", USER_AGENT)
        .send()?;

    if !response.status().is_success() {
        return Err(format!("GitHub API request failed: {}", response.status()).into());
    }

    Ok(response.json()?)
}

pub fn fetch_tags(repo: &str) -> Result<Vec<Tag>> {
    let response = get_client()
        .get(format!(
            "https://api.github.com/repos/{repo}/tags?per_page=100"
        ))
        .header("User-Agent", USER_AGENT)
        .send()?;

    if !response.status().is_success() {
        return Err(format!("GitHub API request failed: {}", response.status()).into());
    }

    let tags: Vec<Tag> = response.json()?;

    Ok(tags.clone())
}

pub fn download<U: IntoUrl>(url: U, progress_cb: impl Fn(f32)) -> Result<Bytes> {
    let request_builder = get_client()
        .get(url)
        .header("Accept", "application/octet-stream")
        .header("User-Agent", USER_AGENT);

    let mut response = request_builder.send()?;
    println!("Status: {}", response.status());
    println!("Headers: {:?}", response.headers());

    let total_size = response.content_length().unwrap_or(0);

    let mut bytes = Vec::new();
    let mut downloaded: u64 = 0;
    let mut buffer = [0; 16384];

    loop {
        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        bytes.write_all(&buffer[..bytes_read])?;
        downloaded += bytes_read as u64;

        // Show progress
        if total_size > 0 {
            let percentage = downloaded as f64 / total_size as f64;
            progress_cb(percentage as f32);
        }
    }

    println!("\nDownload complete!");

    Ok(bytes.into())
}
