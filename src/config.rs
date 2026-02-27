use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Default)]
pub struct ItkConfig {
    #[serde(default)]
    pub defaults: Defaults,
}

#[derive(Debug, Deserialize, Default)]
pub struct Defaults {
    /// Default to compact mode
    #[serde(default)]
    pub compact: bool,
    /// Default to aggressive mode
    #[serde(default)]
    pub aggressive: bool,
    /// Default to no-frame mode
    #[serde(default)]
    pub no_frame: bool,
    /// Always show stats
    #[serde(default)]
    pub stats: bool,
}

/// Load configuration from `.itk.json` (project) or `~/.config/itk/config.json` (global).
/// Project config takes precedence over global.
/// Returns None if no config found — that's fine, CLI flags are sufficient.
pub fn load() -> Option<ItkConfig> {
    // 1. Check project-local .itk.json
    let project_config = PathBuf::from(".itk.json");
    if project_config.exists() {
        if let Some(cfg) = load_from(&project_config) {
            return Some(cfg);
        }
    }

    // 2. Check global config
    let global_config = global_config_path();
    if let Some(ref path) = global_config {
        if path.exists() {
            if let Some(cfg) = load_from(path) {
                return Some(cfg);
            }
        }
    }

    None
}

fn load_from(path: &PathBuf) -> Option<ItkConfig> {
    let content = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<ItkConfig>(&content) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            eprintln!("itk: warning: invalid config at {}: {e}", path.display());
            None
        }
    }
}

fn global_config_path() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        Some(PathBuf::from(home).join(".config").join("itk").join("config.json"))
    } else if let Ok(profile) = std::env::var("USERPROFILE") {
        Some(PathBuf::from(profile).join(".config").join("itk").join("config.json"))
    } else {
        None
    }
}
