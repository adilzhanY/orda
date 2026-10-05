//! A small read-only snapshot of the repo orda runs in.

use std::process::Command;

#[derive(Default, Clone)]
pub struct Snapshot {
    pub branch: String,
    pub ahead: u32,
    pub worktrees: Vec<Worktree>,
    pub commits: Vec<Commit>,
    /// Files of the newest commit: (status, path, added, removed).
    pub head_files: Vec<(String, String, u32, u32)>,
}

#[derive(Clone)]
pub struct Worktree {
    pub name: String,
    pub files: u32,
    pub added: u32,
    pub removed: u32,
    /// Commits on its branch that the main branch does not have yet.
    pub ahead: u32,
}

#[derive(Clone)]
pub struct Commit {
    pub hash: String,
    pub subject: String,
    pub when: String,
    /// The main branch when it is in main, otherwise the branch that holds it.
    pub on: String,
    /// Set by orda for commits its agents made: "testing 14/20", "in review", ...
    pub status: Option<(String, Tone)>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tone {
    Busy,
    Good,
    Bad,
    Info,
}

fn git(dir: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn snapshot(dir: &str) -> Option<Snapshot> {
    let branch = git(dir, &["branch", "--show-current"])?.trim().to_string();
    let ahead = git(dir, &["rev-list", "--count", "@{upstream}..HEAD"])
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    // every branch: the team's work sits on wt/ branches until the boss merges it
    let commits = git(
        dir,
        &[
            "log",
            "--branches",
            "--no-merges",
            "-n",
            "12",
            "--pretty=%h%x09%s%x09%cr",
        ],
    )
    .unwrap_or_default()
    .lines()
    .filter_map(|l| {
        let mut p = l.splitn(3, '\t');
        let hash: String = p.next()?.into();
        let on = if git(dir, &["merge-base", "--is-ancestor", &hash, "HEAD"]).is_some() {
            branch.clone()
        } else {
            crate::work::author(std::path::Path::new(dir), &hash).unwrap_or_default()
        };
        Some(Commit {
            hash,
            subject: p.next()?.into(),
            when: p.next()?.replace(" ago", ""),
            on,
            status: None,
        })
    })
    .collect();
    let head_files = git(dir, &["show", "--numstat", "--format=", "HEAD"])
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut p = l.split('\t');
            let added = p.next()?.parse().unwrap_or(0);
            let removed = p.next()?.parse().unwrap_or(0);
            Some((String::new(), p.next()?.to_string(), added, removed))
        })
        .collect();
    let worktrees = git(dir, &["worktree", "list", "--porcelain"])
        .unwrap_or_default()
        .split("\n\n")
        .filter_map(|block| {
            let path = block.lines().find_map(|l| l.strip_prefix("worktree "))?;
            let name = block
                .lines()
                .find_map(|l| l.strip_prefix("branch refs/heads/"))
                .unwrap_or(path)
                .to_string();
            let (mut files, mut added, mut removed) = (0, 0, 0);
            for l in git(path, &["diff", "--numstat", "HEAD"])
                .unwrap_or_default()
                .lines()
            {
                let mut p = l.split('\t');
                files += 1;
                added += p.next().and_then(|n| n.parse().ok()).unwrap_or(0);
                removed += p.next().and_then(|n| n.parse().ok()).unwrap_or(0);
            }
            let ahead = git(dir, &["rev-list", "--count", &format!("HEAD..{name}")])
                .and_then(|n| n.trim().parse().ok())
                .unwrap_or(0);
            Some(Worktree {
                name,
                files,
                added,
                removed,
                ahead,
            })
        })
        .collect();
    Some(Snapshot {
        branch,
        ahead,
        worktrees,
        commits,
        head_files,
    })
}

/// The diff a commit added, for the secret scanner.
pub fn show(dir: &str, hash: &str) -> String {
    git(dir, &["show", "--format=", "--unified=0", hash]).unwrap_or_default()
}

/// The files a commit changed.
pub fn files(dir: &str, hash: &str) -> Vec<String> {
    git(dir, &["show", "--name-only", "--format=", hash])
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

/// Recent commits on every local branch, merges left out: what the team made.
pub fn recent_all(dir: &str) -> Vec<(String, String)> {
    git(
        dir,
        &[
            "log",
            "--branches",
            "--no-merges",
            "-n",
            "60",
            "--pretty=%h%x09%s",
        ],
    )
    .unwrap_or_default()
    .lines()
    .filter_map(|l| {
        l.split_once('\t')
            .map(|(h, s)| (h.to_string(), s.to_string()))
    })
    .collect()
}
