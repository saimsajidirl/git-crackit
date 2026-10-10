use crate::helpers::*;
use crate::state::{self, AppState, Credentials};
use crate::types::*;
use git2::{FetchOptions, Repository};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Runtime, State};

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
        uses_lfs: repo_uses_lfs(repo),
        lfs_installed: lfs_available(),
    })
}

#[tauri::command]
pub fn open_repository<R: Runtime>(app: AppHandle<R>, state: State<AppState>, path: String) -> Result<RepoInfo, String> {
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
    crate::watcher::start(&app, &state, &root);
    state::add_recent(&info.path);
    Ok(info)
}

#[tauri::command]
pub fn init_repository<R: Runtime>(app: AppHandle<R>, state: State<AppState>, path: String, bare: Option<bool>) -> Result<RepoInfo, String> {
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
    crate::watcher::start(&app, &state, &p);
    state::add_recent(&info.path);
    Ok(info)
}

#[tauri::command]
pub fn clone_repository<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    url: String,
    dest: String,
) -> Result<RepoInfo, String> {
    let creds = get_creds(&state);
    let dest_path = PathBuf::from(&dest);
    let repo = if is_ssh_url(&url) {
        // SSH transport: shell out to `git` (uses user's ssh-agent/config).
        git_clone(&app, &url, &dest_path, &creds)?;
        Repository::open(&dest_path).map_err(|e| e.message().to_string())?
    } else {
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

        builder
            .clone(&url, &dest_path)
            .map_err(|e| e.message().to_string())?
    };
    lfs_pull(&repo, &get_creds(&state));

    let info = repo_info(&repo, &dest_path)?;
    *state.repo_path.lock().map_err(|e| e.to_string())? = Some(dest_path.clone());
    crate::watcher::start(&app, &state, &dest_path);
    state::add_recent(&info.path);
    Ok(info)
}

#[tauri::command]
pub fn close_repository(state: State<AppState>) -> Result<(), String> {
    *state.repo_path.lock().map_err(|e| e.to_string())? = None;
    crate::watcher::stop(&state);
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
    remember: Option<bool>,
) -> Result<(), String> {
    let creds = Credentials {
        username: username.filter(|s| !s.is_empty()),
        password: password.filter(|s| !s.is_empty()),
    };
    *state.creds.lock().map_err(|e| e.to_string())? = creds.clone();
    if remember.unwrap_or(false) {
        state::save_credentials(if creds.username.is_some() || creds.password.is_some() {
            Some(creds)
        } else {
            None
        });
    } else {
        state::save_credentials(None);
    }
    Ok(())
}

#[tauri::command]
pub fn get_credentials(state: State<AppState>) -> Result<CredentialStatus, String> {
    let creds = get_creds(&state);
    Ok(CredentialStatus {
        username: creds.username,
        has_password: creds.password.is_some(),
        remembered: state::credentials_remembered(),
    })
}

#[tauri::command]
pub fn clear_credentials(state: State<AppState>) -> Result<(), String> {
    *state.creds.lock().map_err(|e| e.to_string())? = Credentials::default();
    state::save_credentials(None);
    Ok(())
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("Only https:// URLs can be opened".to_string());
    }
    #[cfg(target_os = "windows")]
    let res = std::process::Command::new("cmd")
        .args(["/C", "start", "", &url])
        .spawn();
    #[cfg(target_os = "macos")]
    let res = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let res = std::process::Command::new("xdg-open").arg(&url).spawn();
    res.map(|_| ()).map_err(|e| format!("Could not open browser: {}", e))
}

/// List private keys under ~/.ssh (first bytes must look like a private key).
#[tauri::command]
pub fn list_ssh_keys() -> Result<Vec<String>, String> {
    let home = dirs::home_dir().ok_or("Could not locate home directory")?;
    let ssh_dir = home.join(".ssh");
    let mut keys = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&ssh_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if !p.is_file() {
                continue;
            }
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with(".pub")
                || matches!(name, "known_hosts" | "config" | "authorized_keys" | "authorized_keys2" | "environment" | "rc")
                || name.starts_with("known_hosts")
            {
                continue;
            }
            let head = std::fs::File::open(&p)
                .ok()
                .and_then(|mut f| {
                    use std::io::Read;
                    let mut buf = [0u8; 128];
                    f.read(&mut buf).ok().map(|n| String::from_utf8_lossy(&buf[..n]).to_string())
                })
                .unwrap_or_default();
            if head.contains("PRIVATE KEY") {
                keys.push(p.to_string_lossy().to_string());
            }
        }
    }
    keys.sort();
    Ok(keys)
}

/// Load a (possibly passphrase-protected) key into the running ssh-agent.
/// The passphrase reaches `ssh-add` through an SSH_ASKPASS helper script —
/// the GUI never blocks on a terminal prompt.
#[tauri::command]
pub fn load_ssh_key(path: String, passphrase: String) -> Result<String, String> {
    if !Path::new(&path).is_file() {
        return Err(format!("Key file not found: {}", path));
    }
    // One-shot askpass: on a wrong passphrase ssh-add re-invokes SSH_ASKPASS in
    // a tight retry loop, so the script must fail on the second call.
    let pid = std::process::id();
    let script = std::env::temp_dir().join(format!("gc-askpass-{}.sh", pid));
    let count = std::env::temp_dir().join(format!("gc-askpass-{}.n", pid));
    let _ = std::fs::remove_file(&count);
    std::fs::write(
        &script,
        "#!/bin/sh\nn=$(cat \"$GC_ASKPASS_N\" 2>/dev/null || echo 0)\nn=$((n + 1))\necho \"$n\" > \"$GC_ASKPASS_N\"\n[ \"$n\" -gt 1 ] && exit 1\nprintf '%s\\n' \"$GC_ASKPASS_PW\"\n",
    )
    .map_err(|e| format!("Could not write askpass helper: {}", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700));
    }
    let mut child = std::process::Command::new("ssh-add")
        .arg(&path)
        .env("SSH_ASKPASS", &script)
        .env("SSH_ASKPASS_REQUIRE", "force")
        .env("DISPLAY", "git-crackit:0")
        .env("GC_ASKPASS_PW", &passphrase)
        .env("GC_ASKPASS_N", &count)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| {
            let _ = std::fs::remove_file(&script);
            "ssh-add not found — install the OpenSSH client tools.".to_string()
        })?;
    // Hard timeout as a safety net in case askpass handling ever stalls.
    let started = std::time::Instant::now();
    let out = loop {
        match child.try_wait() {
            Ok(Some(_)) => break child.wait_with_output(),
            Ok(None) if started.elapsed().as_secs() > 15 => {
                let _ = child.kill();
                break child.wait_with_output();
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(e) => {
                let _ = child.kill();
                break Err(std::io::Error::new(std::io::ErrorKind::Other, e));
            }
        }
    };
    let _ = std::fs::remove_file(&script);
    let _ = std::fs::remove_file(&count);
    let out = out.map_err(|e| format!("ssh-add failed: {}", e))?;
    if out.status.success() {
        return Ok("Key loaded into ssh-agent. Retry the operation.".to_string());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let lower = stderr.to_lowercase();
    if lower.contains("could not open a connection") {
        Err("ssh-agent isn't running — start it (`eval $(ssh-agent)`, or enable the \"OpenSSH Authentication Agent\" service on Windows), then retry.".into())
    } else if lower.contains("incorrect passphrase") || lower.contains("bad passphrase") {
        Err("Incorrect passphrase for that key.".into())
    } else {
        Err(format!("ssh-add failed: {}", stderr.trim()))
    }
}
