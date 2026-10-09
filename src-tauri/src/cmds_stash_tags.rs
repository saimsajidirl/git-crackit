use crate::helpers::*;
use crate::state::AppState;
use crate::types::*;
use git2::{ObjectType, Oid, StashFlags};
use std::cell::RefCell;
use tauri::State;

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
