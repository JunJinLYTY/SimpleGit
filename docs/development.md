# SimpleGit 开发指南

本文面向本地开发和功能扩展。使用说明见根目录 `README.md`，模块关系见 [architecture.md](architecture.md)，命令协议见 [api.md](api.md)。

同步基准：`SimpleGit 0.1.0`，核对日期 `2026-11-15`。

## 环境准备

基础环境：

- Node.js 24+ 和 npm 11+
- Rust stable 与 Cargo
- 系统 Git 2.28+
- Windows：Visual Studio 2022 Build Tools，包含 MSVC C++ 和 Windows SDK
- macOS 预留：Xcode Command Line Tools

先确认工具可见：

```powershell
node --version
npm --version
rustc --version
cargo --version
git --version
```

安装 JavaScript 依赖：

```powershell
npm install
```

Cargo 依赖在首次检查、测试或构建时下载并编译。

## 常用命令

| 目的 | 命令 |
| --- | --- |
| 启动 Tauri 开发应用 | `npm run tauri dev` |
| 只启动 Vite | `npm run dev` |
| 前端测试 | `npm test` |
| 前端监听测试 | `npm run test:watch` |
| TypeScript 检查与生产构建 | `npm run build` |
| Rust 格式检查 | `cargo fmt --manifest-path src-tauri\Cargo.toml --check` |
| Rust 静态检查 | `cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings` |
| Rust 测试 | `cargo test --manifest-path src-tauri\Cargo.toml` |
| Windows 打包 | `npm run tauri build` |

`npm run build` 先执行 `tsc --noEmit`，再由 Vite 写入 `dist/`。Tauri 构建会调用同一前端构建，然后编译 Rust 并生成安装包。

## 测试分层

### React 单元与组件测试

- `src/api.test.ts` 校验每个 Tauri 命令名、参数和错误归一化。
- `src/App.test.tsx` 校验打开仓库、暂存、diff、生成、提交、同步确认、取消和错误保留。
- `src/components/SettingsDialog.test.tsx` 校验模型表单、密钥输入语义、连接测试、删除、规则保存和未保存确认。
- 测试使用注入的 `SimpleGitClient` 和目录选择函数，不启动真实 Tauri 窗口。

运行：

```powershell
npm test
```

### Rust 单元测试

porcelain v2 解析、提示词约束、端点规范化、响应提取、URL 凭据脱敏和取消注册表等纯逻辑优先使用单元测试。

### Rust 集成测试

- `src-tauri/tests/git_status.rs`：真实临时仓库、工作树内打开、非仓库拒绝、重命名和冲突解析。
- `src-tauri/tests/git_worktree.rs`：选择/全部暂存、取消暂存、首次提交前状态、diff、stdin 多行提交、hook 失败和取消。
- `src-tauri/tests/git_sync.rs`：本地 bare remote、首次 upstream、fast-forward、分叉、非快进、锁串行化与取消。
- `src-tauri/tests/settings.rs`：模型档案、系统密钥、默认模型回退、设置往返、损坏文件保护、最近仓库和真实凭据往返。
- `src-tauri/tests/ai_client.rs`：本地临时 HTTP 服务、鉴权头、超时、非法响应、响应数组和字符上限。

涉及以下行为的改动必须保留真实 Git 或真实本地 HTTP 验证：Git 参数语义、暂存区状态、hook、remote、凭据库、模型协议。只 mock 到 Rust service 边界会掩盖这些风险。

## 典型开发流程

1. 先检查工作区，保留与当前任务无关的改动。
2. 阅读相关前端状态流、Rust 业务模块和现有测试。
3. 先定义一个能复现缺口或约束目标行为的测试。
4. 做最小完整实现，避免顺手重构无关模块。
5. 运行受影响的前端或 Rust 测试，再做完整静态检查。
6. 根据 [maintenance.md](maintenance.md) 更新文档。
7. 检查最终 diff，确认没有生成物、密钥和无关格式变化。

## 常见改动路径

### 修改界面文案或布局

主要位置：

- `src/components/`
- `src/App.tsx` 中与按钮启用、状态提示和操作编排有关的逻辑
- `src/styles.css`

若改动只影响展示，优先复用现有 props；若新增页面级状态，把协调逻辑放在 `App`，让组件保持可通过 fake client 测试。

### 新增 Git 操作

1. 在 `src-tauri/src/git/worktree.rs` 或 `src-tauri/src/git/sync.rs` 实现业务。
2. 所有 Git 调用经 `git/runner.rs`，参数保持数组形式。
3. 在 `commands.rs` 暴露精简命令，并决定是否需要仓库锁和 operation ID。
4. 在 `lib.rs` 注册命令。
5. 同步 `src/types.ts`、`src/api.ts`、`src/api.test.ts` 和 `docs/api.md`。
6. 使用临时真实仓库覆盖成功、失败、冲突、upstream 或取消等真实分支。

不要把 Git 业务直接写进 Tauri command，也不要让前端拼接命令参数。

新增任何后台子进程时都必须经 `child_process::configure_background_command` 配置启动参数。发布版 SimpleGit 使用 Windows 无控制台子系统，遗漏该配置会让 `git --version`、状态刷新等短进程弹出黑窗口。

### 修改仓库状态解析

主要位置：

- `src-tauri/src/git/status.rs`
- `src-tauri/tests/git_status.rs`
- `src/types.ts`
- `src/components/ChangesPanel.tsx`

porcelain v2 使用 NUL 分隔记录，重命名记录还会占用下一条路径记录。任何字段新增都要考虑路径中的空格、改名、冲突和 detached HEAD。

### 新增模型配置字段

主要位置：

- `src-tauri/src/settings/mod.rs`
- `src-tauri/src/settings/store.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/ai/`
- `src/types.ts`
- `src/components/ModelProfileForm.tsx`
- `src-tauri/tests/settings.rs`
- `src-tauri/tests/ai_client.rs`

非敏感字段进入 `AppSettings`/`ModelProfile`；秘密值只能通过 `SecretStore`。若字段会写入已有 `settings.json`，必须明确迁移和缺省值策略，避免旧文件无法加载。

### 接入新的模型服务

服务必须兼容当前 Chat Completions 请求和 `choices[0].message.content` 响应，或先在 `ai/client.rs` 增加明确的协议分支。Base URL 可以省略 `/chat/completions`，后端会补齐一次并移除查询与片段。

若要改变固定安全提示、上下文上限或响应清洗规则，更新 `ai/prompt.rs`、相应集成测试、设计规格和本文档。不得把 diff 中的文本提升为模型指令。

### 修改取消或并发行为

主要位置：

- `src-tauri/src/operations.rs`
- `src-tauri/src/git/runner.rs`
- `src-tauri/tests/git_worktree.rs`
- `src-tauri/tests/git_sync.rs`
- `src/App.tsx`

取消必须同时处理前端 operation ID、Rust 注册表清理、子进程树退出和 `OPERATION_CANCELLED` 返回。Windows 和 Unix 的平台实现不能只依赖 `child.kill()`，否则 Git hook 或子进程可能残留。

## 前后端契约规则

Tauri 命令名及参数映射很容易出现“编译通过但运行时找不到参数”的问题。新增命令后至少检查：

- `generate_handler!` 是否注册。
- 前端 `invoke` 参数是否与 Rust 参数名转换后的 camelCase 一致。
- 返回结构是否使用相同字段命名。
- 错误是否为可序列化的 `AppError`。
- `src/api.test.ts` 是否断言了完整参数。

## 平台与发布约束

- Windows 是当前首个完成构建和烟测的平台。
- macOS 代码路径已预留，但实机启动、Keychain 和打包尚未验证。
- Windows 打包配置限定为 MSI；macOS 安装包必须在 macOS 主机构建验证。
- 真实云端模型计费流程、带认证 remote、MSI 升级/卸载/签名尚未完成正式分发级验证。

发布前执行完整检查：

```powershell
npm test
npm run build
cargo fmt --manifest-path src-tauri\Cargo.toml --check
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri build
```

具体环境、安装包路径和已知未验证项以最新验证记录为准。

## 调试建议

- 找不到 Git：查看设置中的诊断信息，或运行 `where.exe git`/`git --version`。
- Windows 出现黑窗口：确认新增的 Git、诊断或取消子进程调用了 `child_process::configure_background_command`。
- 前端请求参数错误：先检查 `src/api.test.ts` 和 Rust 命令签名。
- 仓库状态异常：运行 `git status --porcelain=v2 --branch -z` 对照解析测试。
- Hook 长时间阻塞：确认 Commit 使用 tracked runner，取消能看到 `OPERATION_CANCELLED`。
- Push/Pull 认证失败：应用禁用终端交互提示，凭据必须由系统 Git 凭据助手预先配置。
- 设置文件损坏：不要直接覆盖；先备份并检查 JSON，再执行迁移或修复。
