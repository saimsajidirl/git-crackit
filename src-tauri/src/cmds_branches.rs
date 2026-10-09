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
    checkout_branch_inner(&repo, &name)
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
    let mut cb = git2::build::CheckoutBuilder::new();
    cb.force();
    repo.checkout_head(Some(&mut cb))
        .map_err(|e| e.message().to_string())?;
    repo.cleanup_state().map_err(|e| e.message().to_string())?;
    Ok(())
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
