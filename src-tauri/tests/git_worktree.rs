use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::time::Duration;

use simplegit_lib::git::{
    cancel_operation, commit, commit_tracked, file_diff, recent_subjects, stage_all, stage_paths,
    staged_diff, unstage_paths, validate_relative_path,
};
use simplegit_lib::operations::OperationRegistry;
use tempfile::TempDir;

fn git(repository: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(repository)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git should be available")
}

fn git_ok(repository: &Path, args: &[&str]) -> String {
    let output = git(repository, args);
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn create_repository() -> TempDir {
    let repository = tempfile::tempdir().expect("temporary directory");
    git_ok(repository.path(), &["init", "-b", "main"]);
    git_ok(
        repository.path(),
        &["config", "user.name", "SimpleGit Test"],
    );
    git_ok(
        repository.path(),
        &["config", "user.email", "simplegit@example.test"],
    );
    fs::write(repository.path().join("a.txt"), "a\n").unwrap();
    fs::write(repository.path().join("b.txt"), "b\n").unwrap();
    fs::write(repository.path().join("c.txt"), "c\n").unwrap();
    git_ok(repository.path(), &["add", "."]);
    git_ok(repository.path(), &["commit", "-m", "chore: initial"]);
    repository
}

fn staged_names(repository: &Path) -> Vec<String> {
    let names = git_ok(repository, &["diff", "--cached", "--name-only"]);
    let mut names: Vec<String> = names.lines().map(ToString::to_string).collect();
    names.sort();
    names
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().expect("tokio runtime")
}

fn stage_hook(repository: &Path) -> PathBuf {
    let hook = repository.join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\nexit 1\n").expect("write hook");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&hook, permissions).unwrap();
    }

    hook
}

#[test]
fn stages_only_selected_paths() {
    let repository = create_repository();
    fs::write(repository.path().join("a.txt"), "a changed\n").unwrap();
    fs::write(repository.path().join("b.txt"), "b changed\n").unwrap();
    fs::write(repository.path().join("c.txt"), "c changed\n").unwrap();

    runtime()
        .block_on(stage_paths(
            repository.path(),
            &["a.txt".to_string(), "b.txt".to_string()],
        ))
        .expect("selected paths should stage");

    assert_eq!(staged_names(repository.path()), vec!["a.txt", "b.txt"]);
    assert!(git_ok(repository.path(), &["diff", "--name-only"]).contains("c.txt"));
}

#[test]
fn stages_modified_deleted_and_untracked_paths_with_stage_all() {
    let repository = create_repository();
    fs::write(repository.path().join("a.txt"), "changed\n").unwrap();
    fs::remove_file(repository.path().join("b.txt")).unwrap();
    fs::write(repository.path().join("new.txt"), "new\n").unwrap();

    runtime()
        .block_on(stage_all(repository.path()))
        .expect("all changes should stage");

    assert_eq!(
        staged_names(repository.path()),
        vec!["a.txt", "b.txt", "new.txt"]
    );
    assert!(git_ok(repository.path(), &["diff", "--name-only"]).is_empty());
}

#[test]
fn unstages_selected_paths_on_an_existing_branch() {
    let repository = create_repository();
    fs::write(repository.path().join("a.txt"), "changed\n").unwrap();
    runtime()
        .block_on(stage_all(repository.path()))
        .expect("change should stage");

    runtime()
        .block_on(unstage_paths(repository.path(), &["a.txt".to_string()]))
        .expect("change should unstage");

    assert!(staged_names(repository.path()).is_empty());
    assert!(git_ok(repository.path(), &["diff", "--name-only"]).contains("a.txt"));
}

#[test]
fn unstages_paths_before_the_first_commit() {
    let repository = tempfile::tempdir().expect("temporary directory");
    git_ok(repository.path(), &["init", "-b", "main"]);
    fs::write(repository.path().join("first.txt"), "first\n").unwrap();
    git_ok(repository.path(), &["add", "first.txt"]);

    runtime()
        .block_on(unstage_paths(repository.path(), &["first.txt".to_string()]))
        .expect("unborn branch path should unstage");

    assert!(staged_names(repository.path()).is_empty());
    assert!(git_ok(repository.path(), &["status", "--short"]).starts_with("??"));
}

#[test]
fn reads_staged_and_unstaged_worktree_diffs() {
    let repository = create_repository();
    fs::write(repository.path().join("a.txt"), "updated\n").unwrap();

    let unstaged = runtime()
        .block_on(file_diff(repository.path(), "a.txt", false))
        .expect("unstaged diff");
    assert!(unstaged.contains("-a"));
    assert!(unstaged.contains("+updated"));

    runtime()
        .block_on(stage_paths(repository.path(), &["a.txt".to_string()]))
        .expect("stage file");
    let staged = runtime()
        .block_on(file_diff(repository.path(), "a.txt", true))
        .expect("staged diff");
    assert!(staged.contains("-a"));
    assert!(staged.contains("+updated"));
}

#[test]
fn commits_the_exact_multiline_message() {
    let repository = create_repository();
    fs::write(repository.path().join("a.txt"), "updated\n").unwrap();
    runtime()
        .block_on(stage_paths(repository.path(), &["a.txt".to_string()]))
        .expect("stage file");
    let message = "feat(git): 精确保留提交说明\n\n中文正文保留细节";

    let result = runtime()
        .block_on(commit(repository.path(), message))
        .expect("commit should succeed");

    assert_eq!(
        git_ok(repository.path(), &["log", "-1", "--pretty=%B"]),
        message
    );
    assert_eq!(result.summary, "feat(git): 精确保留提交说明");
    assert_eq!(result.hash.len(), 40);
}

#[test]
fn rejects_a_commit_without_staged_changes() {
    let repository = create_repository();

    let error = runtime()
        .block_on(commit(repository.path(), "chore: empty"))
        .expect_err("empty commit must fail");

    assert_eq!(error.code, "NO_STAGED_CHANGES");
}

#[test]
fn reports_hook_failure_and_preserves_the_index() {
    let repository = create_repository();
    stage_hook(repository.path());
    fs::write(repository.path().join("a.txt"), "hook rejection\n").unwrap();
    runtime()
        .block_on(stage_paths(repository.path(), &["a.txt".to_string()]))
        .expect("stage file");
    let previous_head = git_ok(repository.path(), &["rev-parse", "HEAD"]);

    let error = runtime()
        .block_on(commit(repository.path(), "fix: hook rejection"))
        .expect_err("rejecting hook must fail the commit");

    assert_eq!(error.code, "GIT_FAILED");
    assert_eq!(
        git_ok(repository.path(), &["rev-parse", "HEAD"]),
        previous_head
    );
    assert_eq!(staged_names(repository.path()), vec!["a.txt"]);
}

#[test]
fn rejects_paths_that_escape_the_repository() {
    assert_eq!(
        validate_relative_path("../outside.txt")
            .expect_err("parent traversal must fail")
            .code,
        "INVALID_PATH"
    );
    #[cfg(windows)]
    assert_eq!(
        validate_relative_path("C:\\Windows\\system.ini")
            .expect_err("absolute Windows path must fail")
            .code,
        "INVALID_PATH"
    );
    assert!(validate_relative_path("src/main.ts").is_ok());
}

#[test]
fn reads_the_complete_staged_diff_and_recent_subjects() {
    let repository = create_repository();
    fs::write(repository.path().join("a.txt"), "a generated\n").unwrap();
    fs::write(repository.path().join("b.txt"), "b generated\n").unwrap();
    git_ok(repository.path(), &["add", "a.txt", "b.txt"]);

    let diff = runtime()
        .block_on(staged_diff(repository.path()))
        .expect("staged diff");
    let subjects = runtime()
        .block_on(recent_subjects(repository.path(), 1))
        .expect("recent subjects");

    assert!(diff.contains("a.txt"));
    assert!(diff.contains("b.txt"));
    assert_eq!(subjects, vec!["chore: initial"]);
}

#[test]
fn cancellation_terminates_a_long_running_commit_hook() {
    let repository = create_repository();
    let hook = repository.path().join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\nsleep 10\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&hook, permissions).unwrap();
    }
    fs::write(repository.path().join("a.txt"), "pending\n").unwrap();
    runtime()
        .block_on(stage_paths(repository.path(), &["a.txt".to_string()]))
        .expect("stage file");
    let previous_head = git_ok(repository.path(), &["rev-parse", "HEAD"]);
    let registry = Arc::new(OperationRegistry::new());
    let task_registry = Arc::clone(&registry);
    let repository_path = repository.path().to_path_buf();

    runtime().block_on(async move {
        let task = tokio::spawn(async move {
            commit_tracked(
                &repository_path,
                "fix: slow hook",
                "slow-commit",
                &task_registry,
            )
            .await
        });
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(cancel_operation("slow-commit", &registry).await.unwrap());
        let error = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("cancelled commit should finish")
            .expect("commit task should not panic")
            .expect_err("cancelled commit must fail");
        assert_eq!(error.code, "OPERATION_CANCELLED");
    });

    assert_eq!(
        git_ok(repository.path(), &["rev-parse", "HEAD"]),
        previous_head
    );
}
