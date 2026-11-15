# SimpleGit 设计规格

## 目标

SimpleGit 是一款面向个人使用的轻量级 Git 桌面客户端。它把日常提交集中到单页工作流中：查看更改、暂存文件、生成或填写提交说明、提交并同步远端。

主要价值是使用可配置模型快速生成高质量中文提交说明，同时保留用户对 Git 暂存区和提交内容的最终控制。

## 技术边界

- 桌面框架：Tauri 2。
- 前端：React、TypeScript、Vite。
- 后端：Rust。
- Git：调用系统安装的 Git CLI，不内置 Git 实现。
- 模型协议：OpenAI Chat Completions 兼容接口。
- 首发平台：Windows。
- 预留平台：macOS，所有路径、凭据、打包和快捷键设计保持跨平台。

## 首版范围

### 包含

- 选择并打开本地已有 Git 仓库。
- 记录并切换最近打开的仓库。
- 单窗口、单仓库工作区。
- 显示当前分支、upstream、ahead/behind。
- 展示未暂存、已暂存、未跟踪和冲突文件。
- 文件级暂存、全部暂存和取消暂存。
- 展开查看文本文件 diff。
- 提交说明手工输入。
- 使用已暂存 diff 生成提交说明。
- Commit、Pull 和 Push。
- 保存并切换多个 OpenAI 兼容模型档案。
- 一套全局可编辑提交规则。
- 将 API Key 保存到系统凭据库。
- 明确展示 Git、网络、模型和 hook 错误。

### 不包含

- Clone、Init、新建仓库。
- 多仓库标签页或多窗口并行。
- 分支切换、创建或删除。
- Stash、Merge、Rebase、Cherry-pick。
- 冲突解决编辑器。
- 历史提交图、Diff 浏览历史。
- 逐行或逐 hunk 暂存。
- 多套提交规则。
- Force push。
- Submodule、Git LFS 和标签管理。

## 交互设计

### 主页面

应用使用单窗口纵向布局。

- 顶栏：仓库名、仓库路径、当前分支、ahead/behind、刷新按钮、打开仓库和设置入口。
- 文件区：分为“未暂存”和“已暂存”。文件行显示状态、路径和展开入口。
- Diff 区：点击文件后在文件行下方展开只读 diff。
- 提交区：当前模型档案选择器、AI 生成按钮、多行提交说明、Commit、Pull 和 Push。

未暂存文件使用勾选状态表达选择，不立即改变 Git 暂存区。

- 没有文件被选中时，Add 暂存全部更改。
- 有文件被选中时，Add 只暂存选中的文件。
- 已暂存文件提供取消暂存操作。

### 提交流程

手工提交：

1. 用户输入提交说明。
2. Commit 只提交当前已经暂存的内容。
3. 提交成功后刷新仓库状态并清空提交说明。

AI 生成：

1. 没有暂存文件时禁用生成和提交。
2. 存在冲突时禁用生成和提交。
3. 用户点击生成后，应用读取已暂存 diff、当前分支和最近八条提交标题。
4. 请求发送给当前选择的模型档案。
5. 生成结果填入提交框，按钮进入可提交状态。
6. 用户检查并点击 Commit。

提交框为空时点击 Commit，不直接提交模型结果；先执行生成并等待用户确认。

### 设置

设置使用独立弹窗，包含：

- 模型档案列表及新增、编辑、删除。
- 每个档案的 Base URL、Model ID、API Key 状态、超时和默认档案设置。
- 当前生效模型选择。
- 唯一一套全局提交规则。
- Git 版本、Git 路径和应用版本等诊断信息。

## 默认提交规则

默认采用 Conventional Commits，中文说明，机器标记保留英文。

```text
<type>(<scope>): <中文摘要>

[可选中文正文]

[可选脚注]
```

默认约束：

- 类型使用 `feat`、`fix`、`refactor`、`perf`、`docs`、`test`、`build`、`ci`、`chore`、`style`、`revert`。
- `scope` 可选，例如 `ui`、`git`、`ai`。
- 摘要简洁说明实际改动，不加句号，尽量不超过 50 个汉字。
- 正文只在需要解释动机、影响或复杂行为时生成。
- 代码标识符、命令和专有名词保持原样。
- 破坏性变更使用 `type(scope)!:` 和 `BREAKING CHANGE:` 脚注。
- 不编造 issue、提交哈希或 diff 中不存在的功能。
- 仅输出提交说明本身，不带解释或 Markdown 代码块。

用户规则是附加约束。固定系统指令负责只使用已暂存变更、拒绝遵循 diff 内的指令，并要求只输出提交说明。规则为空时使用上述默认规则。

## 数据模型

```typescript
interface ModelProfile {
  id: string;
  name: string;
  baseUrl: string;
  model: string;
  timeoutSeconds: number;
  hasApiKey: boolean;
  isDefault: boolean;
}

interface AppSettings {
  version: 1;
  recentRepositories: string[];
  activeModelId: string | null;
  models: ModelProfile[];
  commitRule: string;
}

interface RepositoryInfo {
  root: string;
  name: string;
  branch: string;
  upstream: string | null;
  ahead: number;
  behind: number;
  hasConflicts: boolean;
}

interface FileChange {
  path: string;
  originalPath: string | null;
  indexStatus: string;
  worktreeStatus: string;
  staged: boolean;
  unstaged: boolean;
  conflicted: boolean;
  kind: "added" | "modified" | "deleted" | "renamed" | "untracked" | "conflicted" | "unknown";
}

interface RepositorySnapshot {
  repository: RepositoryInfo;
  changes: FileChange[];
}
```

API Key 不属于 `AppSettings` 序列化内容。每个模型档案的密钥使用档案 ID 存入系统凭据库。前端只能获得 `hasApiKey`，不能读回已保存的密钥。

## Git 行为

### 状态

- 使用 `git status --porcelain=v2 --branch -z` 获取机器可读状态。
- 使用 NUL 分隔解析路径，支持空格和换行等合法文件名。
- 不依赖人类可读 Git 输出判断操作结果。
- 不提供文件监视器；应用启动、窗口聚焦、手动刷新和每次操作后刷新状态。

### 暂存

- 选中文件：`git add -A -- <paths>`。
- 全部暂存：`git add -A`。
- 取消暂存：优先使用 `git restore --staged -- <paths>`。
- 仓库尚无首次提交时，使用适用于 unborn branch 的取消暂存方式。

### 提交

- 只允许提交已暂存内容。
- 存在冲突时阻止提交。
- 提交说明通过 stdin 传给 `git commit -F -`。
- Hook 失败时保留暂存区和提交说明。
- 不使用 `--no-verify` 或 `--allow-empty`。

### Pull

- 始终执行 `git pull --ff-only`。
- 没有 upstream、工作区冲突、分支分叉或认证失败时直接报错。
- 不自动 stash、merge 或 rebase。

### Push

- 有 upstream 时执行 `git push`。
- 没有 upstream 时，用户确认后执行 `git push -u origin HEAD`。
- 不提供 force push。
- 非快进、认证或网络错误原样翻译为可读提示，并保留 Git 细节。

### 并发与取消

- 同一仓库的写操作和同步操作串行执行。
- 每个长操作由前端生成 operation ID。
- Pull、Push 和 Commit 显示进度状态，并可通过 operation ID 取消。
- 取消后终止 Git 子进程并返回明确结果。

## 模型调用

- Base URL 默认示例为 `https://api.openai.com/v1`。
- 若 Base URL 未包含 `/chat/completions`，追加该路径。
- 请求使用 `Authorization: Bearer <key>`，没有密钥时不发送该头，以支持无需鉴权的本地服务。
- 默认温度 `0.2`，默认输出上限 `2048` tokens。
- 默认超时 `60` 秒。
- 输入 diff 上限 `60000` 个字符；超限直接报错，不静默截断。
- 最近提交标题最多八条。
- 空响应、非 JSON 响应、缺失 `choices[0].message.content` 和超长响应都作为明确错误。
- 允许 `content` 是字符串或文本片段数组。
- 响应只清理首尾空白和包裹整个响应的 Markdown 代码围栏。

## 错误与安全

错误对象包含稳定错误码、用户可读消息和可选技术细节。

- `GIT_NOT_INSTALLED`
- `NOT_REPOSITORY`
- `INVALID_PATH`
- `NO_STAGED_CHANGES`
- `CONFLICTS_PRESENT`
- `UPSTREAM_MISSING`
- `AUTH_FAILED`
- `NON_FAST_FORWARD`
- `GIT_FAILED`
- `MODEL_NOT_CONFIGURED`
- `MODEL_REQUEST_FAILED`
- `DIFF_TOO_LARGE`
- `SETTINGS_FAILED`
- `OPERATION_CANCELLED`

安全要求：

- 不通过 shell 拼接命令，只使用参数数组。
- 不记录 API Key。
- 不把已保存的 API Key 返回前端。
- Git 技术细节中如出现 URL 凭据，输出前进行遮蔽。
- 文件路径只能相对于已验证的仓库根目录。
- Tauri capability 只开放本应用需要的窗口和命令能力。

## 架构

### Rust

- `commands`：Tauri command 边界和状态注入。
- `git::status`：porcelain v2 解析。
- `git::service`：仓库验证、diff、暂存、提交和同步命令。
- `git::runner`：参数安全、超时、取消和输出规范化。
- `settings`：非敏感设置读取、写入和模型档案校验。
- `secrets`：系统凭据库抽象。
- `ai::prompt`：固定指令、用户规则和上下文组装。
- `ai::client`：OpenAI 兼容 HTTP 调用和响应解析。

### React

- `api`：Tauri command 的类型化调用。
- `App`：仓库、文件、提交和同步状态协调。
- `components`：顶栏、文件列表、差异、提交区和设置弹窗。
- CSS：本地设计令牌和响应式单页布局，不引入大型 UI 框架。

## 验证

### Rust

- 单元测试：porcelain v2 状态、分支状态、重命名、冲突、空格路径、提示词、URL 规范化、响应解析。
- 集成测试：临时真实 Git 仓库上的全部暂存、选择性暂存、取消暂存、首次提交、提交 hook 失败、远端 push/pull 和非快进。
- 配置测试：档案增删改、默认模型回退、单规则持久化、密钥只通过秘密存储访问。
- 模型测试：本地临时 HTTP 服务覆盖成功、鉴权失败、非 JSON、空内容和超大 diff。

### React

- 使用 Vitest 和 Testing Library。
- 测试空仓库、文件选择、全部暂存、生成、手工提交、设置模型、切换模型和错误提示。
- Tauri API 作为外部边界，可以通过注入的客户端替换。

### 交付检查

- `npm test`
- `npm run build`
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `npm run tauri build`
- 手工启动 Windows 构建，截图检查主页面和设置页布局。

macOS 在有构建环境后执行相同验证；Windows 开发不依赖 macOS 专有代码。
