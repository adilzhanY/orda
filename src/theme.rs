use ratatui::style::Color;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub text: Color,
    pub bright: Color,
    pub dim: Color,
    pub line: Color,
    pub badge: Color,
    pub claude: Color,
    pub codex: Color,
    pub gemini: Color,
    pub fable: Color,
    pub ask: Color,
    pub ok: Color,
    pub bad: Color,
}

const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const ORDA: Theme = Theme {
    bg: rgb(0x1b1d22),
    text: rgb(0xc9cdd4),
    bright: rgb(0xeef0f3),
    dim: rgb(0x6b7280),
    line: rgb(0x3a3f48),
    badge: rgb(0x262a31),
    claude: rgb(0xe8855a),
    codex: rgb(0x7aa7ff),
    gemini: rgb(0x5fd3c4),
    fable: rgb(0xa990e0),
    ask: rgb(0xf2c14e),
    ok: rgb(0x7ad67a),
    bad: rgb(0xef6b6b),
};

// Iris has no accent colours: greys and white, red only for danger.
const IRIS: Theme = Theme {
    bg: rgb(0x000000),
    text: rgb(0xa6a6ac),
    bright: rgb(0xf2f2f2),
    dim: rgb(0x6e6e73),
    line: rgb(0x232326),
    badge: rgb(0x161618),
    claude: rgb(0xf2f2f2),
    codex: rgb(0xf2f2f2),
    gemini: rgb(0xf2f2f2),
    fable: rgb(0xf2f2f2),
    ask: rgb(0xf2f2f2),
    ok: rgb(0xf2f2f2),
    bad: rgb(0xff6961),
};

impl Theme {
    pub fn build(name: &str, overrides: &HashMap<String, String>) -> Self {
        let mut t = if name == "iris" { IRIS } else { ORDA };
        for (key, value) in overrides {
            let Some(c) = parse(value) else { continue };
            let slot = match key.as_str() {
                "bg" => &mut t.bg,
                "text" => &mut t.text,
                "bright" => &mut t.bright,
                "dim" => &mut t.dim,
                "line" => &mut t.line,
                "badge" => &mut t.badge,
                "claude" => &mut t.claude,
                "codex" => &mut t.codex,
                "gemini" => &mut t.gemini,
                "fable" => &mut t.fable,
                "ask" => &mut t.ask,
                "ok" => &mut t.ok,
                "bad" => &mut t.bad,
                _ => continue,
            };
            *slot = c;
        }
        t
    }

    /// Accent for an agent: Fable models get the advisor colour, the rest their vendor's.
    pub fn accent(&self, vendor: &str, model: &str) -> Color {
        if model.contains("fable") {
            return self.fable;
        }
        match vendor {
            "codex" => self.codex,
            "gemini" => self.gemini,
            _ => self.claude,
        }
    }
}

fn parse(v: &str) -> Option<Color> {
    if v == "none" {
        return Some(Color::Reset);
    }
    let hex = u32::from_str_radix(v.strip_prefix('#')?, 16).ok()?;
    (v.len() == 7).then(|| rgb(hex))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_apply() {
        let mut o = HashMap::new();
        o.insert("claude".into(), "#010203".into());
        o.insert("bg".into(), "none".into());
        o.insert("ok".into(), "garbage".into());
        let t = Theme::build("orda", &o);
        assert_eq!(t.claude, Color::Rgb(1, 2, 3));
        assert_eq!(t.bg, Color::Reset);
        assert_eq!(t.ok, ORDA.ok);
    }

    #[test]
    fn iris_is_black() {
        assert_eq!(
            Theme::build("iris", &HashMap::new()).bg,
            Color::Rgb(0, 0, 0)
        );
    }
}
