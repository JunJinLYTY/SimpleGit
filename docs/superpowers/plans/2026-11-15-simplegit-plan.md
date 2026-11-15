# SimpleGit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a working Windows-first Tauri desktop client for staging, AI-assisted commit message generation, committing, pulling, and pushing a single Git repository.

**Architecture:** React owns the renderer and invokes typed Tauri commands. Rust owns Git CLI execution, settings, OS credential storage, model HTTP requests, operation locking, and cancellation. The renderer never executes shell commands or reads saved API keys.

**Tech Stack:** Tauri 2, React 19, TypeScript, Vite, Vitest, Testing Library, Rust 1.100, Tokio, Reqwest, Keyring, Serde, Tempfile.

**Spec:** `docs/superpowers/specs/2026-11-15-simplegit-design.md`

## Global Constraints

- Windows is the first verified platform; source code must remain macOS-compatible.
- Only open existing local Git repositories. No Clone or Init.
- Exactly one repository workspace at a time.
- Display branch state only. No branch creation or switching.
- Git operations use the system Git CLI with argument arrays, never a shell string.
- Commit, Pull, and Push act on the current branch only.
- Pull always uses `--ff-only`.
- No force push, automatic stash, merge, rebase, or hook bypassing.
- AI generation uses only staged diff, branch, recent eight commit subjects, and one global editable rule.
- Default rule is Chinese Conventional Commits.
- Saved API keys never cross from Rust back to React.
- Diff input limit is exactly 60000 characters.
- Model response token limit defaults to 2048 and request temperature to 0.2.
- Same-repository Git operations must be serialized.

---

### Task 1: Scaffold Tauri, React, and test infrastructure

**Files:**
- Create: `.gitignore`
- Create: `package.json`
- Create: `package-lock.json`
- Create: `index.html`
- Create: `tsconfig.json`
- Create: `tsconfig.node.json`
- Create: `vite.config.ts`
- Create: `src/main.tsx`
- Create: `src/App.tsx`
- Create: `src/App.test.tsx`
- Create: `src/test/setup.ts`
- Create: `src/styles.css`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`
- Create: `README.md`

**Interfaces:**
- Consumes: none.
- Produces: `npm run dev`, `npm run build`, `npm test`, `cargo test`, and a Tauri application named `SimpleGit`.

- [ ] **Step 1: Create the frontend manifest and configuration**

Use exact core versions:

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "test": "vitest run",
    "test:watch": "vitest",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.12.4",
    "@tauri-apps/plugin-dialog": "^2.8.4",
    "react": "^19.3.1",
    "react-dom": "^19.3.1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.12.4",
    "@testing-library/jest-dom": "^7.0.1",
    "@testing-library/react": "^16.3.3",
    "@testing-library/user-event": "^14.6.10",
    "@types/react": "^19.3.0",
    "@types/react-dom": "^19.3.0",
    "@vitejs/plugin-react": "^6.3.0",
    "jsdom": "^31.0.1",
    "typescript": "^7.0.2",
    "vite": "^8.3.8",
    "vitest": "^5.1.0"
  }
}
```

Set Vite `clearScreen: false`, `server.port: 1420`, and `server.strictPort: true`. Configure Vitest with `environment: "jsdom"`, `setupFiles: "./src/test/setup.ts"`, `globals: true`, and `css: true`.

- [ ] **Step 2: Create the minimum renderer and smoke test**

`src/App.test.tsx` must render `<App />` and assert that a level-one heading named `SimpleGit` is present. The initial `App` returns the same heading. Run:

```powershell
npm test
```

Expected before dependencies are installed: command fails because packages are missing. After `npm install`, expected: one passing test.

- [ ] **Step 3: Create the Tauri shell**

Use `tauri = { version = "2" }`, `tauri-build = { version = "2" }`, `serde`, and `serde_json`. Configure product name `SimpleGit`, identifier `com.simplegit.desktop`, frontend development URL `http://localhost:1420`, frontend distribution `../dist`, and commands:

```json
{
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [{ "title": "SimpleGit", "width": 1120, "height": 760, "minWidth": 880, "minHeight": 620 }]
  },
  "bundle": {
    "active": true,
    "targets": "all"
  }
}
```

The default capability must contain only `core:default` and `dialog:allow-open`.

- [ ] **Step 4: Install and verify the scaffold**

Run:

```powershell
npm install
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
cargo check --manifest-path src-tauri\Cargo.toml
```

Expected: frontend test and build pass; Rust tests compile and pass.

- [ ] **Step 5: Commit**

```powershell
git add .
git commit -m "chore: scaffold Tauri React application"
```

### Task 2: Parse Git status and open repositories

**Files:**
- Create: `src-tauri/src/error.rs`
- Create: `src-tauri/src/git/mod.rs`
- Create: `src-tauri/src/git/status.rs`
- Create: `src-tauri/src/git/service.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/tests/git_status.rs`

**Interfaces:**
- Consumes: Tauri shell from Task 1.
- Produces:
  - `pub struct RepositoryInfo`
  - `pub struct FileChange`
  - `pub struct RepositorySnapshot`
  - `pub fn parse_status(input: &[u8]) -> RepositorySnapshot`
  - `pub async fn open_repository(path: &Path) -> AppResult<RepositorySnapshot>`
  - `pub async fn repository_status(path: &Path) -> AppResult<RepositorySnapshot>`

Implement serde with `camelCase`. Status kinds serialize as lowercase strings: `added`, `modified`, `deleted`, `renamed`, `untracked`, `conflicted`, `unknown`.

- [ ] **Step 1: Write failing parser tests**

Add literal byte fixtures covering:

```rust
# branch.oid 0123456789abcdef
# branch.head main
# branch.upstream origin/main
# branch.ab +2 -3
1 M. N... 100644 100644 100644 aaa bbb src/main.ts
1 .M N... 100644 100644 100644 aaa bbb docs/with space.md
? new file.txt
u UU N... 100644 100644 100644 100644 aaa bbb ccc merge.txt
```

NUL bytes separate records. Assert branch `main`, upstream `origin/main`, ahead `2`, behind `3`, four file changes, staged and unstaged flags, conflict flag, and paths containing spaces.

Add a rename fixture:

```rust
2 R. N... 100644 100644 100644 aaa bbb R100 new name.txt\0old name.txt\0
```

Assert `path == "new name.txt"` and `originalPath == Some("old name.txt")`.

- [ ] **Step 2: Verify tests fail**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml parse_status
```

Expected: compilation fails because `parse_status` and its types do not exist.

- [ ] **Step 3: Implement status parsing**

Parse headers and record types `1`, `2`, `u`, `?`, and `!`. Treat any `u` record as conflicted. Set `staged` when index status is not `.` and `unstaged` when worktree status is not `.`. Use `String::from_utf8_lossy` only when converting raw paths for the renderer.

- [ ] **Step 4: Add repository integration tests**

Create a temporary Git repo with `tempfile`. Initialize it, configure `user.name` and `user.email`, create tracked and untracked files, and assert `open_repository` returns the canonical root and expected changes. Also assert a non-Git directory returns `NOT_REPOSITORY`.

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml git_status
```

Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src src-tauri/tests
git commit -m "feat(git): parse repository status"
```

### Task 3: Implement staging, diff, and commit

**Files:**
- Create: `src-tauri/src/git/runner.rs`
- Create: `src-tauri/src/git/worktree.rs`
- Modify: `src-tauri/src/git/mod.rs`
- Modify: `src-tauri/src/git/service.rs`
- Create: `src-tauri/tests/git_worktree.rs`

**Interfaces:**

```rust
pub async fn stage_paths(repo: &Path, paths: &[String]) -> AppResult<()>;
pub async fn stage_all(repo: &Path) -> AppResult<()>;
pub async fn unstage_paths(repo: &Path, paths: &[String]) -> AppResult<()>;
pub async fn file_diff(repo: &Path, path: &str, staged: bool) -> AppResult<String>;
pub async fn commit(repo: &Path, message: &str) -> AppResult<CommitResult>;
```

`CommitResult` contains `hash: String` and `summary: String`.

- [ ] **Step 1: Write failing integration tests**

Use temporary repositories to test:

- `stage_paths` stages exactly two selected files and leaves a third unstaged.
- `stage_all` stages tracked modifications, deletions, and untracked files.
- `unstage_paths` reverses staging on a repository with an existing HEAD.
- `unstage_paths` also works before the first commit.
- `file_diff` returns the added line for staged and unstaged modes.
- `commit` creates a commit whose exact two-line message survives.
- `commit` with no staged changes returns `NO_STAGED_CHANGES`.
- an executable `.git/hooks/pre-commit` that exits `1` produces `GIT_FAILED`, leaves files staged, and preserves no false success.

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test git_worktree
```

Expected: compile failure while the worktree API is absent.

- [ ] **Step 2: Implement safe Git execution**

Run Git with `tokio::process::Command`, set the repository as `current_dir`, use `.arg()` for every argument, capture stdout/stderr, and use exit status to decide success.

For `commit`, start `git commit -F -`, write the UTF-8 message to stdin, then wait. Do not use a temporary message file or shell.

For unstaging in an unborn branch, detect missing `HEAD` with `git rev-parse --verify HEAD`; use `git rm --cached --ignore-unmatch -- <paths>` in that state.

- [ ] **Step 3: Implement path validation**

Reject absolute paths, parent traversal, NUL, and paths outside the canonical repository root. Write a unit test asserting `../outside.txt` is rejected before Git is invoked.

- [ ] **Step 4: Verify PASS**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test git_worktree
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/git src-tauri/tests/git_worktree.rs
git commit -m "feat(git): stage files and create commits"
```

### Task 4: Add pull, push, operation locking, and cancellation

**Files:**
- Create: `src-tauri/src/operations.rs`
- Create: `src-tauri/tests/git_sync.rs`
- Modify: `src-tauri/src/git/runner.rs`
- Modify: `src-tauri/src/git/service.rs`
- Modify: `src-tauri/src/git/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**

```rust
pub async fn pull(repo: &Path, operation_id: &str, registry: &OperationRegistry) -> AppResult<OperationResult>;
pub async fn push(repo: &Path, set_upstream: bool, operation_id: &str, registry: &OperationRegistry) -> AppResult<OperationResult>;
pub async fn cancel_operation(operation_id: &str, registry: &OperationRegistry) -> AppResult<bool>;
```

`OperationResult` contains `output: String`.

- [ ] **Step 1: Write failing sync tests**

Create a bare remote and two working clones. Test:

- first push with `set_upstream = true` creates upstream tracking.
- pull fast-forwards a clean clone.
- pull returns `UPSTREAM_MISSING` when branch has no upstream.
- pull returns `GIT_FAILED` when local and remote diverge.
- push returns an error when the remote has a newer commit.
- two simultaneous operations against the same repository are serialized; one operation observes the other's completed filesystem effect.

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test git_sync
```

Expected: compile failure while sync APIs are absent.

- [ ] **Step 2: Implement locking**

`OperationRegistry` owns `HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>` and active process IDs keyed by operation ID. Acquire the repository lock before starting every state-changing or network Git operation.

- [ ] **Step 3: Implement sync and cancellation**

Pull runs `git pull --ff-only`. Push runs `git push`, or `git push -u origin HEAD` only when `set_upstream` is true.

Register child process IDs while running. `cancel_operation` terminates the process tree with `taskkill /PID <pid> /T /F` on Windows and a process-group `kill` on Unix. A cancelled operation maps to `OPERATION_CANCELLED`.

- [ ] **Step 4: Verify PASS and clippy**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test git_sync
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
```

Expected: PASS with no clippy warnings.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/operations.rs src-tauri/src/git src-tauri/src/lib.rs src-tauri/tests/git_sync.rs
git commit -m "feat(git): add pull push and cancellation"
```

### Task 5: Persist model profiles and secrets

**Files:**
- Create: `src-tauri/src/settings/mod.rs`
- Create: `src-tauri/src/settings/store.rs`
- Create: `src-tauri/src/secrets.rs`
- Create: `src-tauri/tests/settings.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**

```rust
pub trait SecretStore: Send + Sync {
    fn set(&self, profile_id: &str, secret: &str) -> AppResult<()>;
    fn clear(&self, profile_id: &str) -> AppResult<()>;
    fn has(&self, profile_id: &str) -> AppResult<bool>;
    fn get(&self, profile_id: &str) -> AppResult<Option<String>>;
}

pub struct AppSettings {
    pub version: u32,
    pub recent_repositories: Vec<String>,
    pub active_model_id: Option<String>,
    pub models: Vec<ModelProfile>,
    pub commit_rule: String,
}

pub struct ModelProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
    pub is_default: bool,
}
```

- [ ] **Step 1: Write failing settings tests**

Use a memory secret store in tests. Assert:

- Default settings contain version `1`, one empty default rule constant, no models, no active model, and no recent repositories.
- Adding two profiles retains both and preserves insertion order.
- Updating a profile without a new API key keeps the existing secret.
- Setting a new key replaces the existing secret value.
- Deleting a profile removes its secret and selects the first remaining model.
- Setting a default profile clears the previous default.
- Saving and loading through a temporary app-data directory round-trips all non-secret fields.
- A corrupted settings JSON returns `SETTINGS_FAILED` and does not overwrite the file.

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test settings
```

Expected: compile failure while config APIs are absent.

- [ ] **Step 2: Implement settings repository**

Store `settings.json` under Tauri's app-data directory. Use a temp file in the same directory and an atomic replace. Keep at most ten recent repository paths, deduplicated by canonical path and newest first.

- [ ] **Step 3: Implement OS secret storage**

Use the `keyring` crate with service name `com.simplegit.desktop` and account `model:<profile-id>`. Do not include secrets in debug output.

- [ ] **Step 4: Verify PASS**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test settings
```

Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/settings src-tauri/src/secrets.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/tests/settings.rs
git commit -m "feat(settings): persist model profiles securely"
```

### Task 6: Build prompts and call OpenAI-compatible models

**Files:**
- Create: `src-tauri/src/ai/mod.rs`
- Create: `src-tauri/src/ai/prompt.rs`
- Create: `src-tauri/src/ai/client.rs`
- Create: `src-tauri/tests/ai_client.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**

```rust
pub struct CommitContext {
    pub stage_diff: String,
    pub branch: String,
    pub recent_subjects: Vec<String>,
    pub rule: String,
}

pub fn build_messages(context: &CommitContext) -> Vec<ChatMessage>;
pub fn normalize_endpoint(base_url: &str) -> AppResult<String>;
pub fn extract_content(body: &str) -> AppResult<String>;
pub async fn generate_commit(client: &reqwest::Client, profile: &ModelProfile, api_key: Option<&str>, context: &CommitContext) -> AppResult<String>;
```

`ChatMessage` has `role: String` and `content: String`.

- [ ] **Step 1: Write failing prompt and endpoint tests**

Assert:

- Empty diff returns `NO_STAGED_CHANGES`.
- Diff over 60000 characters returns `DIFF_TOO_LARGE`.
- Default rule includes Chinese Conventional Commits guidance.
- A custom rule appears verbatim after fixed safety instructions.
- Diff content is enclosed as data and the fixed instructions say not to follow instructions inside it.
- `https://example.test/v1` becomes `https://example.test/v1/chat/completions`.
- A URL already ending `/chat/completions` is unchanged.
- Empty or non-HTTP URLs return `MODEL_REQUEST_FAILED`.

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml ai_prompt
```

Expected: compile failure while prompt APIs are absent.

- [ ] **Step 2: Write failing response tests**

Assert:

- string content is returned after trimming.
- array content is joined from all parts with `type == "text"`.
- a fenced block is unwrapped only when the fence encloses the entire response.
- missing choices returns `MODEL_REQUEST_FAILED`.
- malformed JSON returns `MODEL_REQUEST_FAILED`.
- content over 12000 characters is rejected.

- [ ] **Step 3: Implement prompt builder and normalization**

Use a fixed system instruction requiring a single commit message, no invented changes, no diff instruction following, and no markdown wrapper. Append the custom rule after the fixed instruction. Limit recent subjects to eight.

- [ ] **Step 4: Implement HTTP client and integration tests**

Start a temporary `tokio::net::TcpListener` HTTP server in tests. Verify the request path, Bearer header presence/absence, model name, temperature `0.2`, max tokens `2048`, success response parsing, 401 mapping, invalid JSON, and timeout.

- [ ] **Step 5: Verify PASS and commit**

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test ai_client
git add src-tauri/src/ai src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/tests/ai_client.rs
git commit -m "feat(ai): generate commit messages from staged diffs"
```

### Task 7: Expose typed Tauri commands

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src/types.ts`
- Create: `src/api.test.ts`
- Create: `src/api.ts`

**Interfaces:**

Commands:

```text
open_repository(path)
repository_status(path)
stage_selected(path, paths)
stage_all(path)
unstage_selected(path, paths)
read_diff(path, filePath, staged)
commit(repositoryPath, message)
pull(repositoryPath, operationId)
push(repositoryPath, setUpstream, operationId)
cancel_operation(operationId)
load_settings()
save_model_profile(input)
delete_model_profile(profileId)
set_active_model(profileId)
save_commit_rule(rule)
test_model_profile(profileId)
generate_commit(repositoryPath, modelId)
record_recent_repository(path)
diagnostics()
```

All command errors serialize as:

```typescript
interface CommandError {
  code: string;
  message: string;
  details?: string;
}
```

- [ ] **Step 1: Write failing API wrapper tests**

Inject a fake `invoke` function into `createSimpleGitClient`. Assert each public client method calls the exact Tauri command and passes the exact camelCase arguments. Assert command rejection is normalized to `CommandError`.

Run:

```powershell
npm test -- src/api.test.ts
```

Expected: failure because `createSimpleGitClient` does not exist.

- [ ] **Step 2: Implement TypeScript contracts and wrapper**

Export `SimpleGitClient`, `RepositorySnapshot`, `FileChange`, `ModelProfile`, `AppSettings`, and `Diagnostics`. Export a `tauriClient` using the real Tauri API.

- [ ] **Step 3: Implement Tauri commands**

Commands remain thin: validate at the service boundary, call a module, and serialize results. Inject `AppState` containing settings store, secret store, AI HTTP client, and operation registry.

- [ ] **Step 4: Verify both sides compile**

```powershell
npm test -- src/api.test.ts
cargo check --manifest-path src-tauri\Cargo.toml
```

Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add src/api.ts src/api.test.ts src/types.ts src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(api): expose typed desktop commands"
```

### Task 8: Build the main commit workspace

**Files:**
- Create: `src/components/RepositoryHeader.tsx`
- Create: `src/components/ChangesPanel.tsx`
- Create: `src/components/DiffView.tsx`
- Create: `src/components/CommitComposer.tsx`
- Create: `src/components/OperationBar.tsx`
- Create: `src/App.tsx`
- Create: `src/App.test.tsx`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: `SimpleGitClient` from Task 7.
- Produces: `<App client={SimpleGitClient} />`; default prop is `tauriClient`.

- [ ] **Step 1: Write failing renderer behavior tests**

With an injected fake client, assert:

- Empty state offers “打开仓库” and calls the folder dialog boundary.
- A loaded snapshot shows repository name, branch, ahead/behind, staged and unstaged files.
- Add with no selection calls `stageAll`.
- Selecting two unselected paths and clicking Add calls `stageSelected` with exactly those paths.
- Clicking a file expands real diff text rendered by the injected client.
- Empty Commit generates first and does not call `commit`.
- A second Commit with generated message calls `commit` with that exact message.
- Manual text commits without calling `generateCommit`.
- Generate and Commit are disabled when there are no staged files or when conflicts exist.
- Push with missing upstream displays confirmation, then calls push with `setUpstream: true`.
- Pull and Push show cancel action while pending.
- Git and model errors keep the message editor content and display error details.

Run:

```powershell
npm test -- src/App.test.tsx
```

Expected: failures because the components and behavior do not exist.

- [ ] **Step 2: Implement state orchestration**

Keep repository state, selected paths, expanded file, editor message, model ID, pending operation, confirmations, and error in one reducer. Refresh status after open, stage, unstage, commit, pull, and push. Use `crypto.randomUUID()` for long operation IDs.

- [ ] **Step 3: Implement components and accessible interactions**

Use buttons, checkboxes, `aria-expanded`, labelled regions, and visible focus styles. Add semantic status messages for loading and operation results.

- [ ] **Step 4: Implement the visual system**

Use a compact desktop layout with neutral light surfaces, high-contrast text, restrained green for commit actions, amber for caution, and red for conflicts. Include a dark-mode media query. No remote fonts or network-loaded assets.

- [ ] **Step 5: Verify PASS and commit**

```powershell
npm test -- src/App.test.tsx
npm run build
git add src/App.tsx src/App.test.tsx src/components src/styles.css
git commit -m "feat(ui): add single-page commit workspace"
```

### Task 9: Build settings and multi-model management

**Files:**
- Create: `src/components/SettingsDialog.tsx`
- Create: `src/components/ModelProfileForm.tsx`
- Create: `src/components/SettingsDialog.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: settings commands from Task 7.
- Produces: accessible modal that edits profiles, active model, and the single global rule.

- [ ] **Step 1: Write failing settings behavior tests**

Assert:

- Opening settings loads profiles and displays key state without displaying a saved key.
- Adding a profile validates name, Base URL, and Model ID before calling `saveModelProfile`.
- Blank API Key on edit preserves the key by omitting `apiKey`.
- A typed replacement key is passed once and cleared from the form.
- “测试连接” calls `testModelProfile` and displays success or failure.
- Deleting the active profile loads the new active selection.
- Editing and saving the commit rule sends the exact text.
- Closing with unsaved form edits asks for confirmation.

Run:

```powershell
npm test -- src/components/SettingsDialog.test.tsx
```

Expected: failures because the dialog does not exist.

- [ ] **Step 2: Implement profile dialogs and forms**

Use one list/detail layout. API Key input is `type="password"`, starts blank, and displays “已配置” when `hasApiKey` is true. Never show a stored key value.

- [ ] **Step 3: Verify PASS and commit**

```powershell
npm test -- src/components/SettingsDialog.test.tsx
npm run build
git add src/App.tsx src/components/SettingsDialog.tsx src/components/ModelProfileForm.tsx src/components/SettingsDialog.test.tsx src/styles.css
git commit -m "feat(settings): manage multiple model profiles"
```

### Task 10: Verify packaging, runtime behavior, and documentation

**Files:**
- Create: `README.md`
- Create: `docs/manual-test.md`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `package.json`

**Interfaces:**
- Consumes: all prior tasks.
- Produces: documented development commands and a Windows installer.

- [ ] **Step 1: Run complete automated verification**

```powershell
npm test
npm run build
cargo fmt --manifest-path src-tauri\Cargo.toml --check
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri build
```

Expected: every command exits `0`; installer artifacts appear under `src-tauri/target/release/bundle/`.

- [ ] **Step 2: Perform a real local repository smoke test**

Use a temporary repository and real Git CLI:

1. Open the repository.
2. Modify three files, stage one selected file, then stage all.
3. Unstage one path.
4. Generate and confirm a commit.
5. Configure a temporary bare remote.
6. Push, clone into another temporary directory, commit there, and pull.

Record evidence in `docs/manual-test.md`.

- [ ] **Step 3: Inspect the rendered UI**

Run the Tauri app, capture screenshots of the empty state, populated main page, expanded diff, and settings dialog. Check text clipping, keyboard focus, narrow-window behavior at 880x620, and dark mode.

- [ ] **Step 4: Document setup and limitations**

README must include exact prerequisites, installation, development, test, build, model configuration, key storage behavior, supported Git operations, and explicit non-goals.

- [ ] **Step 5: Commit**

```powershell
git add README.md docs/manual-test.md src-tauri/tauri.conf.json package.json package-lock.json
git commit -m "docs: document setup and verification"
```
