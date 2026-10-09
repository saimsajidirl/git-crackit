use crate::helpers::*;
use crate::state::AppState;
use crate::types::*;
use git2::{BranchType, FetchOptions, PushOptions};
use tauri::{AppHandle, Emitter, Runtime, State};

#[tauri::command]
pub fn list_remotes(state: State<AppState>) -> Result<Vec<RemoteInfo>, String> {
    let repo = open_repo(&state)?;
    let names = repo.remotes().map_err(|e| e.message().to_string())?;
    let mut out = Vec::new();
    for name in names.iter().flatten() {
        if let Ok(r) = repo.find_remote(name) {
            out.push(RemoteInfo {
                name: name.to_string(),
                url: r.url().map(|s| s.to_string()),
                push_url: r.pushurl().map(|s| s.to_string()),
            });
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn add_remote(state: State<AppState>, name: String, url: String) -> Result<(), String> {
    let repo = open_repo(&state)?;
    repo.remote(&name, &url).map_err(|e| e.message().to_string())?;
    Ok(())
}

#[tauri::command]
pub fn remove_remote(state: State<AppState>, name: String) -> Result<(), String> {
    let repo = open_repo(&state)?;
    repo.remote_delete(&name).map_err(|e| e.message().to_string())?;
    Ok(())
}

#[tauri::command]
pub fn rename_remote(state: State<AppState>, old: String, new: String) -> Result<(), String> {
    let repo = open_repo(&state)?;
    repo.remote_rename(&old, &new).map_err(|e| e.message().to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_remote_url(state: State<AppState>, name: String, url: String) -> Result<(), String> {
    let repo = open_repo(&state)?;
    repo.remote_set_url(&name, &url).map_err(|e| e.message().to_string())?;
    Ok(())
}

fn default_remote(repo: &git2::Repository) -> Result<String, String> {
    let names = repo.remotes().map_err(|e| e.message().to_string())?;
    let list: Vec<String> = names.iter().flatten().map(|s| s.to_string()).collect();
    if list.iter().any(|n| n == "origin") {
        return Ok("origin".into());
    }
    list.into_iter()
        .next()
        .ok_or_else(|| "No remotes configured".to_string())
}

/// Remote used by current branch's upstream, else default.
fn upstream_remote(repo: &git2::Repository) -> Result<String, String> {
    if let Ok(h) = repo.head() {
        if h.is_branch() {
            if let Some(short) = h.shorthand() {
                if let Ok(b) = repo.find_branch(short, BranchType::Local) {
                    if let Ok(name) = b.upstream() {
                        if let Ok(Some(full)) = name.name() {
                            if let Some((remote, _)) = full.split_once('/') {
                                return Ok(remote.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    default_remote(repo)
}

fn fetch_one(repo: &git2::Repository, remote_name: &str, creds: crate::state::Credentials) -> Result<(), String> {
    let mut remote = repo
        .find_remote(remote_name)
        .map_err(|e| format!("Remote '{}': {}", remote_name, e.message()))?;
    let cb = remote_callbacks(creds);
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(cb);
    remote
        .fetch(&[] as &[&str], Some(&mut fo), None)
        .map_err(|e| {
            let m = e.message().to_string();
            if m.contains("authentication") || m.contains("credentials") || m.contains("401") || m.contains("403") {
                format!("AUTH: {}", m)
            } else {
                m
            }
        })
}

#[tauri::command]
pub fn fetch_remote<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    remote: Option<String>,
) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let creds = get_creds(&state);
    let name = match remote.filter(|s| !s.is_empty()) {
        Some(r) => r,
        None => upstream_remote(&repo)?,
    };
    let _ = app.emit("op-progress", OpProgress { op: "fetch".into(), received: 0, total: 0, path: name.clone() });
    let res = fetch_one(&repo, &name, creds);
    let _ = app.emit("op-done", "fetch");
    res
}

#[tauri::command]
pub fn fetch_all(state: State<AppState>) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let creds = get_creds(&state);
    let names = repo.remotes().map_err(|e| e.message().to_string())?;
    let mut errs = Vec::new();
    for n in names.iter().flatten() {
        if let Err(e) = fetch_one(&repo, n, creds.clone()) {
            errs.push(format!("{}: {}", n, e));
        }
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs.join("; "))
    }
}

#[tauri::command]
pub fn pull(state: State<AppState>) -> Result<MergeResult, String> {
    let repo = open_repo(&state)?;
    let creds = get_creds(&state);
    let remote_name = upstream_remote(&repo)?;
    fetch_one(&repo, &remote_name, creds)?;

    // Merge FETCH_HEAD
    let fetch_head = repo
        .find_reference("FETCH_HEAD")
        .map_err(|_| "Nothing fetched".to_string())?;
    let annotated = repo
        .reference_to_annotated_commit(&fetch_head)
        .map_err(|e| e.message().to_string())?;
    let branch_label = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| format!("{}/{}", remote_name, s)))
        .unwrap_or_else(|| format!("{}", remote_name));
    merge_annotated(&repo, &annotated, &branch_label)
}

#[tauri::command]
pub fn push(state: State<AppState>, set_upstream: Option<bool>) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let head = repo.head().map_err(|_| "Nothing to push (no commits yet)".to_string())?;
    if !head.is_branch() {
        return Err("Cannot push while HEAD is detached. Checkout a branch first.".into());
    }
    let branch_name = head.shorthand().unwrap_or("").to_string();
    let remote_name = upstream_remote(&repo).unwrap_or_else(|_| "origin".into());
    if repo.find_remote(&remote_name).is_err() {
        return Err(format!("Remote '{}' does not exist", remote_name));
    }
    let creds = get_creds(&state);
    let mut remote = repo.find_remote(&remote_name).map_err(|e| e.message().to_string())?;
    let cb = remote_callbacks(creds);
    let mut po = PushOptions::new();
    po.remote_callbacks(cb);
    let refspec = format!("refs/heads/{0}:refs/heads/{0}", branch_name);
    remote
        .push(&[refspec.as_str()], Some(&mut po))
        .map_err(|e| {
            let m = e.message().to_string();
            if m.contains("authentication") || m.contains("credentials") || m.contains("401") || m.contains("403") {
                format!("AUTH: {}", m)
            } else {
                m
            }
        })?;

    if set_upstream.unwrap_or(true) {
        if let Ok(mut b) = repo.find_branch(&branch_name, BranchType::Local) {
            let _ = b.set_upstream(Some(&format!("{}/{}", remote_name, branch_name)));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn push_tag(state: State<AppState>, name: String, remote: Option<String>) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let remote_name = remote.filter(|s| !s.is_empty()).map(Ok).unwrap_or_else(|| default_remote(&repo))?;
    let creds = get_creds(&state);
    let mut r = repo.find_remote(&remote_name).map_err(|e| e.message().to_string())?;
    let cb = remote_callbacks(creds);
    let mut po = PushOptions::new();
    po.remote_callbacks(cb);
    let refspec = format!("refs/tags/{0}:refs/tags/{0}", name);
    r.push(&[refspec.as_str()], Some(&mut po))
        .map_err(|e| e.message().to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_branch_upstream(
    state: State<AppState>,
    branch: String,
    upstream: String,
) -> Result<(), String> {
    let repo = open_repo(&state)?;
    let mut b = repo
        .find_branch(&branch, BranchType::Local)
        .map_err(|e| e.message().to_string())?;
    if upstream.is_empty() {
        b.set_upstream(None).map_err(|e| e.message().to_string())?;
    } else {
        b.set_upstream(Some(&upstream)).map_err(|e| e.message().to_string())?;
    }
    Ok(())
}
