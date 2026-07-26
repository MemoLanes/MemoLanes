use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use geo_data_format::{Locale, Worldview};
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};

use crate::atomic_write::write_atomically_with;

pub fn ensure_geojson(path: &Path, worldview: Worldview) -> Result<()> {
    ensure_pinned(
        path,
        &worldview.source_url(),
        worldview.spec().source_sha256,
    )
}

pub fn ensure_cldr(path: &Path, locale: Locale) -> Result<()> {
    ensure_pinned(
        path,
        &locale.cldr_source_url(),
        locale.spec().cldr_source_sha256,
    )
}

fn ensure_pinned(path: &Path, url: &str, expected: &str) -> Result<()> {
    match sha256_of(path)? {
        Some(actual) if actual == expected => {
            eprintln!(
                "[geo_rasterizer] {} cache hit ({})",
                path.display(),
                &expected[..12]
            );
            return Ok(());
        }
        Some(actual) => eprintln!(
            "[geo_rasterizer] {} hash mismatch (got {}, expected {}) — re-downloading",
            path.display(),
            &actual[..12],
            &expected[..12]
        ),
        None => eprintln!(
            "[geo_rasterizer] {} missing — downloading from {}",
            path.display(),
            url
        ),
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    download_to(path, url).with_context(|| format!("downloading {}", path.display()))?;

    let actual =
        sha256_of(path)?.ok_or_else(|| anyhow!("downloaded file vanished: {}", path.display()))?;
    if actual != expected {
        bail!(
            "downloaded {} has SHA-256 {actual} but expected {}",
            path.display(),
            expected
        );
    }
    eprintln!("[geo_rasterizer] download verified");
    Ok(())
}

fn sha256_of(path: &Path) -> Result<Option<String>> {
    let mut f = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("opening {}", path.display())),
    };
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(Some(hex_lower(&hasher.finalize())))
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn download_to(path: &Path, url: &str) -> Result<()> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .build()
        .new_agent();
    let resp = agent
        .get(url)
        .call()
        .with_context(|| format!("GET {url}"))?;
    let total: u64 = resp
        .headers()
        .get("content-length")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.parse().ok())
        .unwrap_or(0);
    let pb = if total > 0 {
        let pb = ProgressBar::new(total);
        let style = ProgressStyle::with_template(
            "[geo_rasterizer]   {bar:30.cyan/blue} {bytes}/{total_bytes} ({eta})",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar());
        pb.set_style(style);
        pb
    } else {
        ProgressBar::new_spinner()
    };

    {
        let mut reader = pb.wrap_read(resp.into_body().into_reader());
        write_atomically_with(path, |f| {
            std::io::copy(&mut reader, f)?;
            Ok(())
        })?;
    }
    pb.finish_and_clear();
    Ok(())
}
