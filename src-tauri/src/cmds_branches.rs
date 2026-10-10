use crate::helpers::*;
use crate::state::AppState;
use crate::types::*;
use git2::BranchType;
use tauri::State;

#[tauri::command]
pub fn list_branches(state: State<AppState>) -> Result<Vec<BranchInfo>, String> {
    let repo = open_repo(&state)?;
    let mut out = Vec::new();
    let iter = repo.branches(None).map_err(|e| e.message().to_string())?;
    for item in iter.flatten() {
        let (branch, btype) = item;
        let is_remote = btype == BranchType::Remote;
        let name = branch
            .name()
            .ok()
            .flatten()
            .unwrap_or("?")
            .to_string();
        // Skip symbolic remote HEAD refs (e.g. origin/HEAD)
        if branch.get().symbolic_target().is_some() {
            continue;
        }
        let is_head = !is_remote && branch.is_head();

        let mut upstream = None;
        let (mut ahead, mut behind) = (None, None);
        if !is_remote {
            if let Ok(up) = branch.upstream() {
                upstream = up.name().ok().flatten().map(|s| s.to_string());
                if let (Some(l), Some(u)) = (branch.get().target(), up.get().target()) {
                    if let Ok((a, b)) = repo.graph_ahead_behind(l, u) {
                        ahead = Some(a);
                        behind = Some(b);
                    }
                }
            }
        }

        let (oid, summary, time) = branch
            .get()
            .target()
            .and_then(|o| repo.find_commit(o).ok())
            .map(|c| {
                (
                    Some(c.id().to_string()),
                    c.summary().unwrap_or("").to_string(),
                    c.time().seconds(),
                )
            })
            .unwrap_or((None, String::new(), 0));

        out.push(BranchInfo {
            name,
            is_head,
            is_remote,
            upstream,
            ahead,
            behind,
            last_commit_oid: oid,
            last_commit_summary: summary,
            last_commit_time: time,
        });
    }
    out.sort_by(|a, b| {
        b.is_head
            .cmp(&a.is_head)
            .then(a.is_remote.cmp(&b.is_remote))
            .then(b.last_commit_time.cmp(&a.last_commit_time))
    });
    Ok(out)
}

#[tauri::command]
pub fn create_branch(
    state: State<AppState>,
    name: String,
    start_point: Option<String>,
    checkout: Option<bool>,
) -> Result<(), String> {
    let repo = open_repo(&state)?;
    if name.trim().is_empty() {
        return Err("Branch name cannot be empty".into());
    }
    let target = match start_point.as_ref().filter(|s| !s.is_empty()) {
        Some(sp) => repo
            .revparse_single(&sp)
            .and_then(|o| o.peel_to_commit())
            .map_err(|e| format!("Invalid start point: {}", e.message()))?,
        None => repo
            .head()
            .and_then(|h| h.peel_to_commit())
            .map_err(|e| e.message().to_string())?,
    };
    let branch = repo
        .branch(name.trim(), &target, false)
        .map_err(|e| e.message().to_string())?;

    // If start point was a remote branch, set upstream.
    if let Some(sp) = start_point.as_ref().filter(|s| !s.is_empty()) {
        if let Ok(rb) = repo.find_branch(&sp, BranchType::Remote) {
            if let Ok(Some(upname)) = rb.name() {
                let _ = branch.into_reference();
                if let Ok(mut b) = repo.find_branch(name.trim(), BranchType::Local) {
                    let _ = b.set_upstream(Some(&upname));
                }
            }
        }
    }

    if checkout.unwrap_or(true) {
        checkout_branch_inner(&repo, name.trim())?;
    }
    Ok(())
}

fn checkout_branch_inner(repo: &git2::Repository, name: &str) -> Result<(), String> {
    // Local branch?
    if let Ok(b) = repo.find_branch(name, BranchType::Local) {
        let refname = b.get().name().ok_or("invalid ref")?.to_string();
        repo.set_head(&refname).map_err(|e| e.message().to_string())?;
        let mut cb = git2::build::CheckoutBuilder::new();
        cb.safe();
        return repo.checkout_head(Some(&mut cb)).map_err(|e| e.message().to_string());
    }
    // Remote branch like "origin/foo" -> create local tracking branch
    if let Ok(rb) = repo.find_branch(name, BranchType::Remote) {
        let local_name = name
            .split_once('/')
            .map(|(_, rest)| rest.to_string())
            .unwrap_or_else(|| name.to_string());
        if repo.find_branch(&local_name, BranchType::Local).is_err() {
            let commit = rb
                .get()
                .peel_to_commit()
                .map_err(|e| e.message().to_string())?;
            let mut lb = repo
                .branch(&local_name, &commit, false)
                .map_err(|e| e.message().to_string())?;
            let _ = lb.set_upstream(Some(name));
        }
        let refname = format!("refs/heads/{}", local_name);
        repo.set_head(&refname).map_err(|e| e.message().to_string())?;
        let mut cb = git2::build::CheckoutBuilder::new();
        cb.safe();
        return repo.checkout_head(Some(&mut cb)).map_err(|e| e.message().to_string());
    }
    Err(format!("Branch '{}' not found", name))
}

#[tauri::command]
pub fn checkout_branch(state: State<AppState>, name: String) -> Result<(), String> {
    let repo = open_repo(&state)?;
    checkout_branch_inner(&repo, &name)?;
    lfs_checkout(&repo);
    Ok(())
}

#[tauri::command]
pub fn delete_branch(state: State<AppState>, name: String) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let is_remote = name.contains('/');
    let bt = if is_remote && repo.find_branch(&name, BranchType::Remote).is_ok() {
        BranchType::Remote
    } else {
        BranchType::Local
    };
    let mut b = repo
        .find_branch(&name, bt)
        .map_err(|_| format!("Branch '{}' not found", name))?;
    if b.is_head() {
        return Err("Cannot delete the current branch".into());
    }
    b.delete().map_err(|e| e.message().to_string())?;
    Ok(())
}

#[tauri::command]
pub fn rename_branch(state: State<AppState>, old: String, new: String) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let mut b = repo
        .find_branch(&old, BranchType::Local)
        .map_err(|_| format!("Branch '{}' not found", old))?;
    b.rename(&new, false).map_err(|e| e.message().to_string())?;
    Ok(())
}

#[tauri::command]
pub fn merge_branch(state: State<AppState>, name: String) -> Result<MergeResult, String> {
    let repo = open_repo(&state)?;
    let branch = repo
        .find_branch(&name, BranchType::Local)
        .or_else(|_| repo.find_branch(&name, BranchType::Remote))
        .map_err(|_| format!("Branch '{}' not found", name))?;
    let annotated = repo
        .reference_to_annotated_commit(branch.get())
        .map_err(|e| e.message().to_string())?;
    merge_annotated(&repo, &annotated, &format!("branch '{}'", name))
}

#[tauri::command]
pub fn abort_merge(state: State<AppState>) -> Result<(), String> {
    let repo = open_repo(&state)?;
    // A CLI-started rebase (drag-drop reorder/squash) must be aborted via git.
    if matches!(
        repo.state(),
        git2::RepositoryState::Rebase
            | git2::RepositoryState::RebaseInteractive
            | git2::RepositoryState::RebaseMerge
            | git2::RepositoryState::ApplyMailbox
            | git2::RepositoryState::ApplyMailboxOrRebase
    ) {
        let dir = repo_dir(&repo).to_path_buf();
        let st = std::process::Command::new("git")
            .current_dir(&dir)
            .args(["rebase", "--abort"])
            .output()
            .map_err(|e| e.to_string())?;
        if !st.status.success() {
            return Err(String::from_utf8_lossy(&st.stderr).trim().to_string());
        }
        return Ok(());
    }
    let mut cb = git2::build::CheckoutBuilder::new();
    cb.force();
    repo.checkout_head(Some(&mut cb))
        .map_err(|e| e.message().to_string())?;
    repo.cleanup_state().map_err(|e| e.message().to_string())?;
    Ok(())
}

/// Reorder/squash a commit by driving `git rebase -i` with a generated
/// sequence-editor script. `action`: "move_after" | "squash_into".
/// On conflict the rebase pauses and repo state becomes "rebasing" —
/// resolve, stage, commit, or Abort.
#[tauri::command]
pub fn rebase_commit_action(
    state: State<AppState>,
    oid: String,
    target: String,
    action: String,
) -> Result<MergeResult, String> {
    let repo = open_repo(&state)?;
    let commit_oid = git2::Oid::from_str(&oid).map_err(|e| e.message().to_string())?;
    let commit = repo
        .find_commit(commit_oid)
        .map_err(|e| e.message().to_string())?;
    let parent = commit.parent(0).ok().map(|p| p.id().to_string());
    let act = if action == "squash_into" { "fixup" } else { "pick" };

    // Sequence editor: rewrite the todo file — drop `move` line, re-emit it
    // (with action `act`) right after the `after` line.
    let script = r#"#!/bin/sh
awk -v move="$GC_MOVE" -v after="$GC_AFTER" -v act="$GC_ACTION" '
{ lines[NR] = $0 }
END {
  mi = 0
  for (i = 1; i <= NR; i++) {
    split(lines[i], a, " ")
    if (a[1] == "pick" && index(move, a[2]) == 1) { mi = i; break }
  }
  if (mi == 0) { print "commit not found in rebase todo" > "/dev/stderr"; exit 1 }
  ml = act substr(lines[mi], index(lines[mi], " "))
  done = 0
  for (i = 1; i <= NR; i++) {
    if (i == mi) continue
    print lines[i]
    split(lines[i], a, " ")
    if (!done && index(after, a[2]) == 1) { print ml; done = 1 }
  }
  if (!done) { print "target not found in rebase todo" > "/dev/stderr"; exit 1 }
}' "$1"
"#;
    let script_dir = dirs::config_dir()
        .map(|d| d.join("git-crackit"))
        .unwrap_or_else(|| std::env::temp_dir());
    std::fs::create_dir_all(&script_dir).map_err(|e| e.to_string())?;
    let script_path = script_dir.join(format!("seq-edit-{}.sh", std::process::id()));
    std::fs::write(&script_path, script).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755));
    }

    let dir = repo_dir(&repo).to_path_buf();
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(&dir)
        .env("GIT_SEQUENCE_EDITOR", format!("sh {}", script_path.display()))
        .env("GIT_EDITOR", "true")
        .env("GC_MOVE", &oid)
        .env("GC_AFTER", &target)
        .env("GC_ACTION", act);
    match parent {
        Some(p) => cmd.args(["rebase", "-i", &p]),
        None => cmd.args(["rebase", "-i", "--root"]),
    };
    let out = cmd.output().map_err(|e| format!("git not found: {}", e))?;
    let _ = std::fs::remove_file(&script_path);

    if out.status.success() {
        lfs_checkout(&repo);
        return Ok(MergeResult {
            status: "merged".into(),
            conflicts: vec![],
        });
    }
    // Rebase may have stopped on a conflict — leave state for resolve/abort flow.
    if !matches!(repo.state(), git2::RepositoryState::Clean) {
        return Ok(MergeResult {
            status: "conflicts".into(),
            conflicts: conflicts_list(&repo),
        });
    }
    Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
}

#[tauri::command]
pub fn rebase_branch(state: State<AppState>, onto: String) -> Result<MergeResult, String> {
    let repo = open_repo(&state)?;
    let onto_ref = repo
        .find_branch(&onto, BranchType::Local)
        .or_else(|_| repo.find_branch(&onto, BranchType::Remote))
        .map_err(|_| format!("Branch '{}' not found", onto))?;
    let onto_ann = repo
        .reference_to_annotated_commit(onto_ref.get())
        .map_err(|e| e.message().to_string())?;
    let head_ann = repo
        .head()
        .and_then(|h| repo.reference_to_annotated_commit(&h))
        .map_err(|e| e.message().to_string())?;

    let signature = sig(&repo)?;
    let mut opts = git2::RebaseOptions::new();
    let mut cb = git2::build::CheckoutBuilder::new();
    cb.safe();
    opts.checkout_options(cb);

    let mut rebase = repo
        .rebase(Some(&head_ann), Some(&onto_ann), None, Some(&mut opts))
        .map_err(|e| e.message().to_string())?;

    while let Some(op) = rebase.next() {
        let _op = op.map_err(|e| e.message().to_string())?;
        match rebase.commit(None, &signature, None) {
            Ok(_) => {}
            Err(e) => {
                let _ = rebase.abort();
                return Ok(MergeResult {
                    status: "conflicts".into(),
                    conflicts: vec![format!(
                        "Rebase stopped on conflict and was aborted: {}",
                        e.message()
                    )],
                });
            }
        }
    }
    rebase.finish(None).map_err(|e| e.message().to_string())?;
    Ok(MergeResult {
        status: "merged".into(),
        conflicts: vec![],
    })
}
