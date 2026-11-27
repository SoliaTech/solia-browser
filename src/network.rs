use reqwest::{Client, StatusCode, header};
use solia::AppInstance;
use std::time::Duration;
use url::{Host, Url};

const MAX_SOURCE: usize = 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(15);

pub struct Loaded {
    pub origin: Url,
    pub app: AppInstance,
}

fn loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(name)) => name == "localhost",
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

fn allowed(url: &Url) -> bool {
    url.has_host()
        && url.username().is_empty()
        && url.password().is_none()
        && (url.scheme() == "https" || (url.scheme() == "http" && loopback(url)))
}

pub fn client() -> Result<Client, String> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())
}

async fn download(client: &Client, mut url: Url) -> Result<(Url, Vec<u8>), String> {
    let loopback_only = url.scheme() == "http";
    for hop in 0..=3 {
        let mut response = client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| format!("Download: {e}"))?;
        let status = response.status();
        if matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
            if hop == 3 {
                return Err("Too many redirects (limit 3)".into());
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or("Redirect has no valid Location")?;
            let next = url
                .join(location)
                .map_err(|e| format!("Invalid redirect: {e}"))?;
            if !allowed(&next)
                || (url.scheme() == "https" && next.scheme() != "https")
                || (loopback_only && !loopback(&next))
            {
                return Err(format!("Redirect destination not allowed: {next}"));
            }
            url = next;
            continue;
        }
        if status != StatusCode::OK {
            return Err(format!("{url}: HTTP {status}"));
        }
        if let Some(value) = response.headers().get(header::CONTENT_TYPE) {
            let media = value
                .to_str()
                .map_err(|_| "Invalid Content-Type")?
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            if !matches!(
                media.as_str(),
                "text/plain" | "application/octet-stream" | "application/vnd.solia.source"
            ) {
                return Err(format!("{url}: unsupported Content-Type {media}"));
            }
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("Reading {url}: {e}"))?
        {
            if chunk.len() > MAX_SOURCE - body.len() {
                return Err(format!("{url}: source exceeds 1 MiB"));
            }
            body.extend_from_slice(&chunk);
        }
        return Ok((url, body));
    }
    unreachable!("redirect limit checked before following")
}

pub async fn load(client: &Client, address: &str) -> Result<Loaded, String> {
    let url = Url::parse(address).map_err(|e| format!("Invalid URL: {e}"))?;
    if !allowed(&url) || !url.path().ends_with(".solia") {
        return Err("Enter an HTTPS .solia source URL, or a loopback HTTP development URL.".into());
    }
    let (origin, bytes) = tokio::time::timeout(DEADLINE, download(client, url))
        .await
        .map_err(|_| "Download timed out after 15 seconds")??;
    let src = std::str::from_utf8(&bytes).map_err(|e| format!("{origin}: invalid UTF-8: {e}"))?;
    let package = solia::compile_source(src).map_err(|e| format!("{origin}:{e}"))?;
    let mut app = AppInstance::load(&package).map_err(|e| format!("{origin}: load failed: {e}"))?;
    app.set_source(origin.to_string());
    Ok(Loaded { origin, app })
}
