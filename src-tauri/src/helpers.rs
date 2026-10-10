use crate::state::{AppState, Credentials};
use crate::types::*;
use git2::{Cred, Delta, Diff, Patch, RemoteCallbacks, Repository, Status};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter, Runtime, State};

pub fn open_repo(state: &State<AppState>) -> Result<Repository, String> {
    let guard = state.repo_path.lock().map_err(|e| e.to_string())?;
    let path = guard
        .clone()
        .ok_or_else(|| "No repository is currently open".to_string())?;
    Repository::open(&path).map_err(|e| e.message().to_string())
}

pub fn get_creds(state: &State<AppState>) -> Credentials {
    state
        .creds
        .lock()
        .map(|c| c.clone())
        .unwrap_or_default()
}

/// True for ssh:// and scp-like (`[user@]host:path`) remote URLs.
/// Local paths and https:// return false; Windows drive letters (C:\) excluded.
pub fn is_ssh_url(url: &str) -> bool {
    if url.contains("://") {
        return url.starts_with("ssh://") || url.starts_with("git+ssh://");
    }
    let before_slash = url.split('/').next().unwrap_or("");
    match before_slash.find(':') {
        Some(i) => i > 1 || before_slash.contains('@'),
        None => false,
    }
}

/// Map stderr from a `git` CLI network op to a friendly (or `AUTH:`-prefixed) error.
fn map_git_err(stderr: &str) -> String {
    let m = stderr.trim();
    let lower = m.to_lowercase();
    let summary = m.to_string();
    if lower.contains("permission denied") || lower.contains("publickey") {
        return "SSH authentication failed — Git Crackit uses your system SSH keys/agent. Run `ssh-add` or check ~/.ssh.".into();
    }
    if lower.contains("could not read username")
        || lower.contains("authentication failed")
        || lower.contains("401")
        || lower.contains("403")
    {
        return format!("AUTH: {}", summary);
    }
    if lower.contains("non-fast-forward") || lower.contains("fetch first") {
        return "Push rejected (non-fast-forward). Fetch/pull first, then push again.".into();
    }
    summary
}

/// Build a `git` command for a network op in `dir`.
/// Stored HTTPS creds are injected via a credential helper (user's own helpers
/// still answer first); interactive prompts are disabled.
fn git_base(dir: Option<&Path>, creds: &Credentials) -> Command {
    let mut cmd = Command::new("git");
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    if std::env::var_os("GIT_SSH_COMMAND").is_none() {
        cmd.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
    }
    if let (Some(u), Some(p)) = (&creds.username, &creds.password) {
        cmd.env("GIT_CRACKIT_USER", u)
            .env("GIT_CRACKIT_PASS", p)
            .arg("-c")
            .arg("credential.helper=!f() { echo \"username=$GIT_CRACKIT_USER\"; echo \"password=$GIT_CRACKIT_PASS\"; }; f");
    }
    cmd
}

/// Run `git <args>` in `dir` for a network op (fetch/push). Returns combined output.
pub fn git_net(dir: &Path, creds: &Credentials, args: &[&str]) -> Result<String, String> {
    let out = git_base(Some(dir), creds)
        .args(args)
        .output()
        .map_err(|_| "SSH remotes require the `git` CLI to be installed.".to_string())?;
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if out.status.success() {
        Ok(format!("{}{}", String::from_utf8_lossy(&out.stdout), stderr))
    } else {
        Err(map_git_err(&stderr))
    }
}

/// `git clone` via CLI (SSH remotes) with live progress → `clone-progress` events.
pub fn git_clone<R: Runtime>(
    app: &AppHandle<R>,
    url: &str,
    dest: &Path,
    creds: &Credentials,
) -> Result<(), String> {
    let mut cmd = git_base(None, creds);
    cmd.arg("clone")
        .arg("--progress")
        .arg(url)
        .arg(dest.as_os_str())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|_| "SSH remotes require the `git` CLI to be installed.".to_string())?;
    let mut pipe = child.stderr.take().unwrap();

    let app2 = app.clone();
    let dest_s = dest.to_string_lossy().to_string();
    let reader = std::thread::spawn(move || {
        // Progress updates arrive separated by \r, so read bytes not lines.
        let mut log = String::new();
        let mut seg: Vec<u8> = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            for &b in &buf[..n] {
                if b == b'\r' || b == b'\n' {
                    let line = String::from_utf8_lossy(&seg).trim_end().to_string();
                    seg.clear();
                    if line.is_empty() {
                        continue;
                    }
                    if let Some(pos) = line.find("Receiving objects:") {
                        if let Some((r, t)) = parse_ratio(&line[pos..]) {
                            let _ = app2.emit(
                                "clone-progress",
                                OpProgress {
                                    op: "clone".into(),
                                    received: r,
                                    total: t,
                                    path: dest_s.clone(),
                                },
                            );
                        }
                    }
                    log.push_str(&line);
                    log.push('\n');
                } else {
                    seg.push(b);
                }
            }
        }
        log
    });

    let status = child.wait().map_err(|e| e.to_string())?;
    let log = reader.join().unwrap_or_default();
    if status.success() {
        Ok(())
    } else {
        Err(map_git_err(&log))
    }
}

/// Parse `(received/total)` out of a git progress line.
fn parse_ratio(s: &str) -> Option<(usize, usize)> {
    let open = s.find('(')?;
    let close = s.find(')')?;
    let (a, b) = s[open + 1..close].split_once('/')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// Directory to run `git` in for `repo` (worktree root, or the git dir for bare repos).
pub fn repo_dir(repo: &Repository) -> &Path {
    repo.workdir().unwrap_or_else(|| repo.path())
}

/// Whether the `git-lfs` CLI is installed.
pub fn lfs_available() -> bool {
    Command::new("git")
        .args(["lfs", "version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// True when the repo's attributes enable the LFS smudge filter.
pub fn repo_uses_lfs(repo: &Repository) -> bool {
    let content = repo
        .workdir()
        .and_then(|d| std::fs::read_to_string(d.join(".gitattributes")).ok())
        .or_else(|| std::fs::read_to_string(repo.path().join("info/attributes")).ok());
    content.map(|c| c.contains("filter=lfs")).unwrap_or(false)
}

/// `git lfs pull` — download LFS objects and smudge the worktree (post clone/fetch/merge).
pub fn lfs_pull(repo: &Repository, creds: &Credentials) {
    if repo_uses_lfs(repo) && lfs_available() {
        let _ = git_net(repo_dir(repo), creds, &["lfs", "pull"]);
    }
}

/// `git lfs checkout` — smudge the worktree after a checkout (local, no network).
pub fn lfs_checkout(repo: &Repository) {
    if repo_uses_lfs(repo) && lfs_available() {
        let _ = git_net(repo_dir(repo), &Credentials::default(), &["lfs", "checkout"]);
    }
}

/// Credentials callback for network ops (HTTPS user/pass-token).
pub fn remote_callbacks<'a>(creds: Credentials) -> RemoteCallbacks<'a> {
    let mut cb = RemoteCallbacks::new();
    cb.credentials(move |_url, username_from_url, allowed| {
        if allowed.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            let user = creds
                .username
                .clone()
                .or_else(|| username_from_url.map(|u| u.to_string()));
            if let (Some(u), Some(p)) = (user, creds.password.clone()) {
                return Cred::userpass_plaintext(&u, &p);
            }
        }
        Cred::default()
    });
    cb
}

pub fn sig(repo: &Repository) -> Result<git2::Signature<'static>, String> {
    repo.signature().map_err(|_| {
        "Git identity is not configured. Set user.name and user.email in Settings.".to_string()
    })
}

pub fn workdir(repo: &Repository) -> Result<&Path, String> {
    repo.workdir()
        .ok_or_else(|| "Repository has no working directory (bare repo)".to_string())
}

pub fn status_str(s: Status) -> &'static str {
    if s.contains(Status::INDEX_NEW) || s.contains(Status::WT_NEW) {
        "added"
    } else if s.contains(Status::INDEX_DELETED) || s.contains(Status::WT_DELETED) {
        "deleted"
    } else if s.contains(Status::INDEX_RENAMED) || s.contains(Status::WT_RENAMED) {
        "renamed"
    } else if s.contains(Status::INDEX_TYPECHANGE) || s.contains(Status::WT_TYPECHANGE) {
        "typechange"
    } else {
        "modified"
    }
}

pub fn delta_str(d: Delta) -> &'static str {
    match d {
        Delta::Added => "added",
        Delta::Deleted => "deleted",
        Delta::Renamed => "renamed",
        Delta::Typechange => "typechange",
        Delta::Copied => "copied",
        Delta::Conflicted => "conflicted",
        Delta::Untracked => "added",
        _ => "modified",
    }
}

/// Map commit oid -> ref labels (branches, remote branches, tags).
pub fn ref_labels(repo: &Repository) -> HashMap<String, Vec<RefLabel>> {
    let mut map: HashMap<String, Vec<RefLabel>> = HashMap::new();
    if let Ok(iter) = repo.references() {
        for r in iter.flatten() {
            let Some(oid) = r.target() else { continue };
            let Some(name) = r.name() else { continue };
            let (label, kind) = if let Some(b) = name.strip_prefix("refs/heads/") {
                (b.to_string(), "branch")
            } else if let Some(b) = name.strip_prefix("refs/remotes/") {
                (b.to_string(), "remote")
            } else if let Some(b) = name.strip_prefix("refs/tags/") {
                (b.to_string(), "tag")
            } else {
                continue;
            };
            map.entry(oid.to_string()).or_default().push(RefLabel {
                name: label,
                kind: kind.to_string(),
            });
        }
    }
    // Annotated tags point at tag objects; peel them.
    if let Ok(names) = repo.tag_names(None) {
        for name in names.iter().flatten() {
            if let Ok(obj) = repo.revparse_single(&format!("refs/tags/{}", name)) {
                if let Ok(commit) = obj.peel_to_commit() {
                    map.entry(commit.id().to_string()).or_default().push(RefLabel {
                        name: name.to_string(),
                        kind: "tag".to_string(),
                    });
                }
            }
        }
    }
    map
}

pub fn commit_info(c: &git2::Commit, labels: &HashMap<String, Vec<RefLabel>>) -> CommitInfo {
    let oid = c.id().to_string();
    CommitInfo {
        short_id: oid.chars().take(7).collect(),
        summary: c.summary().unwrap_or("").to_string(),
        body: c.body().unwrap_or("").to_string(),
        author_name: c.author().name().unwrap_or("").to_string(),
        author_email: c.author().email().unwrap_or("").to_string(),
        committer_name: c.committer().name().unwrap_or("").to_string(),
        author_time: c.author().when().seconds(),
        committer_time: c.committer().when().seconds(),
        parents: c.parent_ids().map(|o| o.to_string()).collect(),
        refs: labels.get(&oid).cloned().unwrap_or_default(),
        oid,
    }
}

pub fn conflicts_list(repo: &Repository) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(index) = repo.index() {
        if let Ok(conflicts) = index.conflicts() {
            for c in conflicts.flatten() {
                if let Some(e) = c.our.or(c.their).or(c.ancestor) {
                    out.push(String::from_utf8_lossy(&e.path).to_string());
                }
            }
        }
    }
    out
}

/// Extract a FileDiff for the delta at `idx` inside `diff`.
pub fn patch_to_filediff(patch: &Patch) -> FileDiff {
    let delta = patch.delta();
    let path = delta
        .new_file()
        .path()
        .or_else(|| delta.old_file().path())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let old_path = if delta.status() == Delta::Renamed {
        delta.old_file().path().map(|p| p.to_string_lossy().to_string())
    } else {
        None
    };
    let is_binary = delta.flags().is_binary();

    let mut hunks = Vec::new();
    let mut adds = 0usize;
    let mut dels = 0usize;
    let mut too_large = false;
    let num = patch.num_hunks();
    for h in 0..num {
        if hunks.iter().map(|x: &DiffHunk| x.lines.len()).sum::<usize>() > 20000 {
            too_large = true;
            break;
        }
        let Ok((hunk, nlines)) = patch.hunk(h) else { continue };
        let mut lines = Vec::new();
        for l in 0..nlines {
            let Ok(line) = patch.line_in_hunk(h, l) else { continue };
            let origin = line.origin();
            let kind = match origin {
                '+' => {
                    adds += 1;
                    "add"
                }
                '-' => {
                    dels += 1;
                    "del"
                }
                ' ' => "context",
                _ => continue, // '\', 'H', '=', '>' etc.
            };
            lines.push(DiffLine {
                kind: kind.to_string(),
                old_lineno: line.old_lineno(),
                new_lineno: line.new_lineno(),
                content: String::from_utf8_lossy(line.content()).to_string(),
            });
        }
        hunks.push(DiffHunk {
            header: String::from_utf8_lossy(hunk.header()).trim_end().to_string(),
            old_start: hunk.old_start(),
            old_lines: hunk.old_lines(),
            new_start: hunk.new_start(),
            new_lines: hunk.new_lines(),
            lines,
        });
    }
    FileDiff {
        path,
        old_path,
        status: delta_str(delta.status()).to_string(),
        is_binary,
        too_large,
        additions: adds,
        deletions: dels,
        hunks,
    }
}

/// Find the FileDiff for `want_path` inside `diff`.
pub fn find_file_in_diff(diff: &mut Diff, want_path: &str) -> Result<FileDiff, String> {
    for i in 0..diff.deltas().len() {
        let delta = diff.get_delta(i).unwrap();
        let matches = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy() == want_path)
            .unwrap_or(false);
        if !matches {
            continue;
        }
        match Patch::from_diff(diff, i) {
            Ok(Some(patch)) => return Ok(patch_to_filediff(&patch)),
            Ok(None) => {
                // No patch (e.g., binary or mode change) — still report delta.
                let path = delta
                    .new_file()
                    .path()
                    .or_else(|| delta.old_file().path())
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                return Ok(FileDiff {
                    path,
                    old_path: delta.old_file().path().map(|p| p.to_string_lossy().to_string()),
                    status: delta_str(delta.status()).to_string(),
                    is_binary: true,
                    too_large: false,
                    additions: 0,
                    deletions: 0,
                    hunks: vec![],
                });
            }
            Err(e) => return Err(e.message().to_string()),
        }
    }
    Err(format!("No diff found for {}", want_path))
}

/// Finish an in-progress merge/cherry-pick state by committing the index.
pub fn finish_in_progress_commit(
    repo: &Repository,
    message: &str,
    extra_parents: &[git2::Oid],
) -> Result<(), String> {
    let mut index = repo.index().map_err(|e| e.message().to_string())?;
    let tree_oid = index.write_tree().map_err(|e| e.message().to_string())?;
    let tree = repo.find_tree(tree_oid).map_err(|e| e.message().to_string())?;
    let head = repo
        .head()
        .and_then(|h| h.peel_to_commit())
        .map_err(|e| e.message().to_string())?;
    let signature = sig(repo)?;
    let mut parents: Vec<&git2::Commit> = vec![&head];
    let extra: Vec<git2::Commit> = extra_parents
        .iter()
        .filter_map(|o| repo.find_commit(*o).ok())
        .collect();
    parents.extend(extra.iter());
    repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &parents)
        .map_err(|e| e.message().to_string())?;
    repo.cleanup_state().map_err(|e| e.message().to_string())?;
    Ok(())
}

/// Shared merge driver: merges `annotated` (label) into HEAD.
pub fn merge_annotated(
    repo: &Repository,
    annotated: &git2::AnnotatedCommit,
    label: &str,
) -> Result<MergeResult, String> {
    let (analysis, _pref) = repo
        .merge_analysis(&[annotated])
        .map_err(|e| e.message().to_string())?;

    if analysis.is_up_to_date() {
        return Ok(MergeResult {
            status: "up_to_date".into(),
            conflicts: vec![],
        });
    }

    if analysis.is_fast_forward() {
        let oid = annotated.id();
        // Move the current branch ref to the target.
        let head_ref = repo.head().map_err(|e| e.message().to_string())?;
        let msg = format!("Fast-forward to {}", label);
        if head_ref.is_branch() {
            let mut r = repo
                .find_reference(head_ref.name().ok_or("bad head")?)
                .map_err(|e| e.message().to_string())?;
            r.set_target(oid, &msg).map_err(|e| e.message().to_string())?;
        } else {
            repo.set_head_detached(oid).map_err(|e| e.message().to_string())?;
        }
        let mut cb = git2::build::CheckoutBuilder::new();
        cb.force();
        repo.checkout_head(Some(&mut cb))
            .map_err(|e| e.message().to_string())?;
        return Ok(MergeResult {
            status: "fast_forward".into(),
            conflicts: vec![],
        });
    }

    if analysis.is_unborn() {
        let oid = annotated.id();
        repo.set_head_detached(oid).map_err(|e| e.message().to_string())?;
        let mut cb = git2::build::CheckoutBuilder::new();
        cb.force();
        repo.checkout_head(Some(&mut cb))
            .map_err(|e| e.message().to_string())?;
        return Ok(MergeResult {
            status: "unborn".into(),
            conflicts: vec![],
        });
    }

    if analysis.is_normal() {
        let mut mo = git2::MergeOptions::new();
        mo.fail_on_conflict(false);
        let mut cb = git2::build::CheckoutBuilder::new();
        cb.safe();
        repo.merge(&[annotated], Some(&mut mo), Some(&mut cb))
            .map_err(|e| e.message().to_string())?;

        let index = repo.index().map_err(|e| e.message().to_string())?;
        if index.has_conflicts() {
            return Ok(MergeResult {
                status: "conflicts".into(),
                conflicts: conflicts_list(repo),
            });
        }
        drop(index);
        finish_in_progress_commit(repo, &format!("Merge {}", label), &[annotated.id()])?;
        return Ok(MergeResult {
            status: "merged".into(),
            conflicts: vec![],
        });
    }

    Err("Unsupported merge state".to_string())
}
