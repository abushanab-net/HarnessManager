//! Grouping sessions by project across tools and accounts: a session's folder is normalized, mapped to
//! its git repository root, and keyed by the repository's remote URL when there is one.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::registry::ProjectAlias;

#[derive(Debug, Clone, Serialize)]
pub struct ProjectInfo {
    pub key: String,
    pub name: String,
    pub root: String,
    pub remote: Option<String>,
}

pub fn normalize(path: &str) -> String {
    let mut s = path.trim().to_string();
    if cfg!(windows) {
        s = s.replace('/', "\\");
        // "/c/Users" (Git Bash style) and "C:/" forms
        if s.len() > 2 && s.starts_with('\\') && s.as_bytes()[2] == b'\\' && s.as_bytes()[1].is_ascii_alphabetic() {
            s = format!("{}:{}", s[1..2].to_ascii_uppercase(), &s[2..]);
        }
        if s.len() >= 2 && s.as_bytes()[1] == b':' {
            s = format!("{}{}", s[..1].to_ascii_uppercase(), &s[1..]);
        }
    }
    while s.len() > 3 && (s.ends_with('\\') || s.ends_with('/')) {
        s.pop();
    }
    s
}

fn git_dir(root: &Path) -> Option<PathBuf> {
    let g = root.join(".git");
    if g.is_dir() {
        return Some(g);
    }
    // worktrees and submodules: ".git" is a file pointing at the real folder
    let text = std::fs::read_to_string(&g).ok()?;
    let p = PathBuf::from(text.trim().strip_prefix("gitdir:")?.trim());
    let p = if p.is_absolute() { p } else { root.join(p) };
    let common = std::fs::read_to_string(p.join("commondir")).ok().map(|c| p.join(c.trim()));
    Some(common.unwrap_or(p))
}

pub fn git_root(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(d) = cur {
        if d.join(".git").exists() {
            return Some(d.to_path_buf());
        }
        cur = d.parent();
    }
    None
}

/// `git@github.com:owner/repo.git` and `https://user@github.com/owner/repo` both become `github.com/owner/repo`.
pub fn normalize_remote(url: &str) -> String {
    let mut s = url.trim().to_string();
    for p in ["https://", "http://", "ssh://", "git://"] {
        if let Some(r) = s.strip_prefix(p) { s = r.to_string(); }
    }
    let host_end = s.find('/').unwrap_or(s.len());
    if let Some(i) = s[..host_end].find('@') { s = s[i + 1..].to_string(); }
    if !url.contains("://") {
        s = s.replacen(':', "/", 1);
    } else if let Some((host, rest)) = s.split_once('/') {
        // ssh://git@host:7999/o/r and git@host:o/r are the same repository
        if let Some((h, port)) = host.split_once(':') { if port.chars().all(|c| c.is_ascii_digit()) { s = format!("{h}/{rest}"); } }
    }
    s.trim_end_matches('/').trim_end_matches(".git").to_ascii_lowercase()
}

pub fn origin_url(root: &Path) -> Option<String> {
    let cfg = std::fs::read_to_string(git_dir(root)?.join("config")).ok()?;
    let mut in_origin = false;
    let mut first_remote = None;
    let mut in_remote = false;
    for line in cfg.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_origin = t == "[remote \"origin\"]";
            in_remote = t.starts_with("[remote ");
            continue;
        }
        if !in_remote { continue; }
        if let Some(v) = t.strip_prefix("url").map(|r| r.trim_start()).and_then(|r| r.strip_prefix('=')) {
            let v = normalize_remote(v);
            if in_origin { return Some(v); }
            first_remote.get_or_insert(v);
        }
    }
    first_remote
}

pub struct Resolver<'a> {
    cache: HashMap<String, ProjectInfo>,
    aliases: &'a [ProjectAlias],
}

impl<'a> Resolver<'a> {
    pub fn new(aliases: &'a [ProjectAlias]) -> Self { Resolver { cache: HashMap::new(), aliases } }

    pub fn resolve(&mut self, cwd: &str) -> ProjectInfo {
        let norm = normalize(cwd);
        if let Some(p) = self.cache.get(&norm) {
            return p.clone();
        }
        let path = PathBuf::from(&norm);
        // an empty path would resolve against our own working directory and pick up its repository
        let root = if norm.is_empty() || !path.exists() { path.clone() } else { git_root(&path).unwrap_or_else(|| path.clone()) };
        let remote = if norm.is_empty() { None } else { origin_url(&root) };
        let root_s = normalize(&root.display().to_string());
        let mut key = remote.clone().unwrap_or_else(|| if cfg!(windows) { root_s.to_lowercase() } else { root_s.clone() });
        let mut name = root.file_name().and_then(|n| n.to_str()).map(String::from).unwrap_or_else(|| root_s.clone());
        if norm.is_empty() {
            key = "(no folder)".into();
            name = "No folder".into();
        }
        // manual merges and names from the synced registry
        let mut hops = 0;
        while let Some(a) = self.aliases.iter().find(|a| a.key == key && a.merge_into.is_some()) {
            key = a.merge_into.clone().unwrap();
            hops += 1;
            if hops > 8 { break; }
        }
        if let Some(n) = self.aliases.iter().find(|a| a.key == key).and_then(|a| a.name.clone()) {
            name = n;
        }
        let info = ProjectInfo { key, name, root: root_s, remote };
        self.cache.insert(norm, info.clone());
        info
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_submodule_url_is_not_the_remote() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join(".git")).unwrap();
        std::fs::write(d.path().join(".git").join("config"), "[core]\n\tbare = false\n[submodule \"lib\"]\n\turl = https://github.com/other/lib.git\n").unwrap();
        assert_eq!(origin_url(d.path()), None);
        std::fs::write(d.path().join(".git").join("config"), "[submodule \"lib\"]\n\turl = https://github.com/other/lib\n[remote \"up\"]\n\turl = git@github.com:me/app.git\n").unwrap();
        assert_eq!(origin_url(d.path()).as_deref(), Some("github.com/me/app"));
    }

    #[test]
    fn no_folder_has_no_repository() {
        // run from inside this repository: an empty cwd must not pick it up
        let p = super::Resolver::new(&[]).resolve("");
        assert_eq!((p.key.as_str(), p.name.as_str(), p.remote), ("(no folder)", "No folder", None));
    }

    #[test]
    fn ports_and_at_signs_in_paths_do_not_split_a_repository() {
        assert_eq!(normalize_remote("ssh://git@host.example:7999/team/app.git"), normalize_remote("git@host.example:team/app.git"));
        assert_eq!(normalize_remote("https://host.example/team/a@b"), "host.example/team/a@b");
    }

    #[test]
    fn remotes_normalize_to_one_key() {
        assert_eq!(normalize_remote("git@github.com:Example/platform-gitops.git"), "github.com/example/platform-gitops");
        assert_eq!(normalize_remote("https://user@github.com/example/platform-gitops"), "github.com/example/platform-gitops");
        assert_eq!(normalize_remote("https://github.com/example/platform-gitops.git/"), "github.com/example/platform-gitops");
    }

    #[test]
    fn subfolders_group_under_the_repository() {
        let t = tempfile::tempdir().unwrap();
        let repo = t.path().join("platform-gitops");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::write(repo.join(".git").join("config"), "[remote \"origin\"]\n\turl = git@github.com:example/platform-gitops.git\n").unwrap();
        std::fs::create_dir_all(repo.join("apps")).unwrap();
        let mut r = Resolver::new(&[]);
        let a = r.resolve(&repo.display().to_string());
        let b = r.resolve(&repo.join("apps").display().to_string());
        assert_eq!(a.key, b.key);
        assert_eq!(a.key, "github.com/example/platform-gitops");
        assert_eq!(b.name, "platform-gitops");
    }

    #[test]
    fn aliases_merge_and_rename() {
        let aliases = vec![
            ProjectAlias { key: "c:\\old".into(), name: None, merge_into: Some("c:\\new".into()) },
            ProjectAlias { key: "c:\\new".into(), name: Some("New name".into()), merge_into: None },
        ];
        let mut r = Resolver::new(&aliases);
        let p = r.resolve("C:\\old");
        if cfg!(windows) {
            assert_eq!(p.key, "c:\\new");
            assert_eq!(p.name, "New name");
        }
    }
}
