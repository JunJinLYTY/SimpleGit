# SimpleGit 命令协议

前端通过 `src/api.ts` 调用 Rust 命令。本文记录当前稳定边界；新增或修改命令时，应同时更新此文档、`src/types.ts`、`src/api.ts`、Rust 命令注册和契约测试。

同步基准：`SimpleGit 0.1.0`，核对日期 `2026-11-15`。

## 通用约定

- Tauri 命令名使用 `snake_case`，例如 `open_repository`。
- TypeScript 方法和参数使用 `camelCase`，例如 `openRepository(path)`。
- Rust 结构体使用 `#[serde(rename_all = "camelCase")]` 与前端字段对齐。
- 命令失败返回 `{ code, message, details? }`；前端统一转换为 `SimpleGitCommandError`。
- 返回应用设置的命令返回完整 `AppSettings`，调用方应使用返回值替换本地状态。
- 改变 Git 状态的命令通常返回最新 `RepositorySnapshot`，避免前端猜测刷新结果。

前端契约定义在 `src/types.ts`，调用包装位于 `src/api.ts`，后端入口位于 `src-tauri/src/commands.rs`，命令注册位于 `src-tauri/src/lib.rs`。

## 仓库读取

| 方法 | Tauri 命令 | 参数 | 返回 |
| --- | --- | --- | --- |
| `openRepository` | `open_repository` | `path: string` | `RepositorySnapshot` |
| `repositoryStatus` | `repository_status` | `path: string` | `RepositorySnapshot` |
| `readDiff` | `read_diff` | `path`, `filePath`, `staged` | `string` |

`RepositorySnapshot` 包含 `repository` 和 `changes`。文件路径使用工作树相对路径；`originalPath` 只在重命名记录中存在。

## 暂存与提交

| 方法 | Tauri 命令 | 参数 | 返回 |
| --- | --- | --- | --- |
| `stageSelected` | `stage_selected` | `path`, `paths: string[]` | `RepositorySnapshot` |
| `stageAll` | `stage_all` | `path` | `RepositorySnapshot` |
| `unstageSelected` | `unstage_selected` | `path`, `paths: string[]` | `RepositorySnapshot` |
| `commit` | `commit` | `repositoryPath`, `message`, `operationId` | `CommitResult` |

前端在 `paths` 为空时提供明确的“暂存全部”入口，而后端 `stage_selected` 对空数组不做变更。首次提交前取消暂存使用 `git rm --cached` 路径。

`CommitResult`：

```json
{
  "hash": "完整提交 SHA",
  "summary": "提交摘要"
}
```

## 同步与取消

| 方法 | Tauri 命令 | 参数 | 返回 |
| --- | --- | --- | --- |
| `pull` | `pull` | `repositoryPath`, `operationId` | `OperationResult` |
| `push` | `push` | `repositoryPath`, `setUpstream`, `operationId` | `OperationResult` |
| `cancelOperation` | `cancel_operation` | `operationId` | `boolean` |

`OperationResult.output` 是 Git stdout/stderr 合并后的可读文本。`cancelOperation` 返回 `true` 表示该操作仍在注册表中并已请求取消，返回 `false` 表示没有找到活动操作。

`operationId` 由前端生成，在单次 Commit、Pull、Push 或 AI 生成期间唯一。重复 ID 会被拒绝。取消 Commit、Pull、Push 会终止 Git 进程树；取消 AI 生成会取消对应异步令牌。

## 设置与模型

| 方法 | Tauri 命令 | 参数 | 返回 |
| --- | --- | --- | --- |
| `loadSettings` | `load_settings` | 无 | `AppSettings` |
| `saveModelProfile` | `save_model_profile` | `input: ModelProfileInput` | `AppSettings` |
| `deleteModelProfile` | `delete_model_profile` | `profileId` | `AppSettings` |
| `setActiveModel` | `set_active_model` | `profileId: string \| null` | `AppSettings` |
| `saveCommitRule` | `save_commit_rule` | `rule` | `AppSettings` |
| `testModelProfile` | `test_model_profile` | `profileId` | `ConnectionTestResult` |
| `generateCommit` | `generate_commit` | `repositoryPath`, `modelId`, `operationId` | `string` |
| `recordRecentRepository` | `record_recent_repository` | `path` | `AppSettings` |
| `diagnostics` | `diagnostics` | 无 | `Diagnostics` |

`ModelProfileInput` 的 API Key 语义：

- 编辑时省略或留空 `apiKey`，保留已有密钥。
- 提供非空 `apiKey`，替换密钥。
- `clearApiKey: true`，删除已有密钥。该字段已由后端支持；当前设置界面没有暴露独立清除入口。

`AppSettings.models[].hasApiKey` 是派生状态，不参与 JSON 序列化。保存模型和读取设置时必须通过 `SecretStore` 计算该值。

`Diagnostics` 返回 `gitVersion`、`gitPath` 和 `appVersion`。Git 不可用时 `gitVersion` 为 `Git 不可用`，诊断命令本身仍可返回。

## 错误结构

```ts
interface CommandError {
  code: string;
  message: string;
  details?: string;
}
```

稳定错误码：

| 错误码 | 含义 |
| --- | --- |
| `GIT_NOT_INSTALLED` | PATH 中找不到 Git |
| `NOT_REPOSITORY` | 所选目录不是 Git 工作树 |
| `INVALID_PATH` | 路径不可访问或不应交给 Git |
| `NO_STAGED_CHANGES` | 缺少可提交或可生成内容的已暂存变更 |
| `CONFLICTS_PRESENT` | 存在未解决冲突，不能生成或提交 |
| `UPSTREAM_MISSING` | 当前分支没有 upstream |
| `AUTH_FAILED` | 远端认证失败 |
| `NON_FAST_FORWARD` | Push 或 Pull 遇到非快进状态 |
| `GIT_FAILED` | 其他 Git 执行失败 |
| `MODEL_NOT_CONFIGURED` | 未选择有效模型档案 |
| `MODEL_REQUEST_FAILED` | 端点、网络、HTTP 或响应格式失败 |
| `DIFF_TOO_LARGE` | 已暂存 diff 超过 60,000 字符 |
| `SETTINGS_FAILED` | 设置文件或系统凭据库失败 |
| `OPERATION_CANCELLED` | 用户取消长操作 |

`message` 面向用户并保持中文；`details` 用于 Git、网络或模型技术信息。不要在 `details` 中加入 API Key。

## 修改命令的同步清单

新增或修改一条命令时至少完成以下步骤：

1. 在 Rust 实现命令并保持参数名可序列化为预期 camelCase。
2. 在 `src-tauri/src/lib.rs` 的 `generate_handler!` 中注册。
3. 在 `src/types.ts` 增加类型和方法签名。
4. 在 `src/api.ts` 实现调用。
5. 更新 `src/api.test.ts`，覆盖命令名、参数和错误归一化。
6. 增加 Rust 单元或集成测试，覆盖成功与失败分支。
7. 更新本文命令表和错误表。
8. 若改变业务行为，更新 `architecture.md` 与相关面向用户的 `README.md`。
