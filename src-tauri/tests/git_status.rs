use std::fs;
use std::process::Command;

use simplegit_lib::git::{open_repository, parse_status};
use tempfile::TempDir;

fn run_git(repo: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git should be available");

    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_repository() -> TempDir {
    let directory = tempfile::tempdir().expect("temporary directory");
    run_git(directory.path(), &["init", "-b", "main"]);
    run_git(directory.path(), &["config", "user.name", "SimpleGit Test"]);
    run_git(
        directory.path(),
        &["config", "user.email", "simplegit@example.test"],
    );
    directory
}

#[test]
fn parses_branch_files_and_conflicts() {
    let input = b"# branch.oid 0123456789abcdef\0\
# branch.head main\0\
# branch.upstream origin/main\0\
# branch.ab +2 -3\0\
1 M. N... 100644 100644 100644 aaa bbb src/main.ts\0\
1 .M N... 100644 100644 100644 aaa bbb docs/with space.md\0\
? new file.txt\0\
u UU N... 100644 100644 100644 100644 aaa bbb ccc merge.txt\0";

    let snapshot = parse_status(input);

    assert_eq!(snapshot.repository.root, "");
    assert_eq!(snapshot.repository.name, "");
    assert_eq!(snapshot.repository.branch, "main");
    assert_eq!(snapshot.repository.upstream.as_deref(), Some("origin/main"));
    assert_eq!(snapshot.repository.ahead, 2);
    assert_eq!(snapshot.repository.behind, 3);
    assert!(snapshot.repository.has_conflicts);
    assert_eq!(snapshot.changes.len(), 4);

    let staged = &snapshot.changes[0];
    assert_eq!(staged.path, "src/main.ts");
    assert!(staged.staged);
    assert!(!staged.unstaged);
    assert_eq!(staged.kind.as_str(), "modified");

    let unstaged = &snapshot.changes[1];
    assert_eq!(unstaged.path, "docs/with space.md");
    assert!(!unstaged.staged);
    assert!(unstaged.unstaged);

    let untracked = &snapshot.changes[2];
    assert_eq!(untracked.path, "new file.txt");
    assert_eq!(untracked.kind.as_str(), "untracked");
    assert!(!untracked.staged);
    assert!(untracked.unstaged);

    let conflicted = &snapshot.changes[3];
    assert_eq!(conflicted.path, "merge.txt");
    assert!(conflicted.conflicted);
    assert!(!conflicted.staged);
    assert!(!conflicted.unstaged);
}

#[test]
fn parses_renames_with_original_paths() {
    let input = b"# branch.head feature/status\0\
2 R. N... 100644 100644 100644 aaa bbb R100 new name.txt\0old name.txt\0";

    let snapshot = parse_status(input);

    assert_eq!(snapshot.changes.len(), 1);
    assert_eq!(snapshot.changes[0].path, "new name.txt");
    assert_eq!(
        snapshot.changes[0].original_path.as_deref(),
        Some("old name.txt")
    );
    assert_eq!(snapshot.changes[0].kind.as_str(), "renamed");
    assert!(snapshot.changes[0].staged);
}

#[test]
fn opens_an_existing_repository_and_reports_changes() {
    let directory = init_repository();
    fs::write(directory.path().join("tracked.txt"), "before\n").expect("write tracked file");
    run_git(directory.path(), &["add", "tracked.txt"]);
    run_git(directory.path(), &["commit", "-m", "chore: initial"]);

    fs::write(directory.path().join("tracked.txt"), "after\n").expect("modify tracked file");
    fs::write(directory.path().join("new file.txt"), "new\n").expect("write untracked file");

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let snapshot = runtime
        .block_on(open_repository(directory.path()))
        .expect("repository should open");

    assert_eq!(
        std::fs::canonicalize(&snapshot.repository.root).unwrap(),
        directory.path().canonicalize().unwrap()
    );
    #[cfg(windows)]
    assert!(!snapshot.repository.root.starts_with(r"\\?\"));
    assert_eq!(snapshot.repository.branch, "main");
    assert_eq!(snapshot.changes.len(), 2);
    assert!(snapshot
        .changes
        .iter()
        .any(|change| change.path == "tracked.txt" && change.unstaged));
    assert!(snapshot
        .changes
        .iter()
        .any(|change| change.path == "new file.txt" && change.kind.as_str() == "untracked"));
}

#[test]
fn rejects_a_directory_that_is_not_a_repository() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

    let error = runtime
        .block_on(open_repository(directory.path()))
        .expect_err("non-repository must fail");

    assert_eq!(error.code, "NOT_REPOSITORY");
}
