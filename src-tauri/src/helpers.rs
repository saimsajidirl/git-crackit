use crate::state::{AppState, Credentials, UndoPoint};
use crate::types::*;
use git2::{Cred, Delta, Diff, Patch, RemoteCallbacks, Repository, Status};
use std::collections::HashMap;
use std::io::{Read, Write};
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
        return "SSH: authentication failed — the key may not be loaded or the passphrase is missing.".into();
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

pub fn commit_info(
    repo: &Repository,
    c: &git2::Commit,
    labels: &HashMap<String, Vec<RefLabel>>,
) -> CommitInfo {
    let oid = c.id().to_string();
    CommitInfo {
        signed: repo.extract_signature(&c.id(), Some("gpgsig")).is_ok(),
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
    commit_maybe_signed(repo, &signature, &signature, message, &tree, &parents)?;
    repo.cleanup_state().map_err(|e| e.message().to_string())?;
    Ok(())
}

/// Paths a merge would update (merge-base → theirs) that are dirty locally —
/// modified, staged, deleted, or untracked. `git merge`/`pull` refuse to start
/// when these overlap; checking first keeps a blocked merge from leaving a
/// half-written index or a force checkout from clobbering the worktree.
fn merge_blocked_paths(
    repo: &Repository,
    head: Option<&git2::Commit>,
    theirs: &git2::Commit,
) -> Result<Vec<String>, String> {
    let mut so = git2::StatusOptions::new();
    so.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo
        .statuses(Some(&mut so))
        .map_err(|e| e.message().to_string())?;
    let dirty: std::collections::HashSet<String> = statuses
        .iter()
        .filter_map(|e| e.path().map(|p| p.to_string()))
        .collect();
    if dirty.is_empty() {
        return Ok(vec![]);
    }

    let base_tree = match head {
        Some(h) => {
            let base_oid = repo
                .merge_base(h.id(), theirs.id())
                .map_err(|e| e.message().to_string())?;
            Some(
                repo.find_commit(base_oid)
                    .and_then(|c| c.tree())
                    .map_err(|e| e.message().to_string())?,
            )
        }
        None => None,
    };
    let their_tree = theirs.tree().map_err(|e| e.message().to_string())?;
    let diff = repo
        .diff_tree_to_tree(base_tree.as_ref(), Some(&their_tree), None)
        .map_err(|e| e.message().to_string())?;

    let mut blocked: Vec<String> = diff
        .deltas()
        .flat_map(|d| {
            [d.new_file().path(), d.old_file().path()]
                .into_iter()
                .flatten()
                .map(|p| p.to_string_lossy().into_owned())
        })
        .filter(|p| dirty.contains(p))
        .collect();
    blocked.sort();
    blocked.dedup();
    Ok(blocked)
}

/// Number of working-tree changes (staged + unstaged + untracked, ignored
/// excluded) — same accounting as `get_status`/the Changes badge.
pub fn dirty_count(repo: &Repository) -> usize {
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    repo.statuses(Some(&mut opts))
        .map(|s| {
            s.iter()
                .filter(|e| {
                    let st = e.status();
                    !st.is_empty() && !st.contains(git2::Status::IGNORED)
                })
                .count()
        })
        .unwrap_or(0)
}

/// `git stash`-style message — libgit2 prepends "On <branch>:" itself, so this
/// yields "On main: WIP: abc1234 init" in the Stashes list.
pub fn wip_message(repo: &Repository) -> String {
    match repo.head().ok().and_then(|h| h.peel_to_commit().ok()) {
        Some(c) => format!("WIP: {:.7} {}", c.id(), c.summary().unwrap_or("")),
        None => "WIP".to_string(),
    }
}

pub fn stash_worktree(repo: &mut Repository, msg: &str) -> Result<(), String> {
    let signature = sig(repo)?;
    repo.stash_save(&signature, msg, Some(git2::StashFlags::INCLUDE_UNTRACKED))
        .map_err(|e| e.message().to_string())?;
    Ok(())
}

pub fn outcome(status: &str) -> CheckoutOutcome {
    CheckoutOutcome {
        status: status.into(),
        dirty_files: 0,
        conflicts: vec![],
    }
}

fn dirty_outcome(n: usize) -> CheckoutOutcome {
    CheckoutOutcome {
        status: "dirty".into(),
        dirty_files: n,
        conflicts: vec![],
    }
}

/// Re-apply stash@{0} after `stash_wrap_op`'s op; on apply conflict the stash
/// is kept as a backup and the outcome reports "conflicts".
fn apply_stash_outcome(repo: &mut Repository, out: CheckoutOutcome) -> Result<CheckoutOutcome, String> {
    let mut opts = git2::StashApplyOptions::new();
    opts.reinstantiate_index();
    if let Err(e) = repo.stash_apply(0, Some(&mut opts)) {
        return Err(format!(
            "Operation finished but couldn't re-apply your changes: {} (kept in stash@{{0}})",
            e.message()
        ));
    }
    if repo.index().map(|i| i.has_conflicts()).unwrap_or(false) {
        return Ok(CheckoutOutcome {
            status: "conflicts".into(),
            dirty_files: 0,
            conflicts: conflicts_list(repo),
        });
    }
    let _ = repo.stash_drop(0);
    Ok(out)
}

/// Stash → run `op` → re-apply the stash on top of the result. If `op` fails
/// the stash is popped back; if `op` ends in "conflicts" the stash is left
/// stashed (don't pile apply-conflicts onto a merge conflict).
pub fn stash_wrap_op<F>(repo: &mut Repository, op: F) -> Result<CheckoutOutcome, String>
where
    F: FnOnce(&Repository) -> Result<CheckoutOutcome, String>,
{
    if dirty_count(repo) == 0 {
        return op(repo);
    }
    let msg = wip_message(repo);
    stash_worktree(repo, &msg)?;
    match op(repo) {
        Err(e) => {
            let _ = repo.stash_pop(0, None);
            Err(e)
        }
        Ok(out) => {
            if out.status == "conflicts" {
                Ok(out)
            } else {
                apply_stash_outcome(repo, out)
            }
        }
    }
}

/// Stash → run `op` → leave the changes in stash@{0} (restored only when `op`
/// fails outright).
pub fn stash_keep_op<F>(repo: &mut Repository, op: F) -> Result<CheckoutOutcome, String>
where
    F: FnOnce(&Repository) -> Result<CheckoutOutcome, String>,
{
    if dirty_count(repo) == 0 {
        return op(repo);
    }
    let msg = wip_message(repo);
    stash_worktree(repo, &msg)?;
    match op(repo) {
        Err(e) => {
            let _ = repo.stash_pop(0, None);
            Err(e)
        }
        Ok(out) => Ok(out),
    }
}

/// Dirty-mode driver for checkout-style ops (branch/commit switches):
/// "ask" reports `status:"dirty"` on any dirty file; "carry" tries a safe
/// checkout first (untouched edits travel free) then falls back to
/// stash → checkout → apply; "stash" keeps the changes in a stash.
pub fn dirty_checkout<F>(repo: &mut Repository, mode: &str, op: F) -> Result<CheckoutOutcome, String>
where
    F: Fn(&Repository) -> Result<(), String>,
{
    match mode {
        "carry" => match op(repo) {
            Ok(()) => Ok(outcome("ok")),
            Err(_) => stash_wrap_op(repo, |r| op(r).map(|_| outcome("ok"))),
        },
        "stash" => stash_keep_op(repo, |r| op(r).map(|_| outcome("ok"))),
        "ask" => {
            let n = dirty_count(repo);
            if n > 0 {
                return Ok(dirty_outcome(n));
            }
            op(repo).map(|_| outcome("ok"))
        }
        other => Err(format!("Unknown dirty mode '{}'", other)),
    }
}

/// Wrap `merge_annotated` in a CheckoutOutcome.
fn merge_outcome(
    repo: &Repository,
    annotated: &git2::AnnotatedCommit,
    label: &str,
) -> Result<CheckoutOutcome, String> {
    let res = merge_annotated(repo, annotated, label)?;
    Ok(CheckoutOutcome {
        status: res.status,
        dirty_files: 0,
        conflicts: res.conflicts,
    })
}

/// Dirty-mode driver for merge-style ops (pull/merge). Unlike `dirty_checkout`
/// the "ask" mode only prompts when the merge would actually touch a dirty
/// file — real `git merge`/`pull` don't care about unrelated dirty work.
/// "carry" = stash → merge → re-apply; "stash" = stash → merge, leave stashed.
/// Takes the target `Oid` (not AnnotatedCommit) so `repo` can be borrowed mutably.
pub fn merge_dirty_op(
    repo: &mut Repository,
    oid: git2::Oid,
    label: &str,
    mode: &str,
) -> Result<CheckoutOutcome, String> {
    match mode {
        "carry" => stash_wrap_op(repo, |r| {
            let ann = r
                .find_annotated_commit(oid)
                .map_err(|e| e.message().to_string())?;
            merge_outcome(r, &ann, label)
        }),
        "stash" => stash_keep_op(repo, |r| {
            let ann = r
                .find_annotated_commit(oid)
                .map_err(|e| e.message().to_string())?;
            merge_outcome(r, &ann, label)
        }),
        "ask" => {
            let blocked = {
                let their_commit = repo
                    .find_commit(oid)
                    .map_err(|e| e.message().to_string())?;
                let head_commit = repo.head().and_then(|h| h.peel_to_commit()).ok();
                merge_blocked_paths(repo, head_commit.as_ref(), &their_commit)?
            };
            if !blocked.is_empty() {
                return Ok(dirty_outcome(blocked.len()));
            }
            let ann = repo
                .find_annotated_commit(oid)
                .map_err(|e| e.message().to_string())?;
            merge_outcome(repo, &ann, label)
        }
        other => Err(format!("Unknown dirty mode '{}'", other)),
    }
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

    // Refuse early when files this merge would update have local changes —
    // same guard `git merge`/`git pull` run before starting. Without it a
    // force checkout silently clobbers uncommitted work.
    let their_commit = repo
        .find_commit(annotated.id())
        .map_err(|e| e.message().to_string())?;
    let head_commit = repo.head().and_then(|h| h.peel_to_commit()).ok();
    let blocked = merge_blocked_paths(repo, head_commit.as_ref(), &their_commit)?;
    if !blocked.is_empty() {
        return Err(format!(
            "Cannot merge {}: local changes would be overwritten — commit or stash them first ({})",
            label,
            blocked.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
        ));
    }

    if analysis.is_fast_forward() {
        let oid = annotated.id();
        // Checkout the target tree FIRST (safe), then move the ref — the
        // previous order (set_target + checkout_head(force)) silently
        // overwrote uncommitted changes.
        let mut cb = git2::build::CheckoutBuilder::new();
        cb.safe();
        repo.checkout_tree(their_commit.as_object(), Some(&mut cb))
            .map_err(|e| format!("Cannot fast-forward to {}: {}", label, e.message()))?;
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
        return Ok(MergeResult {
            status: "fast_forward".into(),
            conflicts: vec![],
        });
    }

    if analysis.is_unborn() {
        let oid = annotated.id();
        let mut cb = git2::build::CheckoutBuilder::new();
        cb.safe();
        repo.checkout_tree(their_commit.as_object(), Some(&mut cb))
            .map_err(|e| format!("Cannot check out {}: {}", label, e.message()))?;
        repo.set_head_detached(oid).map_err(|e| e.message().to_string())?;
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

/* ---------------- Undo / redo ---------------- */

/// Snapshot of where HEAD points, for `record_undo` after a successful op.
pub fn head_point(repo: &Repository, label: &str, soft: bool) -> Option<UndoPoint> {
    let head = repo.head().ok()?;
    let oid = head.target()?;
    let refname = if head.is_branch() {
        head.name().map(String::from)
    } else {
        None
    };
    Some(UndoPoint {
        label: label.into(),
        refname,
        oid,
        soft,
    })
}

/// Push an undo point after an op mutated the repo; clears the redo stack.
/// `point` is None when the op was a no-op (e.g. "dirty"/"up_to_date").
pub fn record_undo(state: &AppState, point: Option<UndoPoint>) {
    let Some(p) = point else { return };
    if let Ok(mut u) = state.undo_stack.lock() {
        u.push(p);
        if u.len() > 200 {
            u.remove(0);
        }
    }
    if let Ok(mut r) = state.redo_stack.lock() {
        r.clear();
    }
}

/// Called on open/close/init/clone — undo points are meaningless cross-repo.
pub fn clear_undo(state: &AppState) {
    if let Ok(mut u) = state.undo_stack.lock() {
        u.clear();
    }
    if let Ok(mut r) = state.redo_stack.lock() {
        r.clear();
    }
}

/// Restore HEAD (ref + worktree) to a recorded point.
/// Soft points only move the ref (index/worktree keep the changes, i.e. an
/// undone commit's content stays staged). Hard-ish restores use
/// `git reset --merge` — git2 lacks ResetType::Merge — which handles
/// conflicted indexes and refuses before clobbering dirty work.
pub fn restore_point(repo: &Repository, p: &UndoPoint) -> Result<(), String> {
    if p.soft {
        match &p.refname {
            Some(r) => {
                repo.find_reference(r)
                    .and_then(|mut rf| rf.set_target(p.oid, "undo"))
                    .map_err(|e| e.message().to_string())?;
            }
            None => repo
                .set_head_detached(p.oid)
                .map_err(|e| e.message().to_string())?,
        }
        repo.cleanup_state().ok();
        return Ok(());
    }

    let prev_head = repo
        .head()
        .ok()
        .map(|h| (h.name().map(String::from), h.target()));
    match &p.refname {
        Some(r) => {
            if repo.find_reference(r).is_err() {
                if let Some(name) = r.strip_prefix("refs/heads/") {
                    let c = repo
                        .find_commit(p.oid)
                        .map_err(|e| e.message().to_string())?;
                    repo.branch(name, &c, false)
                        .map_err(|e| e.message().to_string())?;
                }
            }
            repo.set_head(r).map_err(|e| e.message().to_string())?;
            repo.find_reference(r)
                .and_then(|mut rf| rf.set_target(p.oid, "undo"))
                .map_err(|e| e.message().to_string())?;
        }
        None => repo
            .set_head_detached(p.oid)
            .map_err(|e| e.message().to_string())?,
    }
    let out = Command::new("git")
        .current_dir(repo_dir(repo))
        .args(["reset", "--merge", &p.oid.to_string()])
        .output()
        .map_err(|e| format!("git: {}", e))?;
    if !out.status.success() {
        // Roll HEAD back to where it was before the failed restore.
        if let Some((name, oid)) = prev_head {
            match name.as_deref() {
                Some(r) if r != "HEAD" => {
                    let _ = repo.set_head(r);
                }
                _ => {
                    if let Some(o) = oid {
                        let _ = repo.set_head_detached(o);
                    }
                }
            }
        }
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    repo.cleanup_state().ok();
    Ok(())
}

/* ---------------- Per-hunk staging via `git apply` ---------------- */

/// Run `git apply <args>` with `patch` on stdin (args include "-" implicitly).
pub fn git_apply(dir: &Path, args: &[&str], patch: &str) -> Result<(), String> {
    let mut child = Command::new("git")
        .current_dir(dir)
        .args(args)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git: {}", e))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(patch.as_bytes());
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git: {}", e))?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    Err(if err.is_empty() {
        "patch does not apply".to_string()
    } else {
        err
    })
}

/// Extract the patch text for hunk `hunk` of `path` from a diff — the file
/// header plus exactly one `@@` block, so `git apply` can (un)stage just it.
pub fn hunk_patch_text(diff: &Diff, path: &str, hunk: usize) -> Result<String, String> {
    for d in 0..diff.deltas().len() {
        let delta = diff
            .get_delta(d)
            .ok_or_else(|| "invalid diff delta".to_string())?;
        let p = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path());
        if p != Some(Path::new(path)) {
            continue;
        }
        let mut patch = Patch::from_diff(diff, d)
            .map_err(|e| e.message().to_string())?
            .ok_or_else(|| "no patch for file".to_string())?;
        let buf = patch.to_buf().map_err(|e| e.message().to_string())?;
        let text = String::from_utf8_lossy(&buf);
        // Split at "@@ " headers: [file header][hunk0][hunk1]…
        let mut header = String::new();
        let mut hunks: Vec<String> = Vec::new();
        for line in text.lines() {
            if line.starts_with("@@ ") {
                hunks.push(String::new());
            }
            if hunks.is_empty() {
                header.push_str(line);
                header.push('\n');
            } else {
                hunks.last_mut().unwrap().push_str(line);
                hunks.last_mut().unwrap().push('\n');
            }
        }
        let body = hunks
            .get(hunk)
            .ok_or_else(|| format!("hunk {} out of range", hunk))?;
        return Ok(format!("{}{}", header, body));
    }
    Err(format!("no diff for {}", path))
}

/* ---------------- Commit signing ---------------- */

/// True when `commit.gpgsign` (or deprecated `commit.gpgSign`) is enabled.
pub fn sign_enabled(repo: &Repository) -> bool {
    repo.config()
        .ok()
        .and_then(|c| {
            c.get_bool("commit.gpgsign")
                .or_else(|_| c.get_bool("commit.gpgSign"))
                .ok()
        })
        .unwrap_or(false)
}

/// Sign `content` with the repo's configured key — `gpg.format` selects
/// "openpgp" (gpg CLI) or "ssh" (ssh-keygen -Y sign). Returns the
/// ASCII-armored signature block for the `gpgsig` header.
fn sign_buffer(repo: &Repository, content: &[u8]) -> Result<String, String> {
    let cfg = repo.config().map_err(|e| e.message().to_string())?;
    let format = cfg
        .get_string("gpg.format")
        .unwrap_or_else(|_| "openpgp".into());
    let key = cfg.get_string("user.signingkey").ok().filter(|s| !s.is_empty());

    match format.as_str() {
        "ssh" => {
            let program = cfg
                .get_string("gpg.ssh.program")
                .unwrap_or_else(|_| "ssh-keygen".into());
            let key = key.ok_or("SSH signing needs user.signingkey (path to a public/private key)")?;
            // ssh-keygen -Y sign needs a file argument — write a temp file.
            let tmp = std::env::temp_dir().join(format!("gc-sign-{}.buf", std::process::id()));
            std::fs::write(&tmp, content).map_err(|e| e.to_string())?;
            let sig_path = tmp.with_extension("sig");
            let _ = std::fs::remove_file(&sig_path);
            let key_path = expand_tilde(&key);
            let out = Command::new(&program)
                .args(["-Y", "sign", "-f"])
                .arg(&key_path)
                .args(["-n", "git"])
                .arg(&tmp)
                .output()
                .map_err(|e| format!("{}: {}", program, e))?;
            let _ = std::fs::remove_file(&tmp);
            if !out.status.success() {
                return Err(format!(
                    "ssh-keygen signing failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            let sig = std::fs::read_to_string(&sig_path).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(&sig_path);
            Ok(sig.trim_end().to_string())
        }
        _ => {
            let program = cfg
                .get_string("gpg.program")
                .unwrap_or_else(|_| "gpg".into());
            let mut args = vec![
                "--batch".to_string(),
                "--yes".to_string(),
                "--detach-sign".to_string(),
                "--armor".to_string(),
            ];
            if let Some(k) = &key {
                args.push("--local-user".into());
                args.push(expand_tilde(k));
            }
            args.push("--output".into());
            args.push("-".into());
            args.push("-".into());
            let mut child = Command::new(&program)
                .args(&args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("{}: {}", program, e))?;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(content);
            }
            let out = child.wait_with_output().map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(format!(
                    "gpg signing failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
        }
    }
}

fn expand_tilde(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    p.to_string()
}

/// Create a signed commit object honoring `commit.gpgsign`, and move HEAD.
/// Falls back to `repo.commit("HEAD", …)` when signing is off.
pub fn commit_maybe_signed(
    repo: &Repository,
    author: &git2::Signature,
    committer: &git2::Signature,
    message: &str,
    tree: &git2::Tree,
    parents: &[&git2::Commit],
) -> Result<git2::Oid, String> {
    if !sign_enabled(repo) {
        return repo
            .commit(Some("HEAD"), author, committer, message, tree, parents)
            .map_err(|e| e.message().to_string());
    }
    let buf = repo
        .commit_create_buffer(author, committer, message, tree, parents)
        .map_err(|e| e.message().to_string())?;
    let content = String::from_utf8_lossy(&buf).to_string();
    let sig_text = sign_buffer(repo, content.as_bytes())?;
    let oid = repo
        .commit_signed(&content, &sig_text, Some("gpgsig"))
        .map_err(|e| e.message().to_string())?;
    // commit_signed creates the object only — move HEAD's ref like `repo.commit`.
    // `repo.reference` creates-or-updates; on unborn branches `repo.head()`
    // fails, so resolve HEAD's symbolic target directly and create it.
    let log = format!("commit: {}", message.lines().next().unwrap_or(""));
    match repo.head() {
        Ok(head) if head.is_branch() => {
            repo.reference(head.name().unwrap_or("HEAD"), oid, true, &log)
                .map(|_| ())
                .map_err(|e| e.message().to_string())?;
        }
        Ok(_) => repo
            .set_head_detached(oid)
            .map_err(|e| e.message().to_string())?,
        Err(_) => {
            let sym = repo
                .find_reference("HEAD")
                .and_then(|r| r.symbolic_target().map(String::from).ok_or_else(|| {
                    git2::Error::from_str("unborn HEAD without symbolic target")
                }))
                .map_err(|e| e.message().to_string())?;
            repo.reference(&sym, oid, true, &log)
                .map(|_| ())
                .map_err(|e| e.message().to_string())?;
        }
    }
    Ok(oid)
}
