//! Pinned Natural Earth source and download helper.
//!
//! Bumping the pin shifts entity IDs, areas, and border tiles, so the URL
//! and content hash live in code (deliberate change, single PR) rather than
//! as CLI arguments.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};

/// Commit pinned on `nvkelso/natural-earth-vector` (master @ 2026-04-26).
/// Bumping this shifts entity IDs, areas, and border tiles for every POV.
pub const NATURAL_EARTH_COMMIT: &str = "ca96624a56bd078437bca8184e78163e5039ad19";

/// Raw-GitHub base for the pinned commit's `geojson/` directory.
pub const NATURAL_EARTH_BASE: &str =
    "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/\
     ca96624a56bd078437bca8184e78163e5039ad19/geojson";

/// Point-of-view variant of Natural Earth Admin-0 Countries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pov {
    Iso,
    Chn,
    Usa,
}

impl Pov {
    pub fn id(self) -> &'static str {
        match self {
            Pov::Iso => "iso",
            Pov::Chn => "chn",
            Pov::Usa => "usa",
        }
    }

    fn filename(self) -> &'static str {
        match self {
            Pov::Iso => "ne_10m_admin_0_countries_iso.geojson",
            Pov::Chn => "ne_10m_admin_0_countries_chn.geojson",
            Pov::Usa => "ne_10m_admin_0_countries_usa.geojson",
        }
    }

    pub fn url(self) -> String {
        format!("{NATURAL_EARTH_BASE}/{}", self.filename())
    }

    /// SHA-256 of the pinned file's raw bytes (recorded at pin time).
    pub fn sha256(self) -> &'static str {
        match self {
            Pov::Iso => "60eb10aa951f5872507c9436937508b09be4b43dc9fa7aad7644f23ef12e1cad",
            Pov::Chn => "a13bf5f310fde87bc0a5f994f8ce9bd706cc198d8ee37d221e61c2546b945372",
            Pov::Usa => "d3166691d3d86f113c0d8db52506f4b72936513691d1593f47010fed01fc0b93",
        }
    }
}

impl std::str::FromStr for Pov {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "iso" => Ok(Pov::Iso),
            "chn" => Ok(Pov::Chn),
            "usa" => Ok(Pov::Usa),
            other => bail!("unknown --pov `{other}` (expected iso|chn|usa)"),
        }
    }
}

/// Ensure `path` contains the pinned Natural Earth GeoJSON. If the file is
/// missing or its SHA-256 doesn't match the pin, re-download and verify.
pub fn ensure_geojson(path: &Path, pov: Pov) -> Result<()> {
    let url = pov.url();
    match sha256_of(path)? {
        Some(actual) if actual == pov.sha256() => {
            eprintln!(
                "[geo_rasterizer] geojson cache hit ({})",
                &pov.sha256()[..12]
            );
            return Ok(());
        }
        Some(actual) => eprintln!(
            "[geo_rasterizer] {} hash mismatch (got {}, expected {}) — re-downloading",
            path.display(),
            &actual[..12],
            &pov.sha256()[..12]
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
    download_to(path, &url).with_context(|| format!("downloading {}", path.display()))?;

    let actual =
        sha256_of(path)?.ok_or_else(|| anyhow!("downloaded file vanished: {}", path.display()))?;
    if actual != pov.sha256() {
        bail!(
            "downloaded {} has SHA-256 {actual} but expected {}",
            path.display(),
            pov.sha256()
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
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(120))
        .build();
    let resp = agent
        .get(url)
        .call()
        .with_context(|| format!("GET {url}"))?;
    let total: u64 = resp
        .header("content-length")
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

    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    let mut tmp = match parent {
        Some(p) => tempfile::NamedTempFile::new_in(p),
        None => tempfile::NamedTempFile::new_in("."),
    }
    .context("creating download tempfile")?;
    {
        let mut reader = pb.wrap_read(resp.into_reader());
        std::io::copy(&mut reader, tmp.as_file_mut())?;
        tmp.as_file_mut().sync_all()?;
    }
    pb.finish_and_clear();
    tmp.persist(path)
        .map_err(|e| anyhow!("renaming download into place: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn pov_table_is_consistent() {
        for pov in [Pov::Iso, Pov::Chn, Pov::Usa] {
            let url = pov.url();
            assert!(
                url.starts_with(NATURAL_EARTH_BASE),
                "url not under pinned base: {url}"
            );
            assert!(url.ends_with(".geojson"));
            assert_eq!(pov.sha256().len(), 64, "sha must be 64 hex chars");
            assert!(pov.sha256().bytes().all(|b| b.is_ascii_hexdigit()));
        }
        // Guard NATURAL_EARTH_BASE itself (the per-POV url assertions above
        // are tautological w.r.t. the base, so spot-check the base directly).
        assert!(
            NATURAL_EARTH_BASE.starts_with("https://raw.githubusercontent.com/nvkelso/"),
            "base URL looks wrong: {NATURAL_EARTH_BASE}"
        );
        assert!(
            !NATURAL_EARTH_BASE.contains(' '),
            "base has embedded whitespace"
        );
        // Pin desync guard: the base must embed the pinned commit, so a
        // future bump that updates only one of the two consts fails here.
        assert!(
            NATURAL_EARTH_BASE.contains(NATURAL_EARTH_COMMIT),
            "NATURAL_EARTH_BASE does not contain NATURAL_EARTH_COMMIT (pin desync)"
        );
        assert_eq!(
            Pov::Iso.sha256(),
            "60eb10aa951f5872507c9436937508b09be4b43dc9fa7aad7644f23ef12e1cad"
        );
        assert_eq!(Pov::from_str("chn").unwrap(), Pov::Chn);
        assert!(Pov::from_str("bogus").is_err());
    }
}
