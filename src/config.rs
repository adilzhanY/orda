use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

pub const DEFAULT: &str = include_str!("default_config.toml");

#[derive(Deserialize, Clone)]
pub struct Config {
    #[serde(default = "orda")]
    pub theme: String,
    #[serde(default)]
    pub colors: HashMap<String, String>,
    #[serde(default)]
    pub layout: Layout,
    #[serde(default)]
    pub notify: Notify,
    #[serde(default = "shipped_guard")]
    pub guard: Guard,
    #[serde(default = "shipped_agents")]
    pub agents: Vec<AgentDef>,
}

/// The guard rules and team from the shipped file, used for whatever a user file leaves out.
#[derive(Deserialize)]
struct Shipped {
    guard: Guard,
    agents: Vec<AgentDef>,
}

fn shipped() -> Shipped {
    toml::from_str(DEFAULT).expect("default config parses")
}

fn shipped_guard() -> Guard {
    shipped().guard
}

fn shipped_agents() -> Vec<AgentDef> {
    shipped().agents
}

fn orda() -> String {
    "orda".into()
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct Layout {
    pub left: Vec<String>,
    pub right: Vec<String>,
    pub bottom: Vec<String>,
    pub bottom_height: u16,
    pub compact_width: u16,
    pub compact_height: u16,
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct Notify {
    pub desktop: bool,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
pub struct Guard {
    pub deny: Vec<String>,
    pub ask: Vec<String>,
}

#[derive(Deserialize, Clone)]
pub struct AgentDef {
    pub name: String,
    #[serde(default = "worker")]
    pub role: String,
    pub vendor: String,
    pub model: String,
    #[serde(default)]
    pub effort: String,
    #[serde(default)]
    pub job: String,
    #[serde(default)]
    pub always_on: bool,
}

fn worker() -> String {
    "worker".into()
}

impl Default for Config {
    fn default() -> Self {
        toml::from_str(DEFAULT).expect("default config parses")
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            left: vec!["team".into()],
            right: ["questions", "git", "web", "guard"]
                .map(String::from)
                .into(),
            bottom: vec!["log".into(), "usage".into()],
            bottom_height: 9,
            compact_width: 170,
            compact_height: 44,
        }
    }
}

impl Default for Notify {
    fn default() -> Self {
        Self { desktop: true }
    }
}

pub fn path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"));
    base.join("orda/config.toml")
}

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// Reads the config file. A missing file means defaults; a broken one returns
/// defaults plus the error so the UI can show it.
pub fn load() -> (Config, Option<String>) {
    match std::fs::read_to_string(path()) {
        Err(_) => (Config::default(), None),
        Ok(text) => match toml::from_str::<Config>(&text) {
            Ok(c) => (c, None),
            Err(e) => (Config::default(), Some(format!("config: {}", e.message()))),
        },
    }
}

pub fn modified() -> Option<std::time::SystemTime> {
    std::fs::metadata(path()).and_then(|m| m.modified()).ok()
}

/// `orda config`: write the commented default file if there is none.
pub fn write_default() -> std::io::Result<()> {
    let p = path();
    if p.exists() {
        println!("{} (already exists)", p.display());
        return Ok(());
    }
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(&p, DEFAULT)?;
    println!("{}", p.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_team() {
        let c = Config::default();
        assert_eq!(c.agents[0].role, "boss");
        assert!(c.agents.iter().any(|a| a.always_on));
        assert_eq!(c.layout.right.len(), 4);
    }

    #[test]
    fn partial_file_keeps_defaults() {
        let c: Config = toml::from_str("theme = \"iris\"").unwrap();
        assert_eq!(c.theme, "iris");
        assert_eq!(c.layout.compact_width, 170);
    }
}
