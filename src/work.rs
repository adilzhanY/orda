//! Where each agent works. Agents that write code or check commits get their own git
//! worktree on a branch named after them (`wt/<name>`), outside the project, so
//! builders work in parallel without touching each other's files or the owner's
//! checkout. The boss and the doc keepers work in the project itself.

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr)
            .trim()
            .lines()
            .last()
            .unwrap_or("git failed")
            .to_string())
    }
}

/// Roles that work in their own worktree.
pub fn in_worktree(role: &str) -> bool {
    matches!(
        role,
        "builder" | "tester" | "ripple" | "aegis" | "referee" | "customs" | "reviewer" | "designer"
    )
}

/// Make `dir` a git repository with at least one commit, so branches and worktrees work.
/// Returns true when it had to create one.
pub fn ensure_repo(dir: &Path) -> Result<bool, String> {
    if git(dir, &["rev-parse", "--verify", "HEAD"]).is_ok() {
        return Ok(false);
    }
    if git(dir, &["rev-parse", "--git-dir"]).is_err() {
        git(dir, &["init", "-q", "-b", "main"])?;
    }
    let mut args = identity(dir);
    args.extend(["commit", "-q", "--allow-empty", "-m", "Start"].map(String::from));
    git(dir, &args.iter().map(String::as_str).collect::<Vec<_>>())?;
    Ok(true)
}

/// `-c user.name=... -c user.email=...` when git has no identity configured, so orda's
/// own commits and merges never fail on a fresh machine.
fn identity(dir: &Path) -> Vec<String> {
    if git(dir, &["config", "user.email"]).is_ok_and(|e| !e.is_empty()) {
        return vec![];
    }
    ["-c", "user.name=orda", "-c", "user.email=orda@localhost"]
        .map(String::from)
        .to_vec()
}

/// The folder that holds this project's worktrees: ~/.local/share/orda/wt/<project>-<hash>.
pub fn base(project: &Path) -> PathBuf {
    let s = project.display().to_string();
    // FNV-1a: a short stable id so two projects with the same folder name do not collide
    let h = s.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    });
    let name = project
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    crate::config::home().join(format!(".local/share/orda/wt/{name}-{:08x}", h as u32))
}

pub enum Job<'a> {
    /// Work on a task: continue the agent's branch if it still holds unmerged work,
    /// otherwise start it fresh from the main branch.
    Task { main: &'a str },
    /// Check a commit: the agent's branch is set to that commit.
    Check { commit: &'a str },
}

/// Get the agent's worktree ready for a job and return its path.
pub fn prepare(project: &Path, agent: &str, job: Job) -> Result<PathBuf, String> {
    let branch = format!("wt/{agent}");
    let path = base(project).join(agent);
    let start = match &job {
        Job::Task { main } => main.to_string(),
        Job::Check { commit } => commit.to_string(),
    };
    let registered = git(project, &["worktree", "list", "--porcelain"])?
        .lines()
        .any(|l| {
            l.strip_prefix("worktree ")
                .is_some_and(|p| Path::new(p) == path)
        });
    if !registered {
        if path.exists() {
            std::fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
        }
        let _ = git(project, &["worktree", "prune"]);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let p = path.display().to_string();
        let exists = git(project, &["rev-parse", "--verify", "--quiet", &branch]).is_ok();
        if exists {
            git(project, &["worktree", "add", "-q", "-f", &p, &branch])?;
        } else {
            git(
                project,
                &["worktree", "add", "-q", "-b", &branch, &p, &start],
            )?;
        }
    }
    match job {
        Job::Check { commit } => {
            git(&path, &["checkout", "-q", "-f", "-B", &branch, commit])?;
            git(&path, &["clean", "-q", "-fd"])?;
        }
        Job::Task { main } => {
            let merged = git(project, &["merge-base", "--is-ancestor", &branch, main]).is_ok();
            let clean = git(&path, &["status", "--porcelain"])?.is_empty();
            if merged && clean {
                // everything it did is in main already: start from the latest main
                git(&path, &["checkout", "-q", "-B", &branch, main])?;
            } else if clean {
                // unmerged work: keep it, but bring in what landed on main meanwhile
                let mut args = identity(&path);
                args.extend(["merge", "-q", "--no-edit", main].map(String::from));
                let _ = git(&path, &args.iter().map(String::as_str).collect::<Vec<_>>());
            }
        }
    }
    Ok(path)
}

/// The branch a commit was made on: the one whose reflog says "commit" for it. Checkers'
/// branches are reset to the commits they check, so "contains" alone would name them too.
pub fn author(project: &Path, commit: &str) -> Option<String> {
    let branches = git(
        project,
        &["branch", "--format=%(refname:short)", "--contains", commit],
    )
    .ok()?;
    let branches: Vec<&str> = branches
        .lines()
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .collect();
    for b in &branches {
        let log = git(
            project,
            &[
                "reflog",
                "show",
                "--format=%h %gs",
                &format!("refs/heads/{b}"),
            ],
        )
        .unwrap_or_default();
        let made_here = log.lines().any(|l| {
            let (h, what) = l.split_once(' ').unwrap_or(("", ""));
            (h.starts_with(commit) || commit.starts_with(h))
                && !h.is_empty()
                && what.starts_with("commit")
        });
        if made_here {
            return Some(b.to_string());
        }
    }
    branches.first().map(|b| b.to_string())
}

/// The agent whose `wt/` branch a commit was made on.
pub fn owners(project: &Path, commit: &str) -> Vec<String> {
    author(project, commit)
        .and_then(|b| b.strip_prefix("wt/").map(String::from))
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("orda-work-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn commit(dir: &Path, file: &str, text: &str) -> String {
        std::fs::write(dir.join(file), text).unwrap();
        git(dir, &["add", "."]).unwrap();
        git(
            dir,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "commit",
                "-qm",
                file,
            ],
        )
        .unwrap();
        git(dir, &["rev-parse", "--short", "HEAD"]).unwrap()
    }

    #[test]
    fn worktrees_follow_the_job() {
        let dir = repo("flow");
        assert!(ensure_repo(&dir).unwrap(), "an empty folder becomes a repo");
        assert!(!ensure_repo(&dir).unwrap());
        let main = git(&dir, &["branch", "--show-current"]).unwrap();

        // a builder works on its own branch in its own folder
        let wt = prepare(&dir, "builder", Job::Task { main: &main }).unwrap();
        assert!(wt.starts_with(base(&dir)) && wt.ends_with("builder"));
        let c = commit(&wt, "a.txt", "one");
        assert_eq!(owners(&dir, &c), ["builder"]);
        assert!(
            !dir.join("a.txt").exists(),
            "the owner's checkout is untouched"
        );

        // a checker gets that exact commit, and the commit still belongs to the builder
        let chk = prepare(&dir, "tester", Job::Check { commit: &c }).unwrap();
        assert_eq!(std::fs::read_to_string(chk.join("a.txt")).unwrap(), "one");
        assert_eq!(owners(&dir, &c), ["builder"]);

        // unmerged work survives the next task
        let wt = prepare(&dir, "builder", Job::Task { main: &main }).unwrap();
        assert!(wt.join("a.txt").exists());

        // once merged, the next task starts from main, which now has it
        git(
            &dir,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "merge",
                "-q",
                "--no-ff",
                "-m",
                "merge",
                "wt/builder",
            ],
        )
        .unwrap();
        commit(&dir, "b.txt", "from main");
        let wt = prepare(&dir, "builder", Job::Task { main: &main }).unwrap();
        assert!(wt.join("a.txt").exists() && wt.join("b.txt").exists());

        for a in ["builder", "tester"] {
            let _ = git(
                &dir,
                &[
                    "worktree",
                    "remove",
                    "-f",
                    &base(&dir).join(a).display().to_string(),
                ],
            );
        }
        let _ = std::fs::remove_dir_all(base(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
