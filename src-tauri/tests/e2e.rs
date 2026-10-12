use git_crackit::{cmds_branches, cmds_files, cmds_history, cmds_remotes, cmds_repo, cmds_stash_tags, state::AppState};
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
    cmds_branches::create_branch(app.state(), "feat".into(), None, Some(true), None).unwrap();
    let branches = cmds_branches::list_branches(app.state()).unwrap();
    assert!(branches.iter().any(|b| b.name == "feat" && b.is_head));

    // commit on feat
    std::fs::write(format!("{}/b.txt", DIR), "feature\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "feat change".into(), None).unwrap();

    // checkout default branch, merge feat (fast-forward)
    cmds_branches::checkout_branch(app.state(), default_branch.clone(), None).unwrap();
    let res = cmds_branches::merge_branch(app.state(), "feat".into(), None).unwrap();
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

#[test]
fn checkout_dirty_prompt_carry_stash() {
    let dir = "/tmp/git-crackit-dirty-e2e";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();

    let app = tauri::test::mock_app();
    app.manage(AppState::default());
    let handle = app.handle().clone();

    let info = cmds_repo::init_repository(handle.clone(), app.state(), dir.into(), Some(false)).unwrap();
    let main = info.head.clone().unwrap_or_else(|| "master".into());
    cmds_files::set_git_identity(app.state(), "T".into(), "t@e.com".into(), Some(false)).unwrap();
    std::fs::write(format!("{}/a.txt", dir), "one\n").unwrap();
    std::fs::write(format!("{}/shared.txt", dir), "same\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "init".into(), None).unwrap();

    // "other" diverges on a.txt so a dirty edit to it can't be checked out safely.
    cmds_branches::create_branch(app.state(), "other".into(), None, Some(true), None).unwrap();
    std::fs::write(format!("{}/a.txt", dir), "two\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "other change".into(), None).unwrap();
    let r = cmds_branches::checkout_branch(app.state(), main.clone(), Some("carry".into())).unwrap();
    assert_eq!(r.status, "ok");
    assert_eq!(std::fs::read_to_string(format!("{}/a.txt", dir)).unwrap(), "one\n");

    // Dirty edit → "ask" reports dirty and does not switch.
    std::fs::write(format!("{}/a.txt", dir), "dirty\n").unwrap();
    let r = cmds_branches::checkout_branch(app.state(), "other".into(), None).unwrap();
    assert_eq!(r.status, "dirty");
    assert_eq!(r.dirty_files, 1);
    assert_eq!(
        cmds_repo::repository_info(app.state()).unwrap().head.as_deref(),
        Some(main.as_str())
    );

    // "stash" → changes stay on main, clean tree on other.
    let r = cmds_branches::checkout_branch(app.state(), "other".into(), Some("stash".into())).unwrap();
    assert_eq!(r.status, "ok");
    assert_eq!(
        cmds_repo::repository_info(app.state()).unwrap().head.as_deref(),
        Some("other")
    );
    assert_eq!(std::fs::read_to_string(format!("{}/a.txt", dir)).unwrap(), "two\n");
    assert!(cmds_files::get_status(app.state()).unwrap().is_empty());
    let stashes = cmds_stash_tags::list_stashes(app.state()).unwrap();
    assert_eq!(stashes.len(), 1);
    assert!(stashes[0].message.starts_with(&format!("On {}: WIP", main)));

    // "carry" a change to a file that's identical on both branches — the safe
    // checkout fast path just brings it along.
    std::fs::write(format!("{}/shared.txt", dir), "sh-dirty\n").unwrap();
    let r = cmds_branches::checkout_branch(app.state(), main.clone(), Some("carry".into())).unwrap();
    assert_eq!(r.status, "ok");
    assert_eq!(
        cmds_repo::repository_info(app.state()).unwrap().head.as_deref(),
        Some(main.as_str())
    );
    assert_eq!(std::fs::read_to_string(format!("{}/a.txt", dir)).unwrap(), "one\n");
    assert_eq!(std::fs::read_to_string(format!("{}/shared.txt", dir)).unwrap(), "sh-dirty\n");
    cmds_files::discard_changes(app.state(), vec!["shared.txt".into()]).unwrap();

    // "carry" a change to a file that diverges — stash → checkout → apply
    // merges it and reports the conflict, keeping the stash as backup.
    std::fs::write(format!("{}/a.txt", dir), "dirty2\n").unwrap();
    let r = cmds_branches::checkout_branch(app.state(), "other".into(), Some("carry".into())).unwrap();
    assert_eq!(r.status, "conflicts");
    assert!(r.conflicts.iter().any(|c| c.contains("a.txt")));
    assert_eq!(
        cmds_repo::repository_info(app.state()).unwrap().head.as_deref(),
        Some("other")
    );
    let content = std::fs::read_to_string(format!("{}/a.txt", dir)).unwrap();
    assert!(content.contains("<<<<<<<"), "expected conflict markers: {}", content);
    // Carry stash kept (index 0) as backup on top of the earlier one.
    let stashes = cmds_stash_tags::list_stashes(app.state()).unwrap();
    assert_eq!(stashes.len(), 2);
    assert!(stashes[0].message.starts_with(&format!("On {}: WIP", main)));

    // Clear the conflict state (reset --merge reverts conflicted files).
    cmds_branches::abort_merge(app.state()).unwrap();

    // Detached commit checkout routes through the same dirty machinery.
    std::fs::write(format!("{}/shared.txt", dir), "det-dirty\n").unwrap();
    let oid = cmds_history::get_history(app.state(), None, None, None, None).unwrap()[0].oid.clone();
    let r = cmds_history::checkout_commit(app.state(), oid, None).unwrap();
    assert_eq!(r.status, "dirty");
    assert!(cmds_repo::repository_info(app.state()).unwrap().head.as_deref() == Some("other"));

    // create_branch + checkout on a different tip asks first — and leaves no
    // stray branch behind when it does.
    let r = cmds_branches::create_branch(app.state(), "dirtyb".into(), Some(main.clone()), Some(true), None).unwrap();
    assert_eq!(r.status, "dirty");
    assert!(!cmds_branches::list_branches(app.state()).unwrap().iter().any(|b| b.name == "dirtyb"));
    // Same-tip branch doesn't prompt: create at HEAD carries the dirty work.
    let r = cmds_branches::create_branch(app.state(), "dirtyb".into(), None, Some(true), None).unwrap();
    assert_eq!(r.status, "ok");
    assert_eq!(cmds_repo::repository_info(app.state()).unwrap().head.as_deref(), Some("dirtyb"));
    assert_eq!(std::fs::read_to_string(format!("{}/shared.txt", dir)).unwrap(), "det-dirty\n");
}

#[test]
fn merge_dirty_guard() {
    let dir = "/tmp/git-crackit-ffdirty-e2e";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();

    let app = tauri::test::mock_app();
    app.manage(AppState::default());
    let handle = app.handle().clone();

    let info = cmds_repo::init_repository(handle.clone(), app.state(), dir.into(), Some(false)).unwrap();
    let main = info.head.clone().unwrap_or_else(|| "master".into());
    cmds_files::set_git_identity(app.state(), "T".into(), "t@e.com".into(), Some(false)).unwrap();
    std::fs::write(format!("{}/a.txt", dir), "one\n").unwrap();
    std::fs::write(format!("{}/u.txt", dir), "u\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "init".into(), None).unwrap();
    let main_oid = cmds_history::get_history(app.state(), None, None, None, None).unwrap()[0].oid.clone();

    // feat changes a.txt + adds new.txt; feat2 adds b.txt on top.
    cmds_branches::create_branch(app.state(), "feat".into(), None, Some(true), None).unwrap();
    std::fs::write(format!("{}/a.txt", dir), "two\n").unwrap();
    std::fs::write(format!("{}/new.txt", dir), "new\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "feat change".into(), None).unwrap();
    cmds_branches::create_branch(app.state(), "feat2".into(), None, Some(true), None).unwrap();
    std::fs::write(format!("{}/b.txt", dir), "b\n").unwrap();
    cmds_files::stage_files(app.state(), vec!["b.txt".into()]).unwrap();
    cmds_files::create_commit(app.state(), "feat2 change".into(), None).unwrap();
    cmds_branches::checkout_branch(app.state(), main.clone(), None).unwrap();

    // "ask": the ff would touch dirty a.txt → "dirty" outcome, nothing moves.
    std::fs::write(format!("{}/a.txt", dir), "dirty\n").unwrap();
    let r = cmds_branches::merge_branch(app.state(), "feat".into(), None).unwrap();
    assert_eq!(r.status, "dirty");
    assert_eq!(r.dirty_files, 1);
    assert_eq!(std::fs::read_to_string(format!("{}/a.txt", dir)).unwrap(), "dirty\n");
    let main_ref = cmds_branches::list_branches(app.state()).unwrap()
        .into_iter()
        .find(|b| b.name == main)
        .and_then(|b| b.last_commit_oid);
    assert_eq!(main_ref.as_deref(), Some(main_oid.as_str()), "branch ref must not move on a blocked ff");
    assert!(!std::path::Path::new(&format!("{}/new.txt", dir)).exists());

    // "carry": stash → ff → apply — the dirty edit (base "one") can't merge
    // over "two" → conflicts, stash kept as backup.
    let r = cmds_branches::merge_branch(app.state(), "feat".into(), Some("carry".into())).unwrap();
    assert_eq!(r.status, "conflicts");
    let content = std::fs::read_to_string(format!("{}/a.txt", dir)).unwrap();
    assert!(content.contains("<<<<<<<"), "expected conflict markers: {}", content);
    assert_eq!(std::fs::read_to_string(format!("{}/new.txt", dir)).unwrap(), "new\n");
    cmds_branches::abort_merge(app.state()).unwrap(); // reset --merge → a.txt reverts
    assert_eq!(std::fs::read_to_string(format!("{}/a.txt", dir)).unwrap(), "two\n");
    assert_eq!(cmds_stash_tags::list_stashes(app.state()).unwrap().len(), 1);
    cmds_stash_tags::stash_drop(app.state(), 0).unwrap();

    // Dirty file untouched by the merge → no prompt, ff proceeds, edit survives.
    std::fs::write(format!("{}/u.txt", dir), "u-dirty\n").unwrap();
    let r = cmds_branches::merge_branch(app.state(), "feat2".into(), None).unwrap();
    assert_eq!(r.status, "fast_forward");
    assert_eq!(std::fs::read_to_string(format!("{}/b.txt", dir)).unwrap(), "b\n");
    assert_eq!(std::fs::read_to_string(format!("{}/u.txt", dir)).unwrap(), "u-dirty\n");

    // Diverge: feat3 adds e.txt, main adds c.txt → real merge.
    // An untracked file at a path the merge adds → "dirty" on ask; "stash"
    // mode keeps it stashed and merges.
    cmds_branches::create_branch(app.state(), "feat3".into(), None, Some(true), None).unwrap();
    std::fs::write(format!("{}/e.txt", dir), "e\n").unwrap();
    cmds_files::stage_files(app.state(), vec!["e.txt".into()]).unwrap();
    cmds_files::create_commit(app.state(), "feat3 change".into(), None).unwrap();
    cmds_branches::checkout_branch(app.state(), main.clone(), Some("carry".into())).unwrap();
    std::fs::write(format!("{}/c.txt", dir), "c\n").unwrap();
    cmds_files::stage_files(app.state(), vec!["c.txt".into()]).unwrap();
    cmds_files::create_commit(app.state(), "main work".into(), None).unwrap();

    std::fs::write(format!("{}/e.txt", dir), "dirty-e\n").unwrap();
    let r = cmds_branches::merge_branch(app.state(), "feat3".into(), None).unwrap();
    assert_eq!(r.status, "dirty");
    assert_eq!(std::fs::read_to_string(format!("{}/e.txt", dir)).unwrap(), "dirty-e\n");
    let r = cmds_branches::merge_branch(app.state(), "feat3".into(), Some("stash".into())).unwrap();
    assert_eq!(r.status, "merged");
    assert_eq!(std::fs::read_to_string(format!("{}/e.txt", dir)).unwrap(), "e\n");
    assert!(cmds_files::get_status(app.state()).unwrap().is_empty());
    assert_eq!(cmds_stash_tags::list_stashes(app.state()).unwrap().len(), 1);
}

#[test]
fn abort_merge_preserves_dirty() {
    let dir = "/tmp/git-crackit-abort-e2e";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();

    let app = tauri::test::mock_app();
    app.manage(AppState::default());
    let handle = app.handle().clone();

    let info = cmds_repo::init_repository(handle.clone(), app.state(), dir.into(), Some(false)).unwrap();
    let main = info.head.clone().unwrap_or_else(|| "master".into());
    cmds_files::set_git_identity(app.state(), "T".into(), "t@e.com".into(), Some(false)).unwrap();
    std::fs::write(format!("{}/a.txt", dir), "one\n").unwrap();
    std::fs::write(format!("{}/u.txt", dir), "u\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "init".into(), None).unwrap();

    cmds_branches::create_branch(app.state(), "side".into(), None, Some(true), None).unwrap();
    std::fs::write(format!("{}/a.txt", dir), "two\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "side".into(), None).unwrap();
    cmds_branches::checkout_branch(app.state(), main.clone(), None).unwrap();
    std::fs::write(format!("{}/a.txt", dir), "three\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "main".into(), None).unwrap();

    // Conflicting merge; during resolution the user makes unrelated edits.
    let res = cmds_branches::merge_branch(app.state(), "side".into(), None).unwrap();
    assert_eq!(res.status, "conflicts");
    std::fs::write(format!("{}/u.txt", dir), "u-dirty\n").unwrap();
    std::fs::write(format!("{}/new.txt", dir), "new\n").unwrap();

    cmds_branches::abort_merge(app.state()).unwrap();

    // Conflicted file reverted to HEAD; unrelated dirty work + untracked kept.
    assert_eq!(std::fs::read_to_string(format!("{}/a.txt", dir)).unwrap(), "three\n");
    assert_eq!(std::fs::read_to_string(format!("{}/u.txt", dir)).unwrap(), "u-dirty\n");
    assert!(std::path::Path::new(&format!("{}/new.txt", dir)).exists());
    let st = cmds_files::get_status(app.state()).unwrap();
    assert!(st.iter().all(|f| !f.conflicted), "conflicts left after abort: {:?}", st.iter().map(|f| &f.path).collect::<Vec<_>>());
    assert_eq!(cmds_repo::repository_info(app.state()).unwrap().state, "clean");
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

#[test]
fn ai_generate_requires_staged_changes() {
    let dir = "/tmp/git-crackit-ai-e2e";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();

    let app = tauri::test::mock_app();
    app.manage(AppState::default());
    let handle = app.handle().clone();

    cmds_repo::init_repository(handle.clone(), app.state(), dir.into(), Some(false)).unwrap();
    std::fs::write(format!("{}/f.txt", dir), "hello").unwrap();

    // Nothing staged → clear guard error before any provider call.
    let err = git_crackit::ai::generate_commit_message(app.state()).unwrap_err();
    assert!(err.contains("No staged changes"), "unexpected: {}", err);
}

#[test]
#[ignore = "live network — run manually: cargo test --test e2e ai_live -- --ignored"]
fn ai_live_generates_message() {
    let dir = "/tmp/git-crackit-ai-live";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let app = tauri::test::mock_app();
    app.manage(AppState::default());
    let handle = app.handle().clone();
    cmds_repo::init_repository(handle.clone(), app.state(), dir.into(), Some(false)).unwrap();
    std::fs::write(format!("{}/f.txt", dir), "hello world\n").unwrap();
    cmds_files::stage_files(app.state(), vec!["f.txt".into()]).unwrap();
    match git_crackit::ai::generate_commit_message(app.state()) {
        Ok(m) => println!("generated: {}", m),
        Err(e) => println!("provider error (expected without keys/ollama): {}", e),
    }
}

#[test]
fn undo_redo_hunks_and_forcepush() {
    let dir = "/tmp/git-crackit-undo-e2e";
    let remote_dir = "/tmp/git-crackit-undo-remote";
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(remote_dir);
    std::fs::create_dir_all(dir).unwrap();
    std::fs::create_dir_all(remote_dir).unwrap();

    let app = tauri::test::mock_app();
    app.manage(AppState::default());
    let handle = app.handle().clone();

    let info = cmds_repo::init_repository(handle.clone(), app.state(), dir.into(), Some(false)).unwrap();
    let main = info.head.clone().unwrap_or_else(|| "master".into());
    cmds_files::set_git_identity(app.state(), "T".into(), "t@e.com".into(), Some(false)).unwrap();
    let lines: String = (0..40).map(|i| format!("line{:02}\n", i)).collect();
    std::fs::write(format!("{}/a.txt", dir), &lines).unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "init".into(), None).unwrap();
    let tip1 = cmds_history::get_history(app.state(), None, None, None, None).unwrap()[0].oid.clone();

    /* ---------- undo/redo commit (soft: ref moves back, work stays staged) ---------- */
    std::fs::write(format!("{}/b.txt", dir), "b\n").unwrap();
    cmds_files::stage_files(app.state(), vec!["b.txt".into()]).unwrap();
    cmds_files::create_commit(app.state(), "second".into(), None).unwrap();
    let tip2 = cmds_history::get_history(app.state(), None, None, None, None).unwrap()[0].oid.clone();
    assert_ne!(tip1, tip2);

    let r = cmds_history::undo_last(app.state()).unwrap();
    assert!(r.contains("Commit"), "unexpected undo label: {}", r);
    assert_eq!(
        cmds_history::get_history(app.state(), None, None, None, None).unwrap()[0].oid,
        tip1
    );
    // Soft undo: b.txt still staged, not lost.
    let st = cmds_files::get_status(app.state()).unwrap();
    assert!(st.iter().any(|f| f.path == "b.txt" && f.staged.is_some()));

    let r = cmds_history::redo_last(app.state()).unwrap();
    assert!(r.contains("Commit"));
    assert_eq!(
        cmds_history::get_history(app.state(), None, None, None, None).unwrap()[0].oid,
        tip2
    );
    assert!(cmds_files::get_status(app.state()).unwrap().is_empty());

    /* ---------- undo/redo checkout ---------- */
    cmds_branches::create_branch(app.state(), "feat".into(), None, Some(true), None).unwrap();
    assert_eq!(
        cmds_repo::repository_info(app.state()).unwrap().head.as_deref(),
        Some("feat")
    );
    cmds_history::undo_last(app.state()).unwrap();
    assert_eq!(
        cmds_repo::repository_info(app.state()).unwrap().head.as_deref(),
        Some(main.as_str())
    );
    cmds_history::redo_last(app.state()).unwrap();
    assert_eq!(
        cmds_repo::repository_info(app.state()).unwrap().head.as_deref(),
        Some("feat")
    );
    cmds_branches::checkout_branch(app.state(), main.clone(), None).unwrap();

    /* ---------- hunk staging ---------- */
    let mut ls: Vec<String> = lines.lines().map(|l| l.to_string()).collect();
    ls[0] = "line00-EDITED".into();
    ls[37] = "line37-EDITED".into();
    std::fs::write(format!("{}/a.txt", dir), ls.join("\n") + "\n").unwrap();
    let d = cmds_files::get_working_diff(app.state(), "a.txt".into(), false).unwrap();
    assert_eq!(d.hunks.len(), 2, "expected two hunks, got {}", d.hunks.len());

    cmds_files::stage_hunk(app.state(), "a.txt".into(), 0).unwrap();
    let unstaged = cmds_files::get_working_diff(app.state(), "a.txt".into(), false).unwrap();
    assert_eq!(unstaged.hunks.len(), 1);
    let staged = cmds_files::get_working_diff(app.state(), "a.txt".into(), true).unwrap();
    assert_eq!(staged.hunks.len(), 1);
    assert!(staged.hunks[0].lines.iter().any(|l| l.content.contains("line00-EDITED")));

    cmds_files::unstage_hunk(app.state(), "a.txt".into(), 0).unwrap();
    assert_eq!(
        cmds_files::get_working_diff(app.state(), "a.txt".into(), false).unwrap().hunks.len(),
        2
    );

    cmds_files::discard_hunk(app.state(), "a.txt".into(), 0).unwrap();
    let d = cmds_files::get_working_diff(app.state(), "a.txt".into(), false).unwrap();
    assert_eq!(d.hunks.len(), 1);
    assert!(!d.hunks[0].lines.iter().any(|l| l.content.contains("line00-EDITED")));
    let content = std::fs::read_to_string(format!("{}/a.txt", dir)).unwrap();
    assert!(content.contains("line00\n") && content.contains("line37-EDITED"));

    /* ---------- force-push with lease (local-path remote → git2 path) ---------- */
    git2::Repository::init_bare(remote_dir).unwrap();
    cmds_remotes::add_remote(app.state(), "origin".into(), remote_dir.into()).unwrap();
    cmds_files::discard_changes(app.state(), vec!["a.txt".into()]).unwrap();
    cmds_remotes::push(app.state(), Some(true), None).unwrap();

    // Amend → local diverged from remote → normal push rejects.
    cmds_files::create_commit(app.state(), "amended".into(), Some(true)).unwrap();
    let err = cmds_remotes::push(app.state(), Some(true), None).err().unwrap();
    assert!(err.contains("non-fast-forward"), "expected non-ff rejection: {}", err);
    cmds_remotes::push(app.state(), Some(true), Some(true)).unwrap();

    // Someone else pushes (simulated by committing to the bare remote
    // directly) → lease must refuse.
    let bare = git2::Repository::open(remote_dir).unwrap();
    let tip = bare
        .find_reference(&format!("refs/heads/{}", main))
        .unwrap()
        .peel_to_commit()
        .unwrap();
    let sig = git2::Signature::now("T", "t@e.com").unwrap();
    bare.commit(
        Some(&format!("refs/heads/{}", main)),
        &sig,
        &sig,
        "remote moved",
        &tip.tree().unwrap(),
        &[&tip],
    )
    .unwrap();
    let err = cmds_remotes::push(app.state(), Some(true), Some(true)).err().unwrap();
    assert!(err.contains("moved since your last fetch"), "expected lease failure: {}", err);
}

#[test]
fn file_history_compare_and_signing() {
    let dir = "/tmp/git-crackit-fh-e2e";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();

    let app = tauri::test::mock_app();
    app.manage(AppState::default());
    let handle = app.handle().clone();

    cmds_repo::init_repository(handle.clone(), app.state(), dir.into(), Some(false)).unwrap();
    cmds_files::set_git_identity(app.state(), "T".into(), "t@e.com".into(), Some(false)).unwrap();

    std::fs::write(format!("{}/a.txt", dir), "one\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "add a".into(), None).unwrap();
    std::fs::write(format!("{}/other.txt", dir), "x\n").unwrap();
    std::fs::write(format!("{}/a.txt", dir), "two\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "edit a + add other".into(), None).unwrap();

    /* ---------- file history ---------- */
    let fh = cmds_history::file_history(app.state(), "a.txt".into()).unwrap();
    assert_eq!(fh.len(), 2, "a.txt history: {:?}", fh.iter().map(|c| &c.summary).collect::<Vec<_>>());
    let fh_other = cmds_history::file_history(app.state(), "other.txt".into()).unwrap();
    assert_eq!(fh_other.len(), 1);

    // Rename follow: git mv a.txt b.txt → history of b.txt keeps a.txt commits.
    std::fs::rename(format!("{}/a.txt", dir), format!("{}/b.txt", dir)).unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "rename a→b".into(), None).unwrap();
    let fh_b = cmds_history::file_history(app.state(), "b.txt".into()).unwrap();
    assert!(fh_b.len() >= 3, "rename-followed history: {:?}", fh_b.iter().map(|c| &c.summary).collect::<Vec<_>>());

    /* ---------- compare refs ---------- */
    cmds_branches::create_branch(app.state(), "cmp-base".into(), None, Some(false), None).unwrap();
    std::fs::write(format!("{}/b.txt", dir), "three\n").unwrap();
    std::fs::write(format!("{}/c.txt", dir), "new\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "on main".into(), None).unwrap();

    let r = cmds_history::compare_refs(app.state(), "cmp-base".into(), "main".into()).unwrap();
    assert_eq!(r.ahead, 1);
    assert_eq!(r.behind, 0);
    let paths: Vec<&str> = r.files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"b.txt") && paths.contains(&"c.txt"), "compare files: {:?}", paths);

    let fd = cmds_history::compare_file_diff(app.state(), "cmp-base".into(), "main".into(), "b.txt".into()).unwrap();
    assert!(fd.hunks.iter().flat_map(|h| h.lines.iter()).any(|l| l.content.contains("three")));
}

#[test]
fn signed_commit_gpg() {
    let dir = "/tmp/git-crackit-sign-e2e";
    let gnupg = "/tmp/git-crackit-sign-gnupg";
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(gnupg);
    std::fs::create_dir_all(dir).unwrap();
    std::fs::create_dir_all(gnupg).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(gnupg, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    let app = tauri::test::mock_app();
    app.manage(AppState::default());
    let handle = app.handle().clone();
    cmds_repo::init_repository(handle.clone(), app.state(), dir.into(), Some(false)).unwrap();
    cmds_files::set_git_identity(app.state(), "T".into(), "t@e.com".into(), Some(false)).unwrap();

    // Throwaway signing key in an isolated GNUPGHOME.
    std::env::set_var("GNUPGHOME", gnupg);
    let keygen = std::process::Command::new("gpg")
        .args([
            "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
            "--quick-generate-key", "T <t@e.com>", "ed25519", "sign", "never",
        ])
        .output()
        .expect("gpg missing");
    assert!(keygen.status.success(), "{}", String::from_utf8_lossy(&keygen.stderr));
    let fpr = {
        let out = std::process::Command::new("gpg")
            .args(["--batch", "--with-colons", "--list-keys"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .find(|l| l.starts_with("fpr:"))
            .and_then(|l| l.split(':').nth(9))
            .unwrap()
            .to_string()
    };

    let repo = git2::Repository::open(dir).unwrap();
    let mut cfg = repo.config().unwrap();
    cfg.set_bool("commit.gpgsign", true).unwrap();
    cfg.set_str("user.signingkey", &fpr).unwrap();
    drop(cfg);
    drop(repo);

    std::fs::write(format!("{}/s.txt", dir), "signed\n").unwrap();
    cmds_files::stage_all(app.state()).unwrap();
    cmds_files::create_commit(app.state(), "signed commit".into(), None).unwrap();

    let hist = cmds_history::get_history(app.state(), None, None, None, None).unwrap();
    assert!(hist[0].signed, "commit should carry a gpgsig header");
    // Verify the signature is real: extract + gpg --verify round-trip.
    let repo = git2::Repository::open(dir).unwrap();
    let oid = git2::Oid::from_str(&hist[0].oid).unwrap();
    let (sig, payload) = repo.extract_signature(&oid, Some("gpgsig")).unwrap();
    let tmp = format!("{}/sig.asc", dir);
    let tmpd = format!("{}/data.txt", dir);
    std::fs::write(&tmp, &*sig).unwrap();
    std::fs::write(&tmpd, &*payload).unwrap();
    let verify = std::process::Command::new("gpg")
        .args(["--batch", "--verify", &tmp, &tmpd])
        .output()
        .unwrap();
    assert!(verify.status.success(), "{}", String::from_utf8_lossy(&verify.stderr));
}
