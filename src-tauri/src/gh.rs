use crate::helpers::{get_creds, open_repo};
use crate::state::AppState;
use crate::types::*;
use serde_json::Value;
use std::time::Duration;
use tauri::State;

/// GitHub OAuth App client id for the device flow. Set at runtime via
/// GIT_CRACKIT_GH_CLIENT_ID, or hardcode one here for your own builds.
fn client_id() -> Option<String> {
    std::env::var("GIT_CRACKIT_GH_CLIENT_ID")
        .ok()
        .filter(|s| !s.is_empty())
        .or(option_env!("GITHUB_CLIENT_ID").map(|s| s.to_string()))
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent("git-crackit")
        .build()
        .into()
}

fn gh_err(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(401) => {
            "GitHub token rejected (401). Check the token or sign in again.".into()
        }
        ureq::Error::StatusCode(403) => {
            "GitHub API forbidden (403) — rate limited or token lacks scopes.".into()
        }
        ureq::Error::StatusCode(404) => {
            "Not found on GitHub (404). Is the repo private? Token may lack `repo` scope.".into()
        }
        ureq::Error::StatusCode(c) => format!("GitHub API error (HTTP {})", c),
        e => format!("GitHub API error: {}", e),
    }
}

fn api_get(token: &str, path: &str) -> Result<Value, String> {
    let url = format!("https://api.github.com{}", path);
    let mut resp = agent()
        .get(&url)
        .header("Authorization", &format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .map_err(gh_err)?;
    resp.body_mut()
        .read_json::<Value>()
        .map_err(|e| format!("Bad GitHub response: {}", e))
}

fn token(state: &State<AppState>) -> Result<String, String> {
    get_creds(state)
        .password
        .filter(|p| !p.is_empty())
        .ok_or_else(|| "NO_TOKEN: sign in with GitHub or enter a token first".to_string())
}

/// Parse `owner/repo` from a GitHub remote URL (https or ssh forms).
pub fn github_repo_from_url(url: &str) -> Option<GhRepo> {
    let path = if let Some(rest) = url.strip_prefix("git@github.com:") {
        rest.to_string()
    } else if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        rest.to_string()
    } else if let Some(rest) = url.strip_prefix("ssh://git@github.com/") {
        rest.to_string()
    } else {
        return None;
    };
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let (owner, name) = path.split_once('/')?;
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    Some(GhRepo {
        owner: owner.to_string(),
        name: name.to_string(),
        url: format!("https://github.com/{}/{}", owner, name),
    })
}

fn current_repo(state: &State<AppState>) -> Result<(git2::Repository, GhRepo), String> {
    let repo = open_repo(state)?;
    let names = repo.remotes().map_err(|e| e.message().to_string())?;
    let mut ordered: Vec<String> = names.iter().flatten().map(|s| s.to_string()).collect();
    ordered.sort_by_key(|n| if n == "origin" { 0 } else { 1 });
    for n in ordered {
        let url = repo
            .find_remote(&n)
            .ok()
            .and_then(|r| r.url().map(|u| u.to_string()));
        if let Some(gh) = url.and_then(|u| github_repo_from_url(&u)) {
            return Ok((repo, gh));
        }
    }
    Err("No GitHub remote on this repository".to_string())
}

fn s(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

/* ---------------- Commands ---------------- */

#[tauri::command]
pub fn github_repo(state: State<AppState>) -> Result<Option<GhRepo>, String> {
    match current_repo(&state) {
        Ok((_, gh)) => Ok(Some(gh)),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub fn github_user(state: State<AppState>) -> Result<GhUser, String> {
    let t = token(&state)?;
    let v = api_get(&t, "/user")?;
    Ok(GhUser {
        login: s(&v, "login"),
        name: v.get("name").and_then(|x| x.as_str()).map(|x| x.to_string()),
        avatar_url: v
            .get("avatar_url")
            .and_then(|x| x.as_str())
            .map(|x| x.to_string()),
    })
}

#[tauri::command]
pub fn github_prs(state: State<AppState>) -> Result<Vec<GhPR>, String> {
    let t = token(&state)?;
    let (_, gh) = current_repo(&state)?;
    let v = api_get(&t, &format!("/repos/{}/{}/pulls?state=open&per_page=50", gh.owner, gh.name))?;
    let arr = v.as_array().cloned().unwrap_or_default();
    Ok(arr
        .iter()
        .map(|p| GhPR {
            number: p.get("number").and_then(|x| x.as_u64()).unwrap_or(0),
            title: s(p, "title"),
            author: s(&p["user"], "login"),
            avatar_url: p["user"]
                .get("avatar_url")
                .and_then(|x| x.as_str())
                .map(|x| x.to_string()),
            head_ref: s(&p["head"], "ref"),
            head_sha: s(&p["head"], "sha"),
            base_ref: s(&p["base"], "ref"),
            draft: p.get("draft").and_then(|x| x.as_bool()).unwrap_or(false),
            html_url: s(p, "html_url"),
            updated_at: s(p, "updated_at"),
        })
        .collect())
}

#[tauri::command]
pub fn github_issues(state: State<AppState>) -> Result<Vec<GhIssue>, String> {
    let t = token(&state)?;
    let (_, gh) = current_repo(&state)?;
    let v = api_get(&t, &format!("/repos/{}/{}/issues?state=open&per_page=50", gh.owner, gh.name))?;
    let arr = v.as_array().cloned().unwrap_or_default();
    Ok(arr
        .iter()
        .filter(|i| i.get("pull_request").is_none()) // issues API also returns PRs
        .map(|i| GhIssue {
            number: i.get("number").and_then(|x| x.as_u64()).unwrap_or(0),
            title: s(i, "title"),
            author: s(&i["user"], "login"),
            avatar_url: i["user"]
                .get("avatar_url")
                .and_then(|x| x.as_str())
                .map(|x| x.to_string()),
            labels: i["labels"]
                .as_array()
                .map(|ls| {
                    ls.iter()
                        .map(|l| GhLabel {
                            name: s(l, "name"),
                            color: s(l, "color"),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            comments: i.get("comments").and_then(|x| x.as_u64()).unwrap_or(0),
            html_url: s(i, "html_url"),
            created_at: s(i, "created_at"),
        })
        .collect())
}

/// Aggregate CI status for a commit: check runs first, legacy statuses fallback.
#[tauri::command]
pub fn github_check_runs(state: State<AppState>, sha: String) -> Result<GhCheckStatus, String> {
    let t = token(&state)?;
    let (_, gh) = current_repo(&state)?;
    let v = api_get(
        &t,
        &format!(
            "/repos/{}/{}/commits/{}/check-runs?per_page=100",
            gh.owner, gh.name, sha
        ),
    )?;
    let runs = v["check_runs"].as_array().cloned().unwrap_or_default();
    if !runs.is_empty() {
        let mut failed = 0usize;
        let mut pending = 0usize;
        for r in &runs {
            match s(r, "conclusion").as_str() {
                "failure" | "timed_out" | "cancelled" | "action_required" => failed += 1,
                "success" | "neutral" | "skipped" => {}
                _ => {
                    // conclusion is null while running → check status field
                    match s(r, "status").as_str() {
                        "queued" | "in_progress" | "waiting" | "requested" | "pending" => {
                            pending += 1
                        }
                        _ => pending += 1,
                    }
                }
            }
        }
        return Ok(GhCheckStatus {
            status: if failed > 0 {
                "failure"
            } else if pending > 0 {
                "pending"
            } else {
                "success"
            }
            .to_string(),
            total: runs.len(),
            failed,
        });
    }
    // Legacy commit statuses (non-Actions CI)
    let v = api_get(&t, &format!("/repos/{}/{}/commits/{}/status", gh.owner, gh.name, sha))?;
    let state_str = s(&v, "state");
    let total = v.get("total_count").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    Ok(GhCheckStatus {
        status: match state_str.as_str() {
            "success" => "success",
            "failure" | "error" => "failure",
            "pending" => "pending",
            _ => "none",
        }
        .to_string(),
        total,
        failed: 0,
    })
}

/* ---------------- OAuth device flow ---------------- */

#[tauri::command]
pub fn github_device_start() -> Result<DeviceFlow, String> {
    let cid = client_id()
        .ok_or_else(|| "NO_CLIENT_ID: GitHub OAuth app not configured".to_string())?;
    let mut resp = agent()
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .send_form([("client_id", cid.as_str()), ("scope", "repo")])
        .map_err(gh_err)?;
    let v = resp
        .body_mut()
        .read_json::<Value>()
        .map_err(|e| e.to_string())?;
    Ok(DeviceFlow {
        device_code: s(&v, "device_code"),
        user_code: s(&v, "user_code"),
        verification_uri: s(&v, "verification_uri"),
        interval: v.get("interval").and_then(|x| x.as_u64()).unwrap_or(5),
    })
}

#[tauri::command]
pub fn github_device_poll(device_code: String) -> Result<DevicePoll, String> {
    let cid = client_id()
        .ok_or_else(|| "NO_CLIENT_ID: GitHub OAuth app not configured".to_string())?;
    let mut resp = agent()
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .send_form([
            ("client_id", cid.as_str()),
            ("device_code", device_code.as_str()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .map_err(gh_err)?;
    let v = resp
        .body_mut()
        .read_json::<Value>()
        .map_err(|e| e.to_string())?;
    if let Some(tok) = v.get("access_token").and_then(|x| x.as_str()) {
        return Ok(DevicePoll {
            status: "authorized".into(),
            token: Some(tok.to_string()),
        });
    }
    let status = match s(&v, "error").as_str() {
        "authorization_pending" => "pending",
        "slow_down" => "slow_down",
        _ => "expired",
    };
    Ok(DevicePoll {
        status: status.into(),
        token: None,
    })
}
