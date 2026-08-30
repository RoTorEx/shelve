use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use tar::Archive;

const APP_NAME: &str = "shelve";
const RELEASE_BASE_URL: &str = "https://github.com/RoTorEx/shelve/releases/latest/download";
const MACOS_AARCH64_ASSET: &str = "shelve-macos-aarch64.tar.gz";
const MACOS_X86_64_ASSET: &str = "shelve-macos-x86_64.tar.gz";

pub fn run() -> Result<()> {
    let asset = release_asset_for(env::consts::OS, env::consts::ARCH).ok_or_else(|| {
        anyhow::anyhow!("shelve update currently supports Apple Silicon and Intel macOS only")
    })?;
    let current_exe = env::current_exe().context("cannot locate the current shelve executable")?;
    let temp_dir = UpdateTempDir::create()?;
    let client = reqwest::blocking::Client::new();

    let archive_url = format!("{RELEASE_BASE_URL}/{asset}");
    let checksum_url = format!("{archive_url}.sha256");
    eprintln!("Downloading {archive_url}");

    let archive_bytes = download_bytes(&client, &archive_url)?;
    let checksum_bytes = download_bytes(&client, &checksum_url)?;
    let checksum_text =
        std::str::from_utf8(&checksum_bytes).context("release checksum is not valid UTF-8")?;
    verify_checksum(&archive_bytes, checksum_text)?;

    let updated = temp_dir.path().join(APP_NAME);
    extract_binary(&archive_bytes, &updated)?;
    set_executable(&updated)?;

    let updated_version = read_version(&updated)?;
    let current_version = format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION"));
    if updated_version == current_version {
        println!("Already up to date: {updated_version}");
        return Ok(());
    }

    install_binary(&updated, &current_exe)?;
    println!("Updated {current_version} -> {updated_version}");
    Ok(())
}

fn release_asset_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("macos", "aarch64") => Some(MACOS_AARCH64_ASSET),
        ("macos", "x86_64") => Some(MACOS_X86_64_ASSET),
        _ => None,
    }
}

fn download_bytes(client: &reqwest::blocking::Client, url: &str) -> Result<Vec<u8>> {
    let response = client
        .get(url)
        .send()
        .with_context(|| format!("failed to download {url}"))?
        .error_for_status()
        .with_context(|| format!("release download failed for {url}"))?;

    Ok(response
        .bytes()
        .with_context(|| format!("failed to read {url}"))?
        .to_vec())
}

fn verify_checksum(archive: &[u8], checksum_file: &str) -> Result<()> {
    let expected = parse_checksum(checksum_file)?;
    let actual = format!("{:x}", Sha256::digest(archive));
    if !actual.eq_ignore_ascii_case(expected) {
        bail!("release archive checksum mismatch");
    }
    Ok(())
}

fn parse_checksum(checksum_file: &str) -> Result<&str> {
    let checksum = checksum_file
        .split_whitespace()
        .next()
        .context("release checksum file is empty")?;
    if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("release checksum is not a SHA-256 digest");
    }
    Ok(checksum)
}

fn extract_binary(archive_bytes: &[u8], destination: &Path) -> Result<()> {
    let decoder = GzDecoder::new(Cursor::new(archive_bytes));
    let mut archive = Archive::new(decoder);
    let mut found = false;

    for entry in archive.entries().context("cannot read release archive")? {
        let mut entry = entry.context("cannot read release archive entry")?;
        if entry
            .path()
            .context("release archive path is invalid")?
            .as_ref()
            != Path::new(APP_NAME)
        {
            continue;
        }
        if found || !entry.header().entry_type().is_file() {
            bail!("release archive contains an invalid shelve entry");
        }
        entry
            .unpack(destination)
            .context("cannot extract updated shelve binary")?;
        found = true;
    }

    if !found {
        bail!("release archive does not contain a shelve binary");
    }
    Ok(())
}

fn read_version(binary: &Path) -> Result<String> {
    let output = Command::new(binary)
        .arg("-V")
        .stdin(Stdio::null())
        .output()
        .context("cannot run the downloaded shelve binary")?;
    if !output.status.success() {
        bail!("downloaded shelve binary failed its version check");
    }
    let version = std::str::from_utf8(&output.stdout)
        .context("downloaded shelve version is not valid UTF-8")?
        .trim();
    if !valid_version_output(version) {
        bail!("downloaded binary returned an invalid shelve version");
    }
    Ok(version.to_owned())
}

fn valid_version_output(version: &str) -> bool {
    version.starts_with("shelve ") && version.split_whitespace().count() == 2
}

fn install_binary(source: &Path, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .context("current shelve executable has no parent directory")?;
    let temporary = parent.join(format!(".shelve-update-{}", std::process::id()));

    let result = (|| -> Result<()> {
        fs::copy(source, &temporary)
            .with_context(|| format!("cannot copy updated shelve to {}", temporary.display()))?;
        set_executable(&temporary)?;
        fs::rename(&temporary, destination)
            .with_context(|| format!("cannot replace {}", destination.display()))?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn set_executable(path: &Path) -> Result<()> {
    let mut permissions = fs::metadata(path)
        .with_context(|| format!("cannot inspect {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("cannot make {} executable", path.display()))
}

struct UpdateTempDir {
    path: PathBuf,
}

impl UpdateTempDir {
    fn create() -> Result<Self> {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system time is before the Unix epoch")?
            .as_nanos();
        let path = env::temp_dir().join(format!("shelve-update-{suffix}"));
        fs::create_dir(&path)
            .with_context(|| format!("cannot create update directory {}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for UpdateTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_every_published_macos_asset() {
        assert_eq!(
            release_asset_for("macos", "aarch64"),
            Some("shelve-macos-aarch64.tar.gz")
        );
        assert_eq!(
            release_asset_for("macos", "x86_64"),
            Some("shelve-macos-x86_64.tar.gz")
        );
        assert_eq!(release_asset_for("linux", "x86_64"), None);
    }

    #[test]
    fn accepts_matching_sha256_checksum() {
        verify_checksum(
            b"shelve release",
            "b980d22c84514f1ba27bb26a9777d4a4814131b2448a54597c1531eef9ab8850  shelve.tar.gz",
        )
        .unwrap();
    }

    #[test]
    fn rejects_mismatched_sha256_checksum() {
        let error = verify_checksum(
            b"tampered",
            "b980d22c84514f1ba27bb26a9777d4a4814131b2448a54597c1531eef9ab8850  shelve.tar.gz",
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "release archive checksum mismatch");
    }

    #[test]
    fn rejects_malformed_checksum() {
        let error = parse_checksum("not-a-checksum shelve.tar.gz").unwrap_err();

        assert_eq!(
            error.to_string(),
            "release checksum is not a SHA-256 digest"
        );
    }

    #[test]
    fn validates_downloaded_version_shape() {
        assert!(valid_version_output("shelve 0.1.0"));
        assert!(!valid_version_output("other 0.1.0"));
        assert!(!valid_version_output("shelve 0.1.0 extra"));
    }
}
