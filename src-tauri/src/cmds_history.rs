use crate::helpers::*;
use crate::state::AppState;
use crate::types::*;
use git2::{DiffOptions, ObjectType, Oid, ResetType, Sort};
use tauri::State;

fn diff_to_changed_files(diff: &mut git2::Diff) -> Vec<ChangedFile> {
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
        let (adds, dels) = match git2::Patch::from_diff(diff, i) {
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

fn diff_changed_files(repo: &git2::Repository, commit: &git2::Commit) -> Vec<ChangedFile> {
    let new_tree = commit.tree().ok();
    let old_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
    let mut opts = DiffOptions::new();
    opts.include_untracked(false).recurse_untracked_dirs(false);
    match repo.diff_tree_to_tree(old_tree.as_ref(), new_tree.as_ref(), Some(&mut opts)) {
        Ok(mut diff) => diff_to_changed_files(&mut diff),
        Err(_) => vec![],
    }
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
        out.push(commit_info(&repo, &commit, &labels));
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
        commit: commit_info(&repo, &commit, &labels),
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
    let undo = head_point(&repo, &format!("Cherry-pick {}", commit.id().to_string().get(..7).unwrap_or("")), false);
    let mut co = git2::CherrypickOptions::new();
    repo.cherrypick(&commit, Some(&mut co))
        .map_err(|e| e.message().to_string())?;
    let index = repo.index().map_err(|e| e.message().to_string())?;
    if index.has_conflicts() {
        record_undo(&state, undo);
        return Ok(MergeResult {
            status: "conflicts".into(),
            conflicts: conflicts_list(&repo),
        });
    }
    drop(index);
    let msg = commit.message().unwrap_or("cherry-pick").to_string();
    finish_in_progress_commit(&repo, &msg, &[])?;
    record_undo(&state, undo);
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
    let undo = head_point(&repo, &format!("Revert {}", commit.id().to_string().get(..7).unwrap_or("")), false);
    let mut ro = git2::RevertOptions::new();
    repo.revert(&commit, Some(&mut ro))
        .map_err(|e| e.message().to_string())?;
    let index = repo.index().map_err(|e| e.message().to_string())?;
    if index.has_conflicts() {
        record_undo(&state, undo);
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
    record_undo(&state, undo);
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
    let undo = head_point(&repo, &format!("Reset --{}", mode), false);
    let mut cb = git2::build::CheckoutBuilder::new();
    cb.force();
    repo.reset(&obj, rt, Some(&mut cb))
        .map_err(|e| e.message().to_string())?;
    record_undo(&state, undo);
    Ok(())
}

fn checkout_commit_inner(repo: &git2::Repository, oid: Oid) -> Result<(), String> {
    let commit = repo
        .find_commit(oid)
        .map_err(|e| e.message().to_string())?;
    // Checkout the target tree first, then move HEAD — see checkout_branch_inner.
    let mut cb = git2::build::CheckoutBuilder::new();
    cb.safe();
    repo.checkout_tree(commit.as_object(), Some(&mut cb))
        .map_err(|e| e.message().to_string())?;
    repo.set_head_detached(oid).map_err(|e| e.message().to_string())?;
    lfs_checkout(repo);
    Ok(())
}

/// `dirty`: "ask" (default — report `status:"dirty"` instead of switching),
/// "carry" (take the changes to the target commit), "stash" (keep them stashed).
#[tauri::command]
pub fn checkout_commit(
    state: State<AppState>,
    oid: String,
    dirty: Option<String>,
) -> Result<CheckoutOutcome, String> {
    let mut repo = open_repo(&state)?;
    let oid = Oid::from_str(&oid).map_err(|e| e.message().to_string())?;
    let mode = dirty.as_deref().unwrap_or("ask");
    let undo = head_point(&repo, &format!("Checkout {}", oid.to_string().get(..7).unwrap_or("")), false);
    let out = dirty_checkout(&mut repo, mode, |r| checkout_commit_inner(r, oid))?;
    if out.status != "dirty" {
        record_undo(&state, undo);
    }
    Ok(out)
}

/// Undo the most recent recorded op — restores the HEAD ref/oid snapshot the
/// op took, with worktree restored via `git reset --merge` (dirty work that
/// doesn't intersect is preserved; the stash side-effects of "carry"/"stash"
/// modes stay in the stash list). Returns a description for the toast.
#[tauri::command]
pub fn undo_last(state: State<AppState>) -> Result<String, String> {
    let repo = open_repo(&state)?;
    let point = state
        .undo_stack
        .lock()
        .map_err(|e| e.to_string())?
        .pop()
        .ok_or_else(|| "Nothing to undo".to_string())?;
    let redo = head_point(&repo, &point.label, point.soft);
    match restore_point(&repo, &point) {
        Ok(()) => {
            if let Some(r) = redo {
                if let Ok(mut s) = state.redo_stack.lock() {
                    s.push(r);
                }
            }
            lfs_checkout(&repo);
            Ok(format!("Undid {}", point.label))
        }
        Err(e) => {
            if let Ok(mut s) = state.undo_stack.lock() {
                s.push(point);
            }
            Err(e)
        }
    }
}

#[tauri::command]
pub fn redo_last(state: State<AppState>) -> Result<String, String> {
    let repo = open_repo(&state)?;
    let point = state
        .redo_stack
        .lock()
        .map_err(|e| e.to_string())?
        .pop()
        .ok_or_else(|| "Nothing to redo".to_string())?;
    let undo = head_point(&repo, &point.label, point.soft);
    match restore_point(&repo, &point) {
        Ok(()) => {
            if let Some(u) = undo {
                if let Ok(mut s) = state.undo_stack.lock() {
                    s.push(u);
                }
            }
            lfs_checkout(&repo);
            Ok(format!("Redid {}", point.label))
        }
        Err(e) => {
            if let Ok(mut s) = state.redo_stack.lock() {
                s.push(point);
            }
            Err(e)
        }
    }
}

/* ---------------- File history & branch compare ---------------- */

/// `git log --follow <path>` — commits touching `path` across all refs, with a
/// rename-follow approximation: when a commit renames into the tracked path,
/// the walk continues against the old name.
#[tauri::command]
pub fn file_history(state: State<AppState>, path: String) -> Result<Vec<CommitInfo>, String> {
    let repo = open_repo(&state)?;
    let mut walk = repo.revwalk().map_err(|e| e.message().to_string())?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)
        .map_err(|e| e.message().to_string())?;
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

    let labels = ref_labels(&repo);
    let mut cur = path;
    let mut out = Vec::new();
    for oid in walk.flatten() {
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let new_tree = commit.tree().ok();
        let old_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
        // Full diff — a pathspec would filter out the rename's old-side delta
        // before find_similar can pair it, breaking the follow.
        let mut diff = match repo.diff_tree_to_tree(old_tree.as_ref(), new_tree.as_ref(), None) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let _ = diff.find_similar(None);
        let cur_path = std::path::PathBuf::from(&cur);
        let mut touches = false;
        let mut renamed_from: Option<String> = None;
        for i in 0..diff.deltas().len() {
            let delta = diff.get_delta(i).unwrap();
            let old_p = delta.old_file().path();
            let new_p = delta.new_file().path();
            if new_p == Some(cur_path.as_path()) || old_p == Some(cur_path.as_path()) {
                touches = true;
                // Follow a rename INTO the tracked path: keep walking under the old name.
                if delta.status() == git2::Delta::Renamed && new_p == Some(cur_path.as_path()) {
                    renamed_from = old_p.map(|o| o.to_string_lossy().to_string());
                }
            }
        }
        if let Some(o) = renamed_from {
            cur = o;
        }
        if touches {
            out.push(commit_info(&repo, &commit, &labels));
            if out.len() >= 500 {
                break;
            }
        }
    }
    Ok(out)
}

/// `git diff base...compare` — merge-base→compare file list + ahead/behind.
#[tauri::command]
pub fn compare_refs(
    state: State<AppState>,
    base: String,
    compare: String,
) -> Result<CompareResult, String> {
    let repo = open_repo(&state)?;
    let base_c = repo
        .revparse_single(&base)
        .and_then(|o| o.peel_to_commit())
        .map_err(|e| format!("Cannot resolve '{}': {}", base, e.message()))?;
    let cmp_c = repo
        .revparse_single(&compare)
        .and_then(|o| o.peel_to_commit())
        .map_err(|e| format!("Cannot resolve '{}': {}", compare, e.message()))?;
    let (ahead, behind) = repo
        .graph_ahead_behind(cmp_c.id(), base_c.id())
        .map_err(|e| e.message().to_string())?;
    // Three-dot semantics: what `compare` changed relative to the merge base.
    let mb = repo
        .merge_base(base_c.id(), cmp_c.id())
        .unwrap_or_else(|_| base_c.id());
    let mb_tree = repo
        .find_commit(mb)
        .and_then(|c| c.tree())
        .map_err(|e| e.message().to_string())?;
    let cmp_tree = cmp_c.tree().map_err(|e| e.message().to_string())?;
    let mut diff = repo
        .diff_tree_to_tree(Some(&mb_tree), Some(&cmp_tree), None)
        .map_err(|e| e.message().to_string())?;
    Ok(CompareResult {
        files: diff_to_changed_files(&mut diff),
        ahead,
        behind,
    })
}

/// The `base...compare` diff for a single file.
#[tauri::command]
pub fn compare_file_diff(
    state: State<AppState>,
    base: String,
    compare: String,
    path: String,
) -> Result<FileDiff, String> {
    let repo = open_repo(&state)?;
    let base_c = repo
        .revparse_single(&base)
        .and_then(|o| o.peel_to_commit())
        .map_err(|e| format!("Cannot resolve '{}': {}", base, e.message()))?;
    let cmp_c = repo
        .revparse_single(&compare)
        .and_then(|o| o.peel_to_commit())
        .map_err(|e| format!("Cannot resolve '{}': {}", compare, e.message()))?;
    let mb = repo
        .merge_base(base_c.id(), cmp_c.id())
        .unwrap_or_else(|_| base_c.id());
    let mb_tree = repo
        .find_commit(mb)
        .and_then(|c| c.tree())
        .map_err(|e| e.message().to_string())?;
    let cmp_tree = cmp_c.tree().map_err(|e| e.message().to_string())?;
    let mut opts = DiffOptions::new();
    opts.pathspec(&path).context_lines(3);
    let mut diff = repo
        .diff_tree_to_tree(Some(&mb_tree), Some(&cmp_tree), Some(&mut opts))
        .map_err(|e| e.message().to_string())?;
    let _ = diff.find_similar(None);
    find_file_in_diff(&mut diff, &path)
}
