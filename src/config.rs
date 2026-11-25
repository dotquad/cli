//! Where the API key comes from: `DOTQUAD_KEY`, then the config file.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use etcetera::BaseStrategy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct File {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    Env,
    File,
}

/// `~/.config/dotquad/config.toml` on macOS and Linux, `%APPDATA%\dotquad\config.toml` on
/// Windows. `DOTQUAD_CONFIG` points somewhere else.
pub fn path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("DOTQUAD_CONFIG").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    let base = etcetera::choose_base_strategy().context("Couldn't find your home folder")?;
    Ok(base.config_dir().join("dotquad").join("config.toml"))
}

pub fn load() -> Result<File> {
    let path = path()?;
    match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).with_context(|| format!("Couldn't read {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(File::default()),
        Err(e) => Err(e).with_context(|| format!("Couldn't open {}", path.display())),
    }
}

pub fn save(file: &File) -> Result<PathBuf> {
    let path = path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("Couldn't create {}", dir.display()))?;
    }
    let text = toml::to_string(file)?;
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&path).with_context(|| format!("Couldn't write {}", path.display()))?;
    // The mode above only applies to a new file; tighten an existing one too.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }
    f.write_all(text.as_bytes())?;
    Ok(path)
}

/// The key to use, and where it came from.
pub fn key() -> Result<Option<(String, KeySource)>> {
    if let Ok(k) = std::env::var("DOTQUAD_KEY") {
        let k = k.trim().to_string();
        if !k.is_empty() {
            return Ok(Some((k, KeySource::Env)));
        }
    }
    Ok(load()?.key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).map(|k| (k, KeySource::File)))
}

pub fn check_key_format(key: &str) -> Result<()> {
    if !key.starts_with("dq_") || key.len() < 12 || key.chars().any(char::is_whitespace) {
        bail!("That doesn't look like a Dotquad key. Keys start with dq_live_.");
    }
    Ok(())
}

/// `dq_live_ab12…9xyz`: enough to tell keys apart, not enough to use one.
pub fn mask(key: &str) -> String {
    let n = key.chars().count();
    if n <= 16 {
        return "dq_…".into();
    }
    let head: String = key.chars().take(12).collect();
    let tail: String = key.chars().skip(n - 4).collect();
    format!("{head}…{tail}")
}
