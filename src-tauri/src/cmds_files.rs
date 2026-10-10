use crate::helpers::*;
use crate::state::AppState;
use crate::types::*;
use base64::Engine;
use git2::{DiffOptions, Status, StatusOptions};
use std::path::Path;
use tauri::State;

fn image_mime(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    }
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{};base64,{}", mime, base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Old (HEAD) vs new (index if `staged`, else worktree) image contents as data URLs.
#[tauri::command]
pub fn get_image_diff(state: State<AppState>, path: String, staged: bool) -> Result<ImageDiff, String> {
    let repo = open_repo(&state)?;
    let rel = Path::new(&path);
    let mime = image_mime(&path);

    let old = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_tree().ok())
        .and_then(|t| t.get_path(rel).ok())
        .and_then(|e| e.to_object(&repo).ok())
        .and_then(|o| o.peel_to_blob().ok())
        .map(|b| data_url(mime, b.content()));

    let new = if staged {
        repo.index()
            .ok()
            .and_then(|idx| idx.get_path(rel, 0).map(|e| e.id))
            .and_then(|oid| repo.find_blob(oid).ok())
            .map(|b| data_url(mime, b.content()))
    } else {
        workdir(&repo)
            .ok()
            .and_then(|d| std::fs::read(d.join(rel)).ok())
            .map(|b| data_url(mime, &b))
    };

    if old.is_none() && new.is_none() {
        return Err("No image data".to_string());
    }
    Ok(ImageDiff {
        old,
        new,
        mime: mime.to_string(),
    })
}

#[tauri::command]
pub fn get_status(state: State<AppState>) -> Result<Vec<FileChange>, String> {
    let repo = open_repo(&state)?;
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true)
        .include_ignored(false);

    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| e.message().to_string())?;
    let mut out = Vec::new();

    for entry in statuses.iter() {
        let st = entry.status();
        if st.is_empty() || st.contains(Status::IGNORED) {
            continue;
        }
        let conflicted = st.contains(Status::CONFLICTED);
        let staged_mask = Status::INDEX_NEW
            | Status::INDEX_MODIFIED
            | Status::INDEX_DELETED
            | Status::INDEX_RENAMED
            | Status::INDEX_TYPECHANGE;
        let wt_mask = Status::WT_NEW
            | Status::WT_MODIFIED
            | Status::WT_DELETED
            | Status::WT_RENAMED
            | Status::WT_TYPECHANGE;

        let staged = if st.intersects(staged_mask) {
            Some(status_str(st & staged_mask).to_string())
        } else {
            None
        };
        let unstaged = if st.intersects(wt_mask) {
            Some(status_str(st & wt_mask).to_string())
        } else {
            None
        };
        if staged.is_none() && unstaged.is_none() && !conflicted {
            continue;
        }
        let path = entry
            .path()
            .map(|s| s.to_string())
            .unwrap_or_else(|| String::from_utf8_lossy(entry.path_bytes()).to_string());
        let old_path = entry
            .head_to_index()
            .and_then(|d| d.old_file().path().map(|p| p.to_string_lossy().to_string()))
            .filter(|o| o != &path)
            .or_else(|| {
                entry
                    .index_to_workdir()
                    .and_then(|d| d.old_file().path().map(|p| p.to_string_lossy().to_string()))
                    .filter(|o| o != &path)
            });
        out.push(FileChange {
            path,
            old_path,
            staged,
            unstaged,
            conflicted,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

#[tauri::command]
pub fn stage_files(state: State<AppState>, paths: Vec<String>) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let wd = workdir(&repo)?.to_path_buf();
    let mut index = repo.index().map_err(|e| e.message().to_string())?;
    for p in &paths {
        let full = wd.join(p);
        if full.exists() {
            index.add_path(Path::new(p)).map_err(|e| e.message().to_string())?;
        } else {
            // deleted in worktree -> remove from index
            index.remove_path(Path::new(p)).map_err(|e| e.message().to_string())?;
        }
    }
    index.write().map_err(|e| e.message().to_string())?;
    Ok(())
}

#[tauri::command]
pub fn unstage_files(state: State<AppState>, paths: Vec<String>) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let head_obj = repo
        .head()
        .and_then(|h| h.peel(git2::ObjectType::Commit))
        .ok();
    if let Some(obj) = head_obj {
        repo.reset_default(Some(&obj), paths.iter())
            .map_err(|e| e.message().to_string())?;
    } else {
        // Unborn HEAD: remove paths from index
        let mut index = repo.index().map_err(|e| e.message().to_string())?;
        for p in &paths {
            let _ = index.remove_path(Path::new(p));
        }
        index.write().map_err(|e| e.message().to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn stage_all(state: State<AppState>) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let mut index = repo.index().map_err(|e| e.message().to_string())?;
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .map_err(|e| e.message().to_string())?;
    // removals for deleted files
    index
        .update_all(["*"].iter(), None)
        .map_err(|e| e.message().to_string())?;
    index.write().map_err(|e| e.message().to_string())?;
    Ok(())
}

#[tauri::command]
pub fn unstage_all(state: State<AppState>) -> Result<(), String> {
    let repo = open_repo(&state)?;
    if let Ok(head_tree) = repo.head().and_then(|h| h.peel_to_tree()) {
        // Reset index to HEAD tree (worktree untouched).
        let mut index = repo.index().map_err(|e| e.message().to_string())?;
        index
            .read_tree(&head_tree)
            .map_err(|e| e.message().to_string())?;
        index.write().map_err(|e| e.message().to_string())?;
    } else {
        let mut index = repo.index().map_err(|e| e.message().to_string())?;
        index.clear().map_err(|e| e.message().to_string())?;
        index.write().map_err(|e| e.message().to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn discard_changes(state: State<AppState>, paths: Vec<String>) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let wd = workdir(&repo)?.to_path_buf();
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| e.message().to_string())?;

    let head_obj = repo
        .head()
        .and_then(|h| h.peel(git2::ObjectType::Tree))
        .ok();

    for p in &paths {
        let entry_st = statuses
            .iter()
            .find(|e| e.path().map(|x| x == p).unwrap_or(false))
            .map(|e| e.status());

        let is_untracked = entry_st
            .map(|s| s.contains(Status::WT_NEW))
            .unwrap_or(false);

        if is_untracked {
            let full = wd.join(p);
            if full.is_dir() {
                std::fs::remove_dir_all(&full).map_err(|e| e.to_string())?;
            } else {
                let _ = std::fs::remove_file(&full);
            }
            continue;
        }

        // unstage first
        if let Some(obj) = &head_obj {
            let _ = repo.reset_default(Some(obj), [p.as_str()]);
        }
        // then checkout HEAD version to worktree
        if let Some(obj) = &head_obj {
            let mut cb = git2::build::CheckoutBuilder::new();
            cb.path(p.as_str()).force();
            repo.checkout_tree(obj, Some(&mut cb))
                .map_err(|e| e.message().to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn create_commit(state: State<AppState>, message: String, amend: Option<bool>) -> Result<String, String> {
    let repo = open_repo(&state)?;
    if message.trim().is_empty() {
        return Err("Commit message cannot be empty".into());
    }
    let signature = sig(&repo)?;
    let mut index = repo.index().map_err(|e| e.message().to_string())?;
    if index.has_conflicts() {
        return Err("There are unresolved merge conflicts".into());
    }
    let tree_oid = index.write_tree().map_err(|e| e.message().to_string())?;
    let tree = repo.find_tree(tree_oid).map_err(|e| e.message().to_string())?;

    if amend.unwrap_or(false) {
        let head = repo.head().map_err(|e| e.message().to_string())?;
        let head_commit = head.peel_to_commit().map_err(|e| e.message().to_string())?;
        let oid = head_commit
            .amend(Some("HEAD"), None, None, None, Some(&message), Some(&tree))
            .map_err(|e| e.message().to_string())?;
        repo.cleanup_state().ok();
        return Ok(oid.to_string());
    }

    // Merge/cherry-pick/revert in progress: include extra parents automatically.
    let mut parents: Vec<git2::Commit> = Vec::new();
    if let Ok(h) = repo.head().and_then(|h| h.peel_to_commit()) {
        parents.push(h);
    }
    for refname in ["MERGE_HEAD", "CHERRY_PICK_HEAD"] {
        if let Ok(r) = repo.find_reference(refname) {
            if let Some(oid) = r.target() {
                if let Ok(c) = repo.find_commit(oid) {
                    parents.push(c);
                }
            }
        }
    }
    if repo.state() == git2::RepositoryState::Revert {
        if let Ok(r) = repo.find_reference("REVERT_HEAD") {
            if let Some(oid) = r.target() {
                if let Ok(c) = repo.find_commit(oid) {
                    parents.push(c);
                }
            }
        }
    }
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    let oid = repo
        .commit(Some("HEAD"), &signature, &signature, &message, &tree, &parent_refs)
        .map_err(|e| e.message().to_string())?;
    repo.cleanup_state().ok();
    Ok(oid.to_string())
}

#[tauri::command]
pub fn get_working_diff(state: State<AppState>, path: String, staged: bool) -> Result<FileDiff, String> {
    let repo = open_repo(&state)?;
    let mut opts = DiffOptions::new();
    opts.pathspec(&path)
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .context_lines(3);

    let mut diff = if staged {
        let head_tree = repo
            .head()
            .and_then(|h| h.peel_to_tree())
            .ok();
        let index = repo.index().map_err(|e| e.message().to_string())?;
        repo.diff_tree_to_index(head_tree.as_ref(), Some(&index), Some(&mut opts))
            .map_err(|e| e.message().to_string())?
    } else {
        let index = repo.index().map_err(|e| e.message().to_string())?;
        repo.diff_index_to_workdir(Some(&index), Some(&mut opts))
            .map_err(|e| e.message().to_string())?
    };
    find_file_in_diff(&mut diff, &path)
}

#[tauri::command]
pub fn list_submodules(state: State<AppState>) -> Result<Vec<SubmoduleInfo>, String> {
    let repo = open_repo(&state)?;
    let subs = repo.submodules().map_err(|e| e.message().to_string())?;
    Ok(subs
        .iter()
        .map(|s| SubmoduleInfo {
            name: s.name().unwrap_or("").to_string(),
            path: s.path().to_string_lossy().to_string(),
            url: s.url().map(|u| u.to_string()),
        })
        .collect())
}

#[tauri::command]
pub fn get_blame(state: State<AppState>, path: String) -> Result<Vec<BlameHunkInfo>, String> {
    let repo = open_repo(&state)?;
    let blame = repo
        .blame_file(Path::new(&path), None)
        .map_err(|e| e.message().to_string())?;
    let mut out = Vec::new();
    for hunk in blame.iter() {
        let oid = hunk.final_commit_id();
        let (author, summary) = repo
            .find_commit(oid)
            .map(|c| {
                (
                    c.author().name().unwrap_or("").to_string(),
                    c.summary().unwrap_or("").to_string(),
                )
            })
            .unwrap_or_default();
        out.push(BlameHunkInfo {
            commit_oid: oid.to_string(),
            short_id: oid.to_string().chars().take(7).collect(),
            author,
            start_line: hunk.final_start_line(),
            line_count: hunk.lines_in_hunk(),
            summary,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn ignore_file(state: State<AppState>, path: String) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let wd = workdir(&repo)?.to_path_buf();
    let ignore = wd.join(".gitignore");
    let mut content = std::fs::read_to_string(&ignore).unwrap_or_default();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    if content.lines().any(|l| l.trim() == path) {
        return Ok(());
    }
    content.push_str(&path);
    content.push('\n');
    std::fs::write(&ignore, content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_submodules(state: State<AppState>) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let subs = repo.submodules().map_err(|e| e.message().to_string())?;
    if subs.is_empty() {
        return Err("No submodules in this repository".into());
    }
    for mut s in subs {
        s.update(true, None).map_err(|e| e.message().to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_git_identity(state: State<AppState>) -> Result<GitConfigInfo, String> {
    // Prefer repo-local config; fall back to global.
    let (mut name, mut email) = (None, None);
    if let Ok(repo) = open_repo(&state) {
        if let Ok(cfg) = repo.config() {
            name = cfg.get_string("user.name").ok().filter(|s| !s.is_empty());
            email = cfg.get_string("user.email").ok().filter(|s| !s.is_empty());
        }
    }
    if name.is_none() || email.is_none() {
        if let Ok(cfg) = git2::Config::open_default() {
            if name.is_none() {
                name = cfg.get_string("user.name").ok().filter(|s| !s.is_empty());
            }
            if email.is_none() {
                email = cfg.get_string("user.email").ok().filter(|s| !s.is_empty());
            }
        }
    }
    Ok(GitConfigInfo { name, email })
}

#[tauri::command]
pub fn set_git_identity(
    state: State<AppState>,
    name: String,
    email: String,
    global: Option<bool>,
) -> Result<(), String> {
    if global.unwrap_or(false) {
        let mut cfg = git2::Config::open_default().map_err(|e| e.message().to_string())?;
        cfg.set_str("user.name", &name).map_err(|e| e.message().to_string())?;
        cfg.set_str("user.email", &email).map_err(|e| e.message().to_string())?;
    } else {
        let repo = open_repo(&state)?;
        let mut cfg = repo.config().map_err(|e| e.message().to_string())?;
        cfg.set_str("user.name", &name).map_err(|e| e.message().to_string())?;
        cfg.set_str("user.email", &email).map_err(|e| e.message().to_string())?;
    }
    Ok(())
}
