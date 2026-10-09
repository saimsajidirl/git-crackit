use crate::helpers::*;
use crate::state::AppState;
use crate::types::*;
use git2::{DiffOptions, ObjectType, Oid, ResetType, Sort};
use tauri::State;

fn diff_changed_files(repo: &git2::Repository, commit: &git2::Commit) -> Vec<ChangedFile> {
    let new_tree = commit.tree().ok();
    let old_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
    let mut opts = DiffOptions::new();
    opts.include_untracked(false).recurse_untracked_dirs(false);
    let mut diff = match repo.diff_tree_to_tree(old_tree.as_ref(), new_tree.as_ref(), Some(&mut opts)) {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    let _ = diff.find_similar(None);
    let mut out = Vec::new();
    for i in 0..diff.deltas().len() {
        // Extract owned delta info first to avoid borrow conflicts with Patch::from_diff.
        let (path, old_path, status) = {
            let delta = diff.get_delta(i).unwrap();
            (
                delta
                    .new_file()
                    .path()
                    .or_else(|| delta.old_file().path())
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
                delta.old_file().path().map(|p| p.to_string_lossy().to_string()),
                delta_str(delta.status()).to_string(),
            )
        };
        let renamed = status == "renamed";
        let (adds, dels) = match git2::Patch::from_diff(&mut diff, i) {
            Ok(Some(p)) => p
                .line_stats()
                .map(|(_ctx, a, d)| (a, d))
                .unwrap_or((0, 0)),
            _ => (0, 0),
        };
        out.push(ChangedFile {
            path,
            old_path: if renamed { old_path } else { None },
            status,
            additions: adds,
            deletions: dels,
        });
    }
    out
}

#[tauri::command]
pub fn get_history(
    state: State<AppState>,
    skip: Option<usize>,
    limit: Option<usize>,
    query: Option<String>,
    refname: Option<String>,
) -> Result<Vec<CommitInfo>, String> {
    let repo = open_repo(&state)?;
    let mut walk = repo.revwalk().map_err(|e| e.message().to_string())?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)
        .map_err(|e| e.message().to_string())?;

    if let Some(r) = refname.filter(|s| !s.is_empty()) {
        // history for a specific branch/ref/commit
        let obj = repo
            .revparse_single(&r)
            .map_err(|e| format!("Cannot resolve '{}': {}", r, e.message()))?;
        walk.push(obj.id()).map_err(|e| e.message().to_string())?;
    } else {
        // all refs (branches, remote branches, tags) + HEAD
        if let Ok(refs) = repo.references() {
            for r in refs.flatten() {
                if let Some(oid) = r.target() {
                    let _ = walk.push(oid);
                }
            }
        }
        if let Ok(h) = repo.head() {
            if let Some(oid) = h.target() {
                let _ = walk.push(oid);
            }
        }
        // stash commits too
        if let Ok(r) = repo.find_reference("refs/stash") {
            if let Some(oid) = r.target() {
                let _ = walk.push(oid);
            }
        }
    }

    let labels = ref_labels(&repo);
    let q = query.map(|s| s.to_lowercase());
    let mut out = Vec::new();
    let limit = limit.unwrap_or(500);
    let skip = skip.unwrap_or(0);
    let mut seen = 0usize;

    for oid in walk.flatten() {
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if let Some(qs) = &q {
            let hay = format!(
                "{} {} {} {}",
                oid,
                commit.summary().unwrap_or(""),
                commit.author().name().unwrap_or(""),
                commit.author().email().unwrap_or("")
            )
            .to_lowercase();
            if !hay.contains(qs) {
                continue;
            }
        }
        if seen < skip {
            seen += 1;
            continue;
        }
        out.push(commit_info(&commit, &labels));
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn get_commit_detail(state: State<AppState>, oid: String) -> Result<CommitDetail, String> {
    let repo = open_repo(&state)?;
    let oid = Oid::from_str(&oid).map_err(|e| e.message().to_string())?;
    let commit = repo.find_commit(oid).map_err(|e| e.message().to_string())?;
    let labels = ref_labels(&repo);
    Ok(CommitDetail {
        commit: commit_info(&commit, &labels),
        files: diff_changed_files(&repo, &commit),
    })
}

#[tauri::command]
pub fn get_commit_file_diff(
    state: State<AppState>,
    oid: String,
    path: String,
) -> Result<FileDiff, String> {
    let repo = open_repo(&state)?;
    let oid = Oid::from_str(&oid).map_err(|e| e.message().to_string())?;
    let commit = repo.find_commit(oid).map_err(|e| e.message().to_string())?;
    let new_tree = commit.tree().map_err(|e| e.message().to_string())?;
    let old_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
    let mut opts = DiffOptions::new();
    opts.pathspec(&path).context_lines(3);
    let mut diff = repo
        .diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts))
        .map_err(|e| e.message().to_string())?;
    find_file_in_diff(&mut diff, &path)
}

#[tauri::command]
pub fn cherry_pick(state: State<AppState>, oid: String) -> Result<MergeResult, String> {
    let repo = open_repo(&state)?;
    let oid = Oid::from_str(&oid).map_err(|e| e.message().to_string())?;
    let commit = repo.find_commit(oid).map_err(|e| e.message().to_string())?;
    let mut co = git2::CherrypickOptions::new();
    repo.cherrypick(&commit, Some(&mut co))
        .map_err(|e| e.message().to_string())?;
    let index = repo.index().map_err(|e| e.message().to_string())?;
    if index.has_conflicts() {
        return Ok(MergeResult {
            status: "conflicts".into(),
            conflicts: conflicts_list(&repo),
        });
    }
    drop(index);
    let msg = commit.message().unwrap_or("cherry-pick").to_string();
    finish_in_progress_commit(&repo, &msg, &[])?;
    Ok(MergeResult {
        status: "merged".into(),
        conflicts: vec![],
    })
}

#[tauri::command]
pub fn revert_commit(state: State<AppState>, oid: String) -> Result<MergeResult, String> {
    let repo = open_repo(&state)?;
    let oid = Oid::from_str(&oid).map_err(|e| e.message().to_string())?;
    let commit = repo.find_commit(oid).map_err(|e| e.message().to_string())?;
    let mut ro = git2::RevertOptions::new();
    repo.revert(&commit, Some(&mut ro))
        .map_err(|e| e.message().to_string())?;
    let index = repo.index().map_err(|e| e.message().to_string())?;
    if index.has_conflicts() {
        return Ok(MergeResult {
            status: "conflicts".into(),
            conflicts: conflicts_list(&repo),
        });
    }
    drop(index);
    let msg = format!(
        "Revert \"{}\"\n\nThis reverts commit {}.",
        commit.summary().unwrap_or(""),
        oid
    );
    finish_in_progress_commit(&repo, &msg, &[])?;
    Ok(MergeResult {
        status: "merged".into(),
        conflicts: vec![],
    })
}

#[tauri::command]
pub fn reset_to_commit(state: State<AppState>, oid: String, mode: String) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let oid = Oid::from_str(&oid).map_err(|e| e.message().to_string())?;
    let obj = repo
        .find_object(oid, Some(ObjectType::Commit))
        .map_err(|e| e.message().to_string())?;
    let rt = match mode.as_str() {
        "soft" => ResetType::Soft,
        "hard" => ResetType::Hard,
        _ => ResetType::Mixed,
    };
    let mut cb = git2::build::CheckoutBuilder::new();
    cb.force();
    repo.reset(&obj, rt, Some(&mut cb))
        .map_err(|e| e.message().to_string())?;
    Ok(())
}

#[tauri::command]
pub fn checkout_commit(state: State<AppState>, oid: String) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let oid = Oid::from_str(&oid).map_err(|e| e.message().to_string())?;
    repo.set_head_detached(oid).map_err(|e| e.message().to_string())?;
    let mut cb = git2::build::CheckoutBuilder::new();
    cb.safe();
    repo.checkout_head(Some(&mut cb))
        .map_err(|e| e.message().to_string())?;
    Ok(())
}
