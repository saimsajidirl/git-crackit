use crate::helpers::*;
use crate::state::AppState;
use crate::types::*;
use git2::{ObjectType, Oid, Repository, StashFlags};
use std::cell::RefCell;
use tauri::State;

/// Resolve `stash@{index}` to its commit oid.
fn stash_oid(repo: &mut Repository, index: usize) -> Result<Oid, String> {
    let found: RefCell<Option<Oid>> = RefCell::new(None);
    repo.stash_foreach(|i, _msg, oid| {
        if i == index {
            *found.borrow_mut() = Some(*oid);
            false
        } else {
            true
        }
    })
    .map_err(|e| e.message().to_string())?;
    found
        .into_inner()
        .ok_or_else(|| format!("No stash@{{{}}}", index))
}

/// Tracked diff (base commit → stash commit) + the untracked-files tree
/// (`stash@{n}^3`, only when stashed with -u). Pure reads — no ODB writes,
/// so the fs watcher stays quiet.
fn stash_parts<'r>(
    repo: &'r Repository,
    oid: Oid,
) -> Result<(git2::Diff<'r>, Option<git2::Tree<'r>>), String> {
    let commit = repo.find_commit(oid).map_err(|e| e.message().to_string())?;
    let stash_tree = commit.tree().map_err(|e| e.message().to_string())?;
    let base_tree = commit
        .parent(0)
        .and_then(|p| p.tree())
        .map_err(|e| e.message().to_string())?;
    let tracked = repo
        .diff_tree_to_tree(Some(&base_tree), Some(&stash_tree), None)
        .map_err(|e| e.message().to_string())?;
    let untracked_tree = commit.parent(2).ok().and_then(|p| p.tree().ok());
    Ok((tracked, untracked_tree))
}

/// A FileDiff for a file that exists only in the stash's untracked tree —
/// every line is an addition.
fn added_filediff(path: &str, blob: &git2::Blob) -> FileDiff {
    if blob.is_binary() {
        return FileDiff {
            path: path.into(),
            old_path: None,
            status: "added".into(),
            is_binary: true,
            too_large: false,
            additions: 0,
            deletions: 0,
            hunks: vec![],
        };
    }
    let content = String::from_utf8_lossy(blob.content());
    let lines: Vec<DiffLine> = content
        .split_inclusive('\n')
        .enumerate()
        .map(|(i, l)| DiffLine {
            kind: "add".into(),
            old_lineno: None,
            new_lineno: Some((i + 1) as u32),
            content: l.to_string(),
        })
        .collect();
    let n = lines.len() as u32;
    FileDiff {
        path: path.into(),
        old_path: None,
        status: "added".into(),
        is_binary: false,
        too_large: n > 20000,
        additions: n as usize,
        deletions: 0,
        hunks: vec![DiffHunk {
            header: format!("@@ -0,0 +1,{} @@", n),
            old_start: 0,
            old_lines: 0,
            new_start: 1,
            new_lines: n,
            lines,
        }],
    }
}

#[tauri::command]
pub fn stash_files(state: State<AppState>, index: usize) -> Result<Vec<StashFileInfo>, String> {
    let mut repo = open_repo(&state)?;
    let oid = stash_oid(&mut repo, index)?;
    let (tracked, untracked_tree) = stash_parts(&repo, oid)?;
    let mut out = Vec::new();
    for delta in tracked.deltas() {
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        out.push(StashFileInfo {
            path,
            status: delta_str(delta.status()).to_string(),
        });
    }
    if let Some(tree) = &untracked_tree {
        let mut walk_err = false;
        let _ = tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
            if entry.kind() == Some(ObjectType::Blob) {
                let path = format!("{}{}", dir, entry.name().unwrap_or_default());
                out.push(StashFileInfo {
                    path,
                    status: "added".into(),
                });
            } else if entry.kind().is_none() {
                walk_err = true;
                return git2::TreeWalkResult::Abort;
            }
            git2::TreeWalkResult::Ok
        });
        if walk_err {
            return Err("failed to read stash untracked tree".into());
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn stash_file_diff(
    state: State<AppState>,
    index: usize,
    path: String,
) -> Result<FileDiff, String> {
    let mut repo = open_repo(&state)?;
    let oid = stash_oid(&mut repo, index)?;
    let (mut tracked, untracked_tree) = stash_parts(&repo, oid)?;
    if let Ok(fd) = find_file_in_diff(&mut tracked, &path) {
        return Ok(fd);
    }
    if let Some(tree) = &untracked_tree {
        if let Ok(entry) = tree.get_path(std::path::Path::new(&path)) {
            if entry.kind() == Some(ObjectType::Blob) {
                let blob = repo.find_blob(entry.id()).map_err(|e| e.message().to_string())?;
                return Ok(added_filediff(&path, &blob));
            }
        }
    }
    Err(format!("No diff found for {}", path))
}

#[tauri::command]
pub fn list_stashes(state: State<AppState>) -> Result<Vec<StashInfo>, String> {
    let mut repo = open_repo(&state)?;
    let items: RefCell<Vec<(usize, String, Oid)>> = RefCell::new(Vec::new());
    repo.stash_foreach(|idx, msg, oid| {
        items.borrow_mut().push((idx, msg.to_string(), *oid));
        true
    })
    .map_err(|e| e.message().to_string())?;
    let out = items
        .into_inner()
        .into_iter()
        .map(|(index, message, oid)| {
            let time = repo
                .find_commit(oid)
                .map(|c| c.time().seconds())
                .unwrap_or(0);
            StashInfo {
                index,
                message,
                oid: oid.to_string(),
                time,
            }
        })
        .collect();
    Ok(out)
}

#[tauri::command]
pub fn stash_save(
    state: State<AppState>,
    message: String,
    include_untracked: Option<bool>,
) -> Result<(), String> {
    let mut repo = open_repo(&state)?;
    let signature = sig(&repo)?;
    let flags = if include_untracked.unwrap_or(true) {
        StashFlags::INCLUDE_UNTRACKED
    } else {
        StashFlags::DEFAULT
    };
    let msg = if message.trim().is_empty() {
        "WIP".to_string()
    } else {
        message
    };
    repo.stash_save(&signature, &msg, Some(flags))
        .map_err(|e| e.message().to_string())?;
    Ok(())
}

#[tauri::command]
pub fn stash_apply(state: State<AppState>, index: usize) -> Result<MergeResult, String> {
    let mut repo = open_repo(&state)?;
    let mut opts = git2::StashApplyOptions::new();
    let mut cb = git2::build::CheckoutBuilder::new();
    cb.safe();
    opts.checkout_options(cb);
    repo.stash_apply(index, Some(&mut opts))
        .map_err(|e| e.message().to_string())?;
    let idx = repo.index().map_err(|e| e.message().to_string())?;
    if idx.has_conflicts() {
        return Ok(MergeResult {
            status: "conflicts".into(),
            conflicts: conflicts_list(&repo),
        });
    }
    Ok(MergeResult {
        status: "merged".into(),
        conflicts: vec![],
    })
}

#[tauri::command]
pub fn stash_pop(state: State<AppState>, index: usize) -> Result<(), String> {
    let mut repo = open_repo(&state)?;
    repo.stash_pop(index, None).map_err(|e| e.message().to_string())?;
    Ok(())
}

#[tauri::command]
pub fn stash_drop(state: State<AppState>, index: usize) -> Result<(), String> {
    let mut repo = open_repo(&state)?;
    repo.stash_drop(index).map_err(|e| e.message().to_string())?;
    Ok(())
}

#[tauri::command]
pub fn list_tags(state: State<AppState>) -> Result<Vec<TagInfo>, String> {
    let repo = open_repo(&state)?;
    let names = repo.tag_names(None).map_err(|e| e.message().to_string())?;
    let mut out = Vec::new();
    for name in names.iter().flatten() {
        let Ok(obj) = repo.revparse_single(&format!("refs/tags/{}", name)) else {
            continue;
        };
        let annotated = obj.kind() == Some(ObjectType::Tag);
        let (msg, target_oid, time) = if annotated {
            if let Ok(tag) = repo.find_tag(obj.id()) {
                let t = tag.target_id();
                let commit_time = repo
                    .find_commit(t)
                    .map(|c| c.time().seconds())
                    .unwrap_or(tag.tagger().map(|s| s.when().seconds()).unwrap_or(0));
                (
                    tag.message().map(|m| m.to_string()),
                    t,
                    commit_time,
                )
            } else {
                (None, obj.id(), 0)
            }
        } else {
            let time = repo
                .find_commit(obj.id())
                .map(|c| c.time().seconds())
                .unwrap_or(0);
            (None, obj.id(), time)
        };
        out.push(TagInfo {
            name: name.to_string(),
            oid: target_oid.to_string(),
            annotated,
            message: msg,
            time,
        });
    }
    out.sort_by(|a, b| b.time.cmp(&a.time));
    Ok(out)
}

#[tauri::command]
pub fn create_tag(
    state: State<AppState>,
    name: String,
    target: Option<String>,
    message: Option<String>,
) -> Result<(), String> {
    let repo = open_repo(&state)?;
    if name.trim().is_empty() {
        return Err("Tag name cannot be empty".into());
    }
    let obj = match target.filter(|s| !s.is_empty()) {
        Some(t) => repo
            .revparse_single(&t)
            .map_err(|e| format!("Invalid target: {}", e.message()))?,
        None => repo
            .head()
            .and_then(|h| h.peel(ObjectType::Commit))
            .map_err(|e| e.message().to_string())?,
    };
    match message.filter(|s| !s.trim().is_empty()) {
        Some(m) => {
            let signature = sig(&repo)?;
            repo.tag(name.trim(), &obj, &signature, &m, false)
                .map_err(|e| e.message().to_string())?;
        }
        None => {
            repo.tag_lightweight(name.trim(), &obj, false)
                .map_err(|e| e.message().to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn delete_tag(state: State<AppState>, name: String) -> Result<(), String> {
    let repo = open_repo(&state)?;
    repo.tag_delete(&name).map_err(|e| e.message().to_string())?;
    Ok(())
}
