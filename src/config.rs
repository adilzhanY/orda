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
    /// vendor -> the model its agents move to while that vendor's limit is used up
    #[serde(default = "shipped_fallback")]
    pub fallback: HashMap<String, Fallback>,
    #[serde(default)]
    pub watch: Watch,
    #[serde(default)]
    pub fresh: Fresh,
}

/// When a run is restarted with a fresh context and a handoff.
#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct Fresh {
    pub minutes: u64,
    pub tool_calls: u64,
    pub context: u64,
}

impl Default for Fresh {
    fn default() -> Self {
        Self {
            minutes: 35,
            tool_calls: 80,
            context: 160_000,
        }
    }
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct Watch {
    /// Hand every new commit to the always-on agents (tester, ripple, aegis).
    pub commits: bool,
}

impl Default for Watch {
    fn default() -> Self {
        Self { commits: true }
    }
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct Fallback {
    pub vendor: String,
    pub model: String,
    #[serde(default)]
    pub effort: String,
}

/// The guard rules and team from the shipped file, used for whatever a user file leaves out.
#[derive(Deserialize)]
struct Shipped {
    guard: Guard,
    agents: Vec<AgentDef>,
    fallback: HashMap<String, Fallback>,
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

fn shipped_fallback() -> HashMap<String, Fallback> {
    shipped().fallback
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
    /// A folder in agents/. Empty means the agent's name.
    #[serde(default)]
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

impl Default for Config {
    fn default() -> Self {
        finish(toml::from_str(DEFAULT).expect("default config parses"))
    }
}

fn finish(mut c: Config) -> Config {
    for a in &mut c.agents {
        if a.role.is_empty() {
            a.role = a.name.clone();
        }
    }
    c
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
            Ok(c) => (finish(c), None),
            Err(e) => (Config::default(), Some(format!("config: {}", e.message()))),
        },
    }
}

/// Switch one agent to another vendor and model in the settings file, keeping
/// everything else (comments included) as it is.
pub fn set_model(name: &str, vendor: &str, model: &str) -> Result<(), String> {
    let p = path();
    let text = std::fs::read_to_string(&p).unwrap_or_else(|_| DEFAULT.to_string());
    let new = set_model_in(&text, name, vendor, model)
        .ok_or(format!("no agent named {name} in {}", p.display()))?;
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&p, new).map_err(|e| e.to_string())
}

fn set_model_in(text: &str, name: &str, vendor: &str, model: &str) -> Option<String> {
    let mut out = Vec::new();
    let mut inside = false;
    let mut found = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            inside = false;
        }
        if t == format!("name = \"{name}\"") {
            inside = true;
            found = true;
        }
        if inside && t.starts_with("vendor =") {
            out.push(format!("vendor = \"{vendor}\""));
        } else if inside && t.starts_with("model =") {
            out.push(format!("model = \"{model}\""));
        } else {
            out.push(line.to_string());
        }
    }
    found.then(|| out.join("\n") + "\n")
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
        assert_eq!(c.layout.right.len(), 6);
    }

    #[test]
    fn set_model_touches_one_agent() {
        let new = set_model_in(DEFAULT, "tester", "codex", "gpt-7").unwrap();
        let c = finish(toml::from_str::<Config>(&new).unwrap());
        let t = c.agents.iter().find(|a| a.name == "tester").unwrap();
        assert_eq!((t.vendor.as_str(), t.model.as_str()), ("codex", "gpt-7"));
        let b = c.agents.iter().find(|a| a.name == "builder-2").unwrap();
        assert_eq!(b.model, "sonnet");
        assert!(new.contains("# orda settings"));
        assert!(set_model_in(DEFAULT, "nobody", "codex", "x").is_none());
    }

    #[test]
    fn partial_file_keeps_defaults() {
        let c: Config = toml::from_str("theme = \"iris\"").unwrap();
        assert_eq!(c.theme, "iris");
        assert_eq!(c.layout.compact_width, 170);
    }
}
