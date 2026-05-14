use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

/// Top-level config file structure (`~/.config/agency/config.toml`).
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub defaults: Defaults,
}

#[derive(Debug, Default, Deserialize)]
pub struct Defaults {
    /// Provider name used when `--provider` is not given on the CLI.
    pub provider: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ProviderConfig {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub default_model: Option<String>,
}

impl Config {
    /// Load config from `path`, or from the default location if `None`.
    /// Returns an empty `Config` if the file does not exist.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let path = match path {
            Some(p) => p.to_path_buf(),
            None => default_path()?,
        };
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Error::Config(format!("{}: {e}", path.display())))?;
        toml::from_str(&text).map_err(|e| Error::Config(format!("{}: {e}", path.display())))
    }

    /// Look up a named provider section.
    pub fn provider(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }
}

fn default_path() -> Result<PathBuf> {
    // Use XDG_CONFIG_HOME if set, otherwise ~/.config — consistent on all
    // platforms and matches user expectations for a CLI tool.
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .ok_or_else(|| Error::Config("cannot locate config directory".into()))?;
    Ok(base.join("agency").join("config.toml"))
}
