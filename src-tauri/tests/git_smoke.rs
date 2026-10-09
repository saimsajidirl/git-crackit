use git2::{DiffOptions, Repository, Signature, StatusOptions};
use std::fs;
use std::path::Path;

fn fresh_repo(dir: &Path) -> Repository {
    let _ = fs::remove_dir_all(dir);
    fs::create_dir_all(dir).unwrap();
    Repository::init(dir).unwrap()
}

fn commit_all(repo: &Repository, msg: &str) -> git2::Oid {
    let sig = Signature::now("Tester", "t@example.com").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None).unwrap();
    idx.write().unwrap();
    let tree = repo.find_tree(idx.write_tree().unwrap()).unwrap();
    let parents: Vec<git2::Commit> = repo
        .head()
        .and_then(|h| h.peel_to_commit())
        .map(|c| vec![c])
        .unwrap_or_default();
    let prefs: Vec<&git2::Commit> = parents.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &prefs).unwrap()
}

#[test]
fn status_diff_merge_flow() {
    let dir = Path::new("/tmp/git-crackit-test");
    let mut repo = fresh_repo(dir);

    // initial commit
    fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
    commit_all(&repo, "init");

    // modify + new file
    fs::write(dir.join("a.txt"), "line1\nCHANGED\nline3\n").unwrap();
    fs::write(dir.join("b.txt"), "new\n").unwrap();

    // status
    {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true).recurse_untracked_dirs(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();
        let paths: Vec<String> = statuses
            .iter()
            .filter_map(|e| e.path().map(|s| s.to_string()))
            .collect();
        assert!(paths.contains(&"a.txt".to_string()));
        assert!(paths.contains(&"b.txt".to_string()));
    }

    // workdir diff patch hunks
    {
        let index = repo.index().unwrap();
        let mut dopts = DiffOptions::new();
        dopts.pathspec("a.txt").include_untracked(true);
        let mut diff = repo.diff_index_to_workdir(Some(&index), Some(&mut dopts)).unwrap();
        let patch = git2::Patch::from_diff(&mut diff, 0).unwrap().unwrap();
        assert!(patch.num_hunks() >= 1);
        let (_hunk, nlines) = patch.hunk(0).unwrap();
        assert!(nlines > 0);
        let line = patch.line_in_hunk(0, 0).unwrap();
        assert!(!line.content().is_empty());
    }

    // branch + checkout flow
    {
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature", &head, false).unwrap();
        repo.set_head("refs/heads/feature").unwrap();
        let mut cb = git2::build::CheckoutBuilder::new();
        cb.safe();
        repo.checkout_head(Some(&mut cb)).unwrap();
    }

    // commit on feature
    fs::write(dir.join("f.txt"), "feature\n").unwrap();
    commit_all(&repo, "feat");

    // merge analysis
    {
        let fb = repo.find_branch("feature", git2::BranchType::Local).unwrap();
        let ann = repo.reference_to_annotated_commit(fb.get()).unwrap();
        let (analysis, _) = repo.merge_analysis(&[&ann]).unwrap();
        assert!(analysis.is_fast_forward() || analysis.is_normal() || analysis.is_up_to_date());
    }

    // revwalk all refs
    {
        let mut walk = repo.revwalk().unwrap();
        for r in repo.references().unwrap().flatten() {
            if let Some(oid) = r.target() {
                let _ = walk.push(oid);
            }
        }
        let count = walk.flatten().count();
        assert!(count >= 2);
    }

    // stash
    fs::write(dir.join("a.txt"), "dirty\n").unwrap();
    {
        let sig = Signature::now("Tester", "t@example.com").unwrap();
        repo.stash_save(&sig, "wip", Some(git2::StashFlags::INCLUDE_UNTRACKED)).unwrap();
    }
    let mut n = 0;
    repo.stash_foreach(|_, _, _| {
        n += 1;
        true
    })
    .unwrap();
    assert_eq!(n, 1);
}
