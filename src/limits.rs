//! Plan limits, read from the limit-watch daemon's state file
//! (~/.local/state/limit-watch/state.json). Claude's windows are also updated
//! live from the rate limit events in its stream.

use serde_json::Value;

#[derive(Clone, Default)]
pub struct Service {
    /// orda's vendor key: claude, codex, gemini
    pub vendor: String,
    pub error: Option<String>,
    pub windows: Vec<Window>,
}

#[derive(Clone)]
pub struct Window {
    pub key: String,
    pub label: String,
    pub used_pct: f64,
    pub resets_at: i64,
}

pub fn load() -> Vec<Service> {
    let path = crate::config::home().join(".local/state/limit-watch/state.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return vec![];
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return vec![];
    };
    // limit-watch names: claude, codex, antigravity (Gemini through Google AI Pro)
    [
        ("claude", "claude"),
        ("codex", "codex"),
        ("antigravity", "gemini"),
    ]
    .into_iter()
    .filter_map(|(key, vendor)| {
        let s = v["services"].get(key)?;
        let mut windows: Vec<Window> = s["windows"]
            .as_object()?
            .iter()
            .filter(|(k, _)| !k.starts_with("3p"))
            .map(|(k, w)| Window {
                key: k.clone(),
                label: short_label(k),
                used_pct: w["used_pct"].as_f64().unwrap_or(0.0),
                resets_at: w["resets_at"].as_i64().unwrap_or(0),
            })
            .collect();
        windows.sort_by_key(|w| (w.label != "5h", w.label != "7d"));
        let error = (!s["ok"].as_bool().unwrap_or(true))
            .then(|| s["error_kind"].as_str().unwrap_or("error").to_string());
        Some(Service {
            vendor: vendor.into(),
            error,
            windows,
        })
    })
    .collect()
}

fn short_label(key: &str) -> String {
    match key {
        "five_hour" | "gemini_5h" => "5h".into(),
        "seven_day" | "gemini_weekly" => "7d".into(),
        k if k.starts_with("seven_day_") => k[10..].to_string(),
        k => k.into(),
    }
}

/// Apply a live window from an agent stream.
pub fn update(
    services: &mut Vec<Service>,
    vendor: &str,
    window: &str,
    used_pct: f64,
    resets_at: i64,
) {
    let idx = match services.iter().position(|s| s.vendor == vendor) {
        Some(i) => i,
        None => {
            services.push(Service {
                vendor: vendor.into(),
                ..Default::default()
            });
            services.len() - 1
        }
    };
    let s = &mut services[idx];
    s.error = None;
    match s.windows.iter_mut().find(|w| w.key == window) {
        Some(w) => {
            w.used_pct = used_pct;
            w.resets_at = resets_at;
        }
        None => s.windows.push(Window {
            key: window.into(),
            label: short_label(window),
            used_pct,
            resets_at,
        }),
    }
}

/// "1h52", "3d", "now"
pub fn until(resets_at: i64, now: i64) -> String {
    let s = resets_at - now;
    if s <= 0 {
        "now".into()
    } else if s < 3600 {
        format!("{}m", s / 60)
    } else if s < 86400 {
        format!("{}h{:02}", s / 3600, s % 3600 / 60)
    } else {
        format!("{}d{}h", s / 86400, s % 86400 / 3600)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn until_formats() {
        assert_eq!(until(100, 200), "now");
        assert_eq!(until(600, 0), "10m");
        assert_eq!(until(6720, 0), "1h52");
        assert_eq!(until(90000, 0), "1d1h");
    }
}
