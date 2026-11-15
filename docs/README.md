# SimpleGit 项目文档

这里汇总 SimpleGit 的开发、架构、接口和维护资料。文档以当前源码和配置为事实来源，面向后续开发、测试、发布与审查。

## 文档导航

| 文档 | 适用场景 | 主要内容 |
| --- | --- | --- |
| [README.md](../README.md) | 首次了解或使用项目 | 产品能力、环境要求、安装和使用 |
| [architecture.md](architecture.md) | 理解系统边界或定位模块 | 前后端职责、核心数据流、安全不变量 |
| [api.md](api.md) | 修改 Tauri 命令或类型契约 | 19 个命令的参数、返回值和错误协议 |
| [development.md](development.md) | 本地开发、测试和扩展功能 | 常用命令、测试分层、典型改动路径 |
| [maintenance.md](maintenance.md) | 提交改动或审查文档 | 文档同步责任、更新流程和检查清单 |
| [manual-test.md](manual-test.md) | 查看某次真实环境验证证据 | 2026-11-15 的 Windows 构建与烟测记录 |
| [simplegit-design.md](superpowers/specs/2026-11-15-simplegit-design.md) | 查看首版产品与技术设计 | 范围、交互、默认规则和安全约束 |
| [simplegit-plan.md](superpowers/plans/2026-11-15-simplegit-plan.md) | 追溯首版实施过程 | 历史任务拆分与验收顺序 |

推荐阅读顺序：

1. 新维护者先读根目录 `README.md` 和 [architecture.md](architecture.md)。
2. 修改命令边界前读 [api.md](api.md)。
3. 开始改动前读 [development.md](development.md)。
4. 提交前按 [maintenance.md](maintenance.md) 同步文档并完成针对性验证。

## 事实优先级

发生冲突时按以下顺序判断当前行为：

1. 当前源码、`package.json`、`Cargo.toml`、Tauri 配置和实际命令输出。
2. `architecture.md`、`api.md`、`development.md` 等维护文档。
3. 根目录 `README.md`。
4. `manual-test.md` 中带日期的验证快照。
5. `superpowers/specs/` 和 `superpowers/plans/` 中的设计或历史实施记录。

设计规格说明“为什么这样做”，实施计划说明“当时如何完成”；两者都不应被当作未经核验的当前行为清单。

## 同步原则

- 行为、命令、数据结构、环境要求或发布流程发生变化时，在同一个改动中更新对应文档。
- 优先维护一个权威说明，其他文档使用链接引用，避免在多处复制同一组细节。
- 命令、路径、版本和测试结果必须来自可执行文件或实际输出，不凭记忆填写。
- 日期化验证记录描述当时的证据；后续重新验证时新增记录或更新“核对基准”，不伪装成未发生变化。
- 文档中的示例应能直接用于当前项目，不保留已删除的脚本、参数或界面流程。
