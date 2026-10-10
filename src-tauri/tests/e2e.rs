use git_crackit::{cmds_branches, cmds_files, cmds_history, cmds_repo, cmds_stash_tags, state::AppState};
use tauri::Manager;

const DIR: &str = "/tmp/git-crackit-e2e";

#[test]
fn full_workflow() {
    let _ = std::fs::remove_dir_all(DIR);
    std::fs::create_dir_all(DIR).unwrap();

    let app = tauri::test::mock_app();
    app.manage(AppState::default());
    let handle = app.handle().clone();

    // init repo
    let info = cmds_repo::init_repository(handle.clone(), app.state(), DIR.into(), Some(false)).unwrap();
    assert_eq!(info.name, "git-crackit-e2e");
    assert!(info.is_unborn);
    let default_branch = info.head.clone().unwrap_or_else(|| "master".into());

    // identity (repo-local)
    cmds_files::set_git_identity(app.state(), "Tester".into(), "t@example.com".into(), Some(false)).unwrap();

    // untracked file shows in status
    std::fs::write(format!("{}/a.txt", DIR), "hello\n").unwrap();
    let st = cmds_files::get_status(app.state()).unwrap();
    assert!(st.iter().any(|f| f.path == "a.txt" && f.unstaged.as_deref() == Some("added")));

    // stage + initial commit
    cmds_files::stage_files(app.state(), vec!["a.txt".into()]).unwrap();
    let oid = cmds_files::create_commit(app.state(), "init".into(), None).unwrap();
    assert_eq!(oid.len(), 40);

    // history has 1 commit
    let h = cmds_history::get_history(app.state(), None, None, None, None).unwrap();
    assert_eq!(h.len(), 1);

    // modify → unstaged diff with hunks
    std::fs::write(format!("{}/a.txt", DIR), "hello\nworld\n").unwrap();
    let d = cmds_files::get_working_diff(app.state(), "a.txt".into(), false).unwrap();
    assert_eq!(d.additions, 1);
    assert!(!d.hunks.is_empty());

    // discard restores
    cmds_files::discard_changes(app.state(), vec!["a.txt".into()]).unwrap();
    assert_eq!(std::fs::read_to_string(format!("{}/a.txt", DIR)).unwrap(), "hello\n");

    // branch create + checkout
    cmds_branches::create_branch(app.state(), "feat".into(), None, Some(true)).unwrap();
    let branches = cmds_branches::list_branches(app.state()).unwrap();
    assert!(branches.iter().any(|b| b.name == "feat" && b.is_head));

    // commit on feat
    std::fs::write(format!("{}/b.txt", DIR), "feature\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "feat change".into(), None).unwrap();

    // checkout default branch, merge feat (fast-forward)
    cmds_branches::checkout_branch(app.state(), default_branch.clone()).unwrap();
    let res = cmds_branches::merge_branch(app.state(), "feat".into()).unwrap();
    assert_eq!(res.status, "fast_forward");

    // commit detail + file diff + blame
    let history = cmds_history::get_history(app.state(), None, None, None, None).unwrap();
    let head = &history[0];
    let det = cmds_history::get_commit_detail(app.state(), head.oid.clone()).unwrap();
    assert!(!det.files.is_empty());
    let fd = cmds_history::get_commit_file_diff(app.state(), head.oid.clone(), det.files[0].path.clone()).unwrap();
    assert_eq!(fd.path, det.files[0].path);
    let blame = cmds_files::get_blame(app.state(), "a.txt".into()).unwrap();
    assert_eq!(blame.len(), 1);

    // stash flow
    std::fs::write(format!("{}/dirty.txt", DIR), "wip\n").unwrap();
    cmds_stash_tags::stash_save(app.state(), "wip".into(), Some(true)).unwrap();
    let stashes = cmds_stash_tags::list_stashes(app.state()).unwrap();
    assert_eq!(stashes.len(), 1);
    let clean = cmds_files::get_status(app.state()).unwrap();
    assert!(clean.is_empty());
    cmds_stash_tags::stash_apply(app.state(), 0).unwrap();
    let st2 = cmds_files::get_status(app.state()).unwrap();
    assert!(st2.iter().any(|f| f.path == "dirty.txt"));
    cmds_stash_tags::stash_drop(app.state(), 0).unwrap();
    cmds_files::discard_changes(app.state(), vec!["dirty.txt".into()]).unwrap();

    // tag
    cmds_stash_tags::create_tag(app.state(), "v1.0".into(), None, Some("release".into())).unwrap();
    let tags = cmds_stash_tags::list_tags(app.state()).unwrap();
    assert_eq!(tags.len(), 1);
    assert!(tags[0].annotated);
    cmds_stash_tags::delete_tag(app.state(), "v1.0".into()).unwrap();
    assert!(cmds_stash_tags::list_tags(app.state()).unwrap().is_empty());

    // revert HEAD commit
    let head_oid = cmds_history::get_history(app.state(), None, None, None, None).unwrap()[0].oid.clone();
    let r = cmds_history::revert_commit(app.state(), head_oid).unwrap();
    assert!(["merged", "conflicts"].contains(&r.status.as_str()));

    // reset soft then commit history still fine
    let h2 = cmds_history::get_history(app.state(), None, None, None, None).unwrap();
    cmds_history::reset_to_commit(app.state(), h2[1].oid.clone(), "soft".into()).unwrap();

    // rename + delete branch
    cmds_branches::rename_branch(app.state(), "feat".into(), "feat-renamed".into()).unwrap();
    cmds_branches::delete_branch(app.state(), "feat-renamed".into()).unwrap();

    // ignore file
    cmds_files::ignore_file(app.state(), "*.log".into()).unwrap();
    let gi = std::fs::read_to_string(format!("{}/.gitignore", DIR)).unwrap();
    assert!(gi.contains("*.log"));

    // repo info + close
    let info2 = cmds_repo::repository_info(app.state()).unwrap();
    assert_eq!(info2.state, "clean");
    cmds_repo::close_repository(app.state()).unwrap();
    assert!(cmds_files::get_status(app.state()).is_err());
}

#[test]
fn stash_files_and_diff() {
    use git_crackit::cmds_stash_tags;
    let dir = "/tmp/git-crackit-stash-e2e";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();

    let app = tauri::test::mock_app();
    app.manage(AppState::default());
    let handle = app.handle().clone();

    cmds_repo::init_repository(handle.clone(), app.state(), dir.into(), Some(false)).unwrap();
    cmds_files::set_git_identity(app.state(), "T".into(), "t@e.com".into(), Some(false)).unwrap();
    std::fs::write(format!("{}/a.txt", dir), "one\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "init".into(), None).unwrap();
    std::fs::write(format!("{}/a.txt", dir), "one\ntwo\n").unwrap();
    std::fs::write(format!("{}/u.txt", dir), "untracked\n").unwrap();
    cmds_stash_tags::stash_save(app.state(), "wip".into(), Some(true)).unwrap();

    let files = cmds_stash_tags::stash_files(app.state(), 0).unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"a.txt"), "tracked file missing: {:?}", paths);
    assert!(paths.contains(&"u.txt"), "untracked file missing: {:?}", paths);

    let d = cmds_stash_tags::stash_file_diff(app.state(), 0, "a.txt".into()).unwrap();
    assert_eq!(d.additions, 1);
    let du = cmds_stash_tags::stash_file_diff(app.state(), 0, "u.txt".into()).unwrap();
    assert_eq!(du.status, "added");
}

fn git_cli(dir: &str, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn rebase_action_preserves_merge_commits() {
    let dir = "/tmp/git-crackit-rebase-e2e";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();

    // Fixture via git CLI: base → m-work + side-work → merge → p1 → p2.
    git_cli(dir, &["init", "-b", "main"]);
    git_cli(dir, &["config", "user.email", "t@e.com"]);
    git_cli(dir, &["config", "user.name", "T"]);
    git_cli(dir, &["config", "commit.gpgsign", "false"]);
    std::fs::write(format!("{}/a.txt", dir), "1\n").unwrap();
    git_cli(dir, &["add", "-A"]);
    git_cli(dir, &["commit", "-qm", "base"]);
    git_cli(dir, &["branch", "side"]);
    std::fs::write(format!("{}/m.txt", dir), "m\n").unwrap();
    git_cli(dir, &["add", "-A"]);
    git_cli(dir, &["commit", "-qm", "main work"]);
    let m_work = git_cli(dir, &["rev-parse", "HEAD"]);
    git_cli(dir, &["checkout", "-q", "side"]);
    std::fs::write(format!("{}/s.txt", dir), "s\n").unwrap();
    git_cli(dir, &["add", "-A"]);
    git_cli(dir, &["commit", "-qm", "side work"]);
    git_cli(dir, &["checkout", "-q", "main"]);
    git_cli(dir, &["merge", "-q", "--no-ff", "side", "-m", "merge side"]);
    std::fs::write(format!("{}/p1.txt", dir), "1\n").unwrap();
    git_cli(dir, &["add", "-A"]);
    git_cli(dir, &["commit", "-qm", "p1"]);
    std::fs::write(format!("{}/p2.txt", dir), "2\n").unwrap();
    git_cli(dir, &["add", "-A"]);
    git_cli(dir, &["commit", "-qm", "p2"]);
    let p2 = git_cli(dir, &["rev-parse", "HEAD"]);

    let app = tauri::test::mock_app();
    app.manage(AppState::default());
    let handle = app.handle().clone();
    cmds_repo::open_repository(handle.clone(), app.state(), dir.into()).unwrap();

    // Move "main work" (older than the merge) after p2 — merge is inside range.
    let res = cmds_branches::rebase_commit_action(
        app.state(),
        m_work.clone(),
        p2.clone(),
        "move_after".into(),
    )
    .unwrap();
    assert_eq!(res.status, "merged", "rebase failed: {:?}", res.conflicts);

    let h = cmds_history::get_history(app.state(), None, None, None, None).unwrap();
    assert_eq!(h[0].summary, "main work", "order: {:?}", h.iter().map(|c| &c.summary).collect::<Vec<_>>());
    assert_eq!(h[1].summary, "p2");
    assert_eq!(h[2].summary, "p1");

    // The merge commit must still exist in the rewritten history.
    let repo = git2::Repository::open(dir).unwrap();
    let mut walk = repo.revwalk().unwrap();
    walk.push_head().unwrap();
    let has_merge = walk
        .flatten()
        .any(|oid| repo.find_commit(oid).unwrap().parent_count() == 2);
    assert!(has_merge, "merge commit was lost by the rebase");
}

#[test]
fn ssh_remote_dispatches_to_git_cli() {
    use git_crackit::cmds_remotes;
    let dir = "/tmp/git-crackit-ssh-e2e";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();

    let app = tauri::test::mock_app();
    app.manage(AppState::default());
    let handle = app.handle().clone();

    cmds_repo::init_repository(handle.clone(), app.state(), dir.into(), Some(false)).unwrap();
    cmds_remotes::add_remote(app.state(), "origin".into(), "git@localhost:nonexistent/repo.git".into()).unwrap();

    let err = cmds_remotes::fetch_remote(handle.clone(), app.state(), Some("origin".into())).unwrap_err();
    // Must reach the system git CLI (ssh connect/auth error), not git2's
    // "unsupported URL protocol" error.
    assert!(!err.contains("unsupported"), "hit git2 path: {}", err);
    assert!(
        err.contains("SSH") || err.contains("ssh") || err.contains("onnect"),
        "unexpected error: {}", err
    );
}
