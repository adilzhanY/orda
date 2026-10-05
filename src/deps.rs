//! Free check for customs: which dependencies a commit adds, and whether each one
//! exists in its registry. Invented package names are common in AI-written code
//! and attackers register them, so a missing package is held at once.

use std::collections::BTreeSet;
use std::process::Command;

/// Manifests and lockfiles: a commit touching one of these goes to customs.
pub fn is_manifest(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    [
        "Cargo.toml",
        "Cargo.lock",
        "package.json",
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "bun.lockb",
        "pyproject.toml",
        "poetry.lock",
        "uv.lock",
        "Pipfile",
        "Pipfile.lock",
        "go.mod",
        "go.sum",
        "Gemfile",
        "Gemfile.lock",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "composer.json",
    ]
    .contains(&name)
        || (name.starts_with("requirements") && name.ends_with(".txt"))
}

/// The dependency names a manifest declares, by ecosystem.
pub fn names(path: &str, text: &str) -> Option<(&'static str, BTreeSet<String>)> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let set: BTreeSet<String> = match name {
        "Cargo.toml" => {
            let v: toml::Value = toml::from_str(text).ok()?;
            ["dependencies", "dev-dependencies", "build-dependencies"]
                .iter()
                .filter_map(|k| v.get(k).and_then(|d| d.as_table()))
                .flat_map(|t| {
                    t.iter().map(|(k, d)| {
                        d.get("package")
                            .and_then(|p| p.as_str())
                            .unwrap_or(k)
                            .to_string()
                    })
                })
                .collect()
        }
        "package.json" => {
            let v: serde_json::Value = serde_json::from_str(text).ok()?;
            [
                "dependencies",
                "devDependencies",
                "peerDependencies",
                "optionalDependencies",
            ]
            .iter()
            .filter_map(|k| v.get(k).and_then(|d| d.as_object()))
            .flat_map(|o| o.keys().cloned())
            .collect()
        }
        "pyproject.toml" => {
            let v: toml::Value = toml::from_str(text).ok()?;
            let mut s: BTreeSet<String> = v
                .get("project")
                .and_then(|p| p.get("dependencies"))
                .and_then(|d| d.as_array())
                .into_iter()
                .flatten()
                .filter_map(|d| d.as_str().map(py_name))
                .collect();
            if let Some(t) = v
                .get("tool")
                .and_then(|t| t.get("poetry"))
                .and_then(|p| p.get("dependencies"))
                .and_then(|d| d.as_table())
            {
                s.extend(t.keys().filter(|k| *k != "python").cloned());
            }
            s
        }
        n if n.starts_with("requirements") && n.ends_with(".txt") => text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('-'))
            .map(py_name)
            .collect(),
        "go.mod" => text
            .lines()
            .map(|l| l.trim().trim_start_matches("require").trim())
            .filter(|l| {
                l.contains('.')
                    && l.contains(' ')
                    && !l.starts_with("module")
                    && !l.starts_with("go ")
            })
            .filter_map(|l| l.split_whitespace().next().map(String::from))
            .collect(),
        _ => return None,
    };
    let eco = match name {
        "Cargo.toml" => "crates.io",
        "package.json" => "npm",
        "go.mod" => "go",
        _ => "pypi",
    };
    Some((eco, set))
}

fn py_name(spec: &str) -> String {
    spec.split(|c: char| "<>=!~;[ (".contains(c))
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase()
}

/// Dependencies a commit adds: in a manifest after it, not before it.
pub fn added(dir: &str, hash: &str, files: &[String]) -> Vec<(&'static str, String)> {
    let show = |rev: &str| {
        Command::new("git")
            .args(["-C", dir, "show", rev])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    };
    let mut out = vec![];
    for f in files {
        let Some((eco, after)) = names(f, &show(&format!("{hash}:{f}"))) else {
            continue;
        };
        let before = names(f, &show(&format!("{hash}^:{f}")))
            .map(|x| x.1)
            .unwrap_or_default();
        out.extend(after.difference(&before).map(|n| (eco, n.clone())));
    }
    out
}

pub fn registry_url(eco: &str, name: &str) -> String {
    match eco {
        "crates.io" => format!("https://crates.io/api/v1/crates/{name}"),
        "npm" => format!("https://registry.npmjs.org/{}", name.replace('/', "%2F")),
        "go" => {
            // the Go proxy writes capitals as "!" plus the lower case letter
            let path: String = name
                .chars()
                .flat_map(|c| {
                    if c.is_ascii_uppercase() {
                        vec!['!', c.to_ascii_lowercase()]
                    } else {
                        vec![c]
                    }
                })
                .collect();
            format!("https://proxy.golang.org/{path}/@v/list")
        }
        _ => format!("https://pypi.org/pypi/{name}/json"),
    }
}

/// Some(true) exists, Some(false) the registry says no, None could not tell (offline, rate limited).
pub fn exists(eco: &str, name: &str) -> Option<bool> {
    let out = Command::new("curl")
        .args([
            "-s",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "-m",
            "8",
            "-A",
            "orda (dependency check)",
            &registry_url(eco, name),
        ])
        .output()
        .ok()?;
    match String::from_utf8_lossy(&out.stdout).trim() {
        "200" => Some(true),
        "404" | "410" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_are_read() {
        let cargo = "[package]\nname = \"x\"\n[dependencies]\nserde = \"1\"\nfoo = { version = \"2\", package = \"bar\" }\n[dev-dependencies]\ninsta = \"1\"";
        assert_eq!(
            names("Cargo.toml", cargo)
                .unwrap()
                .1
                .into_iter()
                .collect::<Vec<_>>(),
            ["bar", "insta", "serde"]
        );
        let pkg =
            r#"{"name":"a","dependencies":{"react":"^19"},"devDependencies":{"@types/node":"*"}}"#;
        assert_eq!(
            names("web/package.json", pkg)
                .unwrap()
                .1
                .into_iter()
                .collect::<Vec<_>>(),
            ["@types/node", "react"]
        );
        let req = "# deps\nRequests>=2.0\nnumpy==2.1 ; python_version>'3.9'\n-r other.txt\n";
        assert_eq!(
            names("requirements-dev.txt", req)
                .unwrap()
                .1
                .into_iter()
                .collect::<Vec<_>>(),
            ["numpy", "requests"]
        );
        let gomod = "module x\n\ngo 1.23\n\nrequire (\n\tgithub.com/BurntSushi/toml v1.4.0\n)\nrequire golang.org/x/sync v0.8.0\n";
        assert_eq!(
            names("go.mod", gomod)
                .unwrap()
                .1
                .into_iter()
                .collect::<Vec<_>>(),
            ["github.com/BurntSushi/toml", "golang.org/x/sync"]
        );
        assert!(
            is_manifest("app/Cargo.lock")
                && is_manifest("requirements.txt")
                && !is_manifest("src/main.rs")
        );
    }

    #[test]
    fn registry_urls() {
        assert_eq!(
            registry_url("npm", "@types/node"),
            "https://registry.npmjs.org/@types%2Fnode"
        );
        assert_eq!(
            registry_url("go", "github.com/BurntSushi/toml"),
            "https://proxy.golang.org/github.com/!burnt!sushi/toml/@v/list"
        );
    }

    #[test]
    fn added_compares_with_the_parent() {
        let dir = std::env::temp_dir().join(format!("orda-deps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .output()
                .unwrap();
        };
        let commit = |msg: &str| {
            git(&[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "commit",
                "-qam",
                msg,
            ])
        };
        git(&["init", "-q"]);
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"x\"\n[dependencies]\nserde = \"1\"\n",
        )
        .unwrap();
        git(&["add", "."]);
        commit("one");
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"x\"\n[dependencies]\nserde = \"1\"\nratatui-flexbox = \"0.3\"\n",
        )
        .unwrap();
        commit("two");
        let got = added(dir.to_str().unwrap(), "HEAD", &["Cargo.toml".into()]);
        assert_eq!(got, [("crates.io", "ratatui-flexbox".to_string())]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Real registries: `cargo test registry_live -- --ignored`
    #[test]
    #[ignore]
    fn registry_live() {
        assert_eq!(exists("crates.io", "serde"), Some(true));
        assert_eq!(
            exists("crates.io", "ratatui-flexbox-does-not-exist-9q"),
            Some(false)
        );
        assert_eq!(exists("npm", "@types/node"), Some(true));
        assert_eq!(exists("pypi", "requests"), Some(true));
    }
}
