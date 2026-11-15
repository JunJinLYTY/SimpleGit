use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::time::Duration;

use simplegit_lib::git::{cancel_operation, commit, pull, push, stage_paths};
use simplegit_lib::operations::OperationRegistry;
use tempfile::TempDir;

fn git(path: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git should be available")
}

fn git_ok(path: &Path, args: &[&str]) -> String {
    let output = git(path, args);
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn configure(path: &Path) {
    git_ok(path, &["config", "user.name", "SimpleGit Test"]);
    git_ok(path, &["config", "user.email", "simplegit@example.test"]);
}

fn create_repository(path: &Path) {
    fs::create_dir_all(path).unwrap();
    git_ok(path, &["init", "-b", "main"]);
    configure(path);
    fs::write(path.join("tracked.txt"), "initial\n").unwrap();
    git_ok(path, &["add", "."]);
    git_ok(path, &["commit", "-m", "chore: initial"]);
}

struct SyncFixture {
    _root: TempDir,
    first: PathBuf,
    second: PathBuf,
}

fn sync_fixture() -> SyncFixture {
    let root = tempfile::tempdir().expect("temporary directory");
    let remote = root.path().join("remote.git");
    let seed = root.path().join("seed");
    let first = root.path().join("first");
    let second = root.path().join("second");

    git_ok(
        root.path(),
        &["init", "--bare", "-b", "main", remote.to_str().unwrap()],
    );
    create_repository(&seed);
    let remote_string = remote.to_string_lossy().into_owned();
    git_ok(&seed, &["remote", "add", "origin", &remote_string]);
    git_ok(&seed, &["push", "-u", "origin", "main"]);
    git_ok(
        root.path(),
        &["clone", &remote_string, first.to_str().unwrap()],
    );
    git_ok(
        root.path(),
        &["clone", &remote_string, second.to_str().unwrap()],
    );
    configure(&first);
    configure(&second);

    SyncFixture {
        _root: root,
        first,
        second,
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().expect("tokio runtime")
}

fn make_commit(repository: &Path, file: &str, contents: &str, message: &str) {
    fs::write(repository.join(file), contents).unwrap();
    runtime()
        .block_on(stage_paths(repository, &[file.to_string()]))
        .expect("stage file");
    runtime()
        .block_on(commit(repository, message))
        .expect("commit file");
}

#[test]
fn first_push_sets_origin_upstream() {
    let root = tempfile::tempdir().unwrap();
    let remote = root.path().join("remote.git");
    let repository = root.path().join("repository");
    git_ok(
        root.path(),
        &["init", "--bare", "-b", "main", remote.to_str().unwrap()],
    );
    create_repository(&repository);
    git_ok(
        &repository,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    let registry = OperationRegistry::new();

    runtime()
        .block_on(push(&repository, true, "first-push", &registry))
        .expect("first push should succeed");

    assert_eq!(
        git_ok(
            &repository,
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}"
            ]
        ),
        "origin/main"
    );
    assert_eq!(
        git_ok(&remote, &["rev-parse", "refs/heads/main"]),
        git_ok(&repository, &["rev-parse", "HEAD"])
    );
}

#[test]
fn pull_fast_forwards_a_clean_clone() {
    let fixture = sync_fixture();
    make_commit(
        &fixture.first,
        "remote.txt",
        "remote\n",
        "feat: remote change",
    );
    git_ok(&fixture.first, &["push"]);
    let registry = OperationRegistry::new();

    runtime()
        .block_on(pull(&fixture.second, "pull", &registry))
        .expect("pull should fast-forward");

    assert_eq!(
        fs::read_to_string(fixture.second.join("remote.txt"))
            .unwrap()
            .trim(),
        "remote"
    );
}

#[test]
fn pull_requires_an_upstream() {
    let root = tempfile::tempdir().unwrap();
    let repository = root.path().join("repository");
    create_repository(&repository);
    let registry = OperationRegistry::new();

    let error = runtime()
        .block_on(pull(&repository, "pull-no-upstream", &registry))
        .expect_err("missing upstream must fail");

    assert_eq!(error.code, "UPSTREAM_MISSING");
}

#[test]
fn pull_rejects_diverged_history() {
    let fixture = sync_fixture();
    make_commit(&fixture.first, "remote.txt", "remote\n", "feat: remote");
    git_ok(&fixture.first, &["push"]);
    make_commit(&fixture.second, "local.txt", "local\n", "fix: local");
    let registry = OperationRegistry::new();

    let error = runtime()
        .block_on(pull(&fixture.second, "pull-diverged", &registry))
        .expect_err("diverged pull must fail");

    assert_eq!(error.code, "GIT_FAILED");
}

#[test]
fn push_reports_non_fast_forward() {
    let fixture = sync_fixture();
    make_commit(&fixture.first, "remote.txt", "remote\n", "feat: remote");
    git_ok(&fixture.first, &["push"]);
    make_commit(&fixture.second, "local.txt", "local\n", "fix: local");
    let registry = OperationRegistry::new();

    let error = runtime()
        .block_on(push(&fixture.second, false, "push-rejected", &registry))
        .expect_err("non-fast-forward push must fail");

    assert_eq!(error.code, "NON_FAST_FORWARD");
}

#[test]
fn repository_locks_serialize_operations() {
    let repository = tempfile::tempdir().unwrap();
    let path = repository.path().to_path_buf();
    let registry = Arc::new(OperationRegistry::new());
    let second_registry = Arc::clone(&registry);

    runtime().block_on(async move {
        let first_lock = registry.repository_lock(&path).await;
        let guard = first_lock.lock().await;
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let second_path = path.clone();
        let waiter = tokio::spawn(async move {
            let lock = second_registry.repository_lock(&second_path).await;
            let _guard = lock.lock().await;
            let _ = sender.send(());
        });

        assert!(tokio::time::timeout(Duration::from_millis(100), receiver)
            .await
            .is_err());
        drop(guard);
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("waiter should acquire the lock after release")
            .expect("waiter task");
    });
}

#[test]
fn cancellation_terminates_a_long_running_push() {
    let fixture = sync_fixture();
    make_commit(
        &fixture.first,
        "pending.txt",
        "pending\n",
        "feat: pending push",
    );
    let hook = fixture.first.join(".git/hooks/pre-push");
    fs::write(&hook, "#!/bin/sh\nsleep 10\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&hook, permissions).unwrap();
    }

    let registry = Arc::new(OperationRegistry::new());
    let task_registry = Arc::clone(&registry);
    let repository = fixture.first.clone();

    runtime().block_on(async move {
        let task =
            tokio::spawn(
                async move { push(&repository, false, "slow-push", &task_registry).await },
            );
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(cancel_operation("slow-push", &registry)
            .await
            .expect("cancel must execute"));

        let error = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("cancelled task should finish")
            .expect("push task should not panic")
            .expect_err("cancelled push must fail");
        assert_eq!(error.code, "OPERATION_CANCELLED");
    });
}
