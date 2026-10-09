use crate::helpers::*;
use crate::state::{self, AppState, Credentials};
use crate::types::*;
use git2::{FetchOptions, Repository};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};

pub fn repo_info(repo: &Repository, path: &Path) -> Result<RepoInfo, String> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repository".to_string());

    let is_unborn = repo.head().is_err();
    let is_detached = repo.head_detached().unwrap_or(false);
    let head = if is_unborn {
        // e.g. on an unborn branch, read the symbolic ref name
        repo.find_reference("HEAD")
            .ok()
            .and_then(|r| r.symbolic_target().map(|s| s.to_string()))
            .and_then(|s| s.strip_prefix("refs/heads/").map(|x| x.to_string()))
    } else {
        let h = repo.head().map_err(|e| e.message().to_string())?;
        if is_detached {
            h.target()
                .map(|o| o.to_string().chars().take(7).collect())
        } else {
            h.shorthand().map(|s| s.to_string())
        }
    };

    let state_str = match repo.state() {
        git2::RepositoryState::Merge => "merging",
        git2::RepositoryState::Revert => "reverting",
        git2::RepositoryState::RevertSequence => "reverting",
        git2::RepositoryState::CherryPick => "cherry_picking",
        git2::RepositoryState::CherryPickSequence => "cherry_picking",
        git2::RepositoryState::Bisect => "bisecting",
        git2::RepositoryState::Rebase
        | git2::RepositoryState::RebaseInteractive
        | git2::RepositoryState::RebaseMerge
        | git2::RepositoryState::ApplyMailbox
        | git2::RepositoryState::ApplyMailboxOrRebase => "rebasing",
        git2::RepositoryState::Clean => "clean",
    }
    .to_string();

    let mut ahead = 0usize;
    let mut behind = 0usize;
    let mut upstream = None;
    if !is_unborn && !is_detached {
        if let Ok(h) = repo.head() {
            if let Ok(branch) = repo.find_branch(h.shorthand().unwrap_or(""), git2::BranchType::Local) {
                if let Ok(up) = branch.upstream() {
                    upstream = up.name().ok().flatten().map(|s| s.to_string());
                    if let (Some(l), Some(u)) = (h.target(), up.get().target()) {
                        if let Ok((a, b)) = repo.graph_ahead_behind(l, u) {
                            ahead = a;
                            behind = b;
                        }
                    }
                }
            }
        }
    }

    let remotes = repo
        .remotes()
        .map(|r| r.iter().flatten().map(|s| s.to_string()).collect())
        .unwrap_or_default();

    Ok(RepoInfo {
        path: path.to_string_lossy().to_string(),
        name,
        head,
        is_detached,
        state: state_str,
        ahead,
        behind,
        upstream,
        is_unborn,
        remotes,
    })
}

#[tauri::command]
pub fn open_repository(state: State<AppState>, path: String) -> Result<RepoInfo, String> {
    let p = PathBuf::from(&path);
    let repo = Repository::open(&p)
        .or_else(|_| Repository::open_ext(&p, git2::RepositoryOpenFlags::empty(), Vec::<&Path>::new()))
        .map_err(|e| format!("Not a git repository: {}", e.message()))?;
    // Use workdir root as canonical repo path
    let root = repo
        .workdir()
        .map(|w| w.to_path_buf())
        .unwrap_or_else(|| p.clone());
    let info = repo_info(&repo, &root)?;
    *state.repo_path.lock().map_err(|e| e.to_string())? = Some(root.clone());
    state::add_recent(&info.path);
    Ok(info)
}

#[tauri::command]
pub fn init_repository(state: State<AppState>, path: String, bare: Option<bool>) -> Result<RepoInfo, String> {
    let p = PathBuf::from(&path);
    std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    let repo = if bare.unwrap_or(false) {
        Repository::init_bare(&p)
    } else {
        Repository::init(&p)
    }
    .map_err(|e| e.message().to_string())?;
    let info = repo_info(&repo, &p)?;
    *state.repo_path.lock().map_err(|e| e.to_string())? = Some(p.clone());
    state::add_recent(&info.path);
    Ok(info)
}

#[tauri::command]
pub fn clone_repository(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    dest: String,
) -> Result<RepoInfo, String> {
    let creds = get_creds(&state);
    let mut cb = remote_callbacks(creds);
    let app2 = app.clone();
    let dest2 = dest.clone();
    cb.transfer_progress(move |p| {
        let _ = app2.emit(
            "clone-progress",
            OpProgress {
                op: "clone".into(),
                received: p.received_objects(),
                total: p.total_objects(),
                path: dest2.clone(),
            },
        );
        true
    });

    let mut fo = FetchOptions::new();
    fo.remote_callbacks(cb);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);

    let dest_path = PathBuf::from(&dest);
    let repo = builder
        .clone(&url, &dest_path)
        .map_err(|e| e.message().to_string())?;

    let info = repo_info(&repo, &dest_path)?;
    *state.repo_path.lock().map_err(|e| e.to_string())? = Some(dest_path);
    state::add_recent(&info.path);
    Ok(info)
}

#[tauri::command]
pub fn close_repository(state: State<AppState>) -> Result<(), String> {
    *state.repo_path.lock().map_err(|e| e.to_string())? = None;
    state::set_last(None);
    Ok(())
}

#[tauri::command]
pub fn repository_info(state: State<AppState>) -> Result<RepoInfo, String> {
    let repo = open_repo(&state)?;
    let p = repo.workdir().unwrap_or_else(|| repo.path()).to_path_buf();
    repo_info(&repo, &p)
}

#[tauri::command]
pub fn get_recent_repositories() -> RecentRepos {
    let st = state::load_persisted();
    RecentRepos {
        recent: st.recent,
        last: st.last_repo,
    }
}

#[tauri::command]
pub fn remove_recent_repository(path: String) -> Result<(), String> {
    state::remove_recent(&path);
    Ok(())
}

#[tauri::command]
pub fn validate_repository_path(path: String) -> Result<bool, String> {
    Ok(Repository::open(Path::new(&path)).is_ok())
}

#[tauri::command]
pub fn set_credentials(
    state: State<AppState>,
    username: Option<String>,
    password: Option<String>,
) -> Result<(), String> {
    *state.creds.lock().map_err(|e| e.to_string())? = Credentials { username, password };
    Ok(())
}
