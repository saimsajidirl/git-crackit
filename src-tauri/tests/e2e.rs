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
