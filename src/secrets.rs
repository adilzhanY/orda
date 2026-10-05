//! orda's own secret and personal data scanner: plain matching over the lines a
//! commit adds, no model, no tokens. aegis judges what it finds.

#[derive(Debug, PartialEq, Clone)]
pub struct Finding {
    pub file: String,
    pub line: usize,
    pub what: &'static str,
}

/// Token prefixes that only appear in real credentials, the characters that may
/// follow, and how many of them make it a credential.
const PREFIXES: &[(&str, &str, usize, &str)] = &[
    ("AKIA", "upper_digit", 16, "AWS access key"),
    ("ASIA", "upper_digit", 16, "AWS temporary key"),
    ("sk-ant-", "token", 32, "Anthropic API key"),
    ("sk-proj-", "token", 32, "OpenAI API key"),
    ("sk-", "token", 40, "API secret key"),
    ("ghp_", "alnum", 36, "GitHub token"),
    ("gho_", "alnum", 36, "GitHub token"),
    ("github_pat_", "token", 40, "GitHub token"),
    ("glpat-", "token", 20, "GitLab token"),
    ("xoxb-", "token", 20, "Slack token"),
    ("xoxp-", "token", 20, "Slack token"),
    ("AIza", "token", 35, "Google API key"),
    ("sk_live_", "alnum", 20, "Stripe secret key"),
    ("rk_live_", "alnum", 20, "Stripe key"),
];

/// Scan a unified diff (`git show <commit>`); only added lines count.
pub fn scan_diff(diff: &str) -> Vec<Finding> {
    let mut out = vec![];
    let mut file = String::new();
    let mut line_no = 0usize;
    for l in diff.lines() {
        if let Some(f) = l.strip_prefix("+++ b/") {
            file = f.to_string();
            let name = f.rsplit('/').next().unwrap_or(f);
            let risky = name == ".env"
                || name.starts_with(".env.")
                || name.ends_with(".pem")
                || name.ends_with(".key")
                || name == "id_rsa"
                || name == "id_ed25519";
            if risky && !name.ends_with(".example") && !name.ends_with(".sample") {
                out.push(Finding {
                    file: file.clone(),
                    line: 0,
                    what: "secrets file committed",
                });
            }
            continue;
        }
        if let Some(h) = l.strip_prefix("@@") {
            // @@ -a,b +c,d @@: the next added line is line c
            line_no = h
                .split('+')
                .nth(1)
                .and_then(|s| s.split([',', ' ']).next())
                .and_then(|n| n.parse().ok())
                .unwrap_or(1);
            continue;
        }
        if l.starts_with("+++") || l.starts_with("---") {
            continue;
        }
        if let Some(added) = l.strip_prefix('+') {
            for what in scan_line(added) {
                out.push(Finding {
                    file: file.clone(),
                    line: line_no,
                    what,
                });
            }
            line_no += 1;
        } else if !l.starts_with('-') {
            line_no += 1;
        }
    }
    out
}

pub fn scan_line(line: &str) -> Vec<&'static str> {
    let mut found = vec![];
    for (prefix, class, min, what) in PREFIXES {
        let mut from = 0;
        while let Some(i) = line[from..].find(prefix) {
            let at = from + i;
            let start_ok = at == 0 || !line[..at].ends_with(|c: char| c.is_ascii_alphanumeric());
            // the generic "sk-" rule leaves keys a more specific rule names
            let named = *prefix == "sk-"
                && ["sk-ant-", "sk-proj-"]
                    .iter()
                    .any(|p| line[at..].starts_with(p));
            let tail = line[at + prefix.len()..]
                .chars()
                .take_while(|c| fits(*c, class))
                .count();
            if start_ok && !named && tail >= *min {
                found.push(*what);
                break;
            }
            from = at + prefix.len();
        }
    }
    if line.contains("PRIVATE KEY-----") {
        found.push("private key");
    }
    if password_literal(line) {
        found.push("password or secret in code");
    }
    if email(line) {
        found.push("email address");
    }
    if card_number(line) {
        found.push("card number");
    }
    found.dedup();
    found
}

fn fits(c: char, class: &str) -> bool {
    match class {
        "upper_digit" => c.is_ascii_uppercase() || c.is_ascii_digit(),
        "alnum" => c.is_ascii_alphanumeric(),
        _ => c.is_ascii_alphanumeric() || c == '-' || c == '_',
    }
}

/// `password = "hunter22"`, `api_key: 'abc...'`: a quoted literal assigned to a secret-looking name.
fn password_literal(line: &str) -> bool {
    let lower = line.to_lowercase();
    let Some(k) = [
        "password",
        "passwd",
        "secret",
        "api_key",
        "apikey",
        "access_token",
        "auth_token",
    ]
    .iter()
    .find_map(|k| lower.find(k)) else {
        return false;
    };
    let rest = &line[k..];
    let Some(op) = rest.find(['=', ':']) else {
        return false;
    };
    let value = rest[op + 1..].trim_start();
    let Some(q) = value.chars().next().filter(|c| *c == '"' || *c == '\'') else {
        return false;
    };
    let lit: String = value[1..].chars().take_while(|c| *c != q).collect();
    let placeholder = [
        "xxx", "example", "changeme", "your", "<", "${", "env", "***", "dummy", "test", "redacted",
    ];
    lit.len() >= 8 && !placeholder.iter().any(|p| lit.to_lowercase().contains(p))
}

fn email(line: &str) -> bool {
    let ok = |c: char| c.is_ascii_alphanumeric() || "._%+-".contains(c);
    for (i, _) in line.match_indices('@') {
        let user = line[..i].chars().rev().take_while(|c| ok(*c)).count();
        let domain: String = line[i + 1..].chars().take_while(|c| ok(*c)).collect();
        let domain = domain.trim_end_matches('.').to_lowercase();
        let harmless = [
            "example.com",
            "example.org",
            "example.net",
            "localhost",
            "noreply",
            "test.",
            ".test",
            "anthropic.com",
            "users.noreply.github.com",
        ];
        if user > 0
            && domain.contains('.')
            && !domain.ends_with(".rs")
            && !harmless.iter().any(|h| domain.contains(h))
        {
            return true;
        }
    }
    false
}

/// 13 to 19 digits (spaces or dashes allowed between) that pass the Luhn check.
fn card_number(line: &str) -> bool {
    let mut digits: Vec<u32> = vec![];
    for c in line.chars().chain(std::iter::once('x')) {
        match c {
            '0'..='9' => digits.push(c as u32 - '0' as u32),
            ' ' | '-' if !digits.is_empty() => {}
            _ => {
                if (13..=19).contains(&digits.len())
                    && luhn(&digits)
                    && digits.iter().any(|d| *d != digits[0])
                {
                    return true;
                }
                digits.clear();
            }
        }
    }
    false
}

fn luhn(d: &[u32]) -> bool {
    let sum: u32 = d
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &x)| {
            if i % 2 == 1 {
                if x * 2 > 9 { x * 2 - 9 } else { x * 2 }
            } else {
                x
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

#[cfg(test)]
mod tests {
    use super::*;

    // assembled at run time so this file never holds a real-looking credential
    fn fake(prefix: &str, n: usize) -> String {
        format!("{prefix}{}", &"A1b2C3d4".repeat(n / 8 + 1)[..n])
    }

    #[test]
    fn finds_credentials() {
        assert_eq!(
            scan_line(&format!("let k = \"{}\";", fake("sk-ant-", 40))),
            ["Anthropic API key"]
        );
        assert_eq!(
            scan_line(&format!("KEY={}", fake("ghp_", 36))),
            ["GitHub token"]
        );
        assert_eq!(
            scan_line(&format!("id: {}", "AKIA".to_string() + &"Q7".repeat(8))),
            ["AWS access key"]
        );
        assert_eq!(
            scan_line(&format!("-----BEGIN RSA {}-----", "PRIVATE KEY")),
            ["private key"]
        );
        assert_eq!(
            scan_line("db_password = \"s3cr3t-pa55w0rd\""),
            ["password or secret in code"]
        );
    }

    #[test]
    fn finds_personal_data() {
        assert_eq!(
            scan_line(&format!("owner: {}@{}", "jane.doe", "gmail.com")),
            ["email address"]
        );
        assert_eq!(
            scan_line(&format!("card = \"{}\"", "4111 1111 1111 1111")),
            ["card number"]
        );
    }

    #[test]
    fn leaves_ordinary_code_alone() {
        for l in [
            "let task = \"sk-\";",
            "password = env::var(\"DB_PASSWORD\")",
            "password: \"changeme\"",
            "contact: someone@example.com",
            "use crate::app::App; // see app.rs@line",
            "let big = 1234567890123;",
            "risk-assessment and desk-top",
        ] {
            assert!(scan_line(l).is_empty(), "{l}: {:?}", scan_line(l));
        }
    }

    #[test]
    fn diff_lines_and_files() {
        let diff = format!(
            "diff --git a/.env b/.env\n+++ b/.env\n@@ -0,0 +1,2 @@\n+DEBUG=1\n+TOKEN={}\ndiff --git a/src/a.rs b/src/a.rs\n+++ b/src/a.rs\n@@ -10,3 +10,4 @@\n fn x() {{}}\n-old\n+mail = \"{}@{}\"\n",
            fake("xoxb-", 24),
            "bob",
            "corp.io"
        );
        let f = scan_diff(&diff);
        assert!(f.contains(&Finding {
            file: ".env".into(),
            line: 0,
            what: "secrets file committed"
        }));
        assert!(f.contains(&Finding {
            file: ".env".into(),
            line: 2,
            what: "Slack token"
        }));
        assert!(f.contains(&Finding {
            file: "src/a.rs".into(),
            line: 11,
            what: "email address"
        }));
    }
}
