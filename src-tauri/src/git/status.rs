use serde::Serialize;

/// 一个变更路径在索引区和工作区中的状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    /// Git 使用的工作树相对路径。
    pub path: String,
    /// 重命名或复制前的原始路径。
    pub original_path: Option<String>,
    /// 索引区单字符状态。
    pub index_status: String,
    /// 工作区单字符状态。
    pub worktree_status: String,
    /// 索引区是否存在已暂存变化。
    pub staged: bool,
    /// 工作区是否存在未暂存变化。
    pub unstaged: bool,
    /// 该路径是否处于未解决冲突状态。
    pub conflicted: bool,
    /// 前端展示使用的变更分类。
    pub kind: FileChangeKind,
}

/// 前端用于筛选和展示的变更分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
    Unknown,
}

impl FileChangeKind {
    /// 返回前端筛选和测试使用的稳定字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
            Self::Renamed => "renamed",
            Self::Untracked => "untracked",
            Self::Conflicted => "conflicted",
            Self::Unknown => "unknown",
        }
    }
}

/// 从 porcelain 分支头解析出的仓库元数据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryInfo {
    /// 仓库工作树绝对路径。
    pub root: String,
    /// 仓库目录名。
    pub name: String,
    /// 当前分支名或 detached 状态。
    pub branch: String,
    /// 当前分支配置的 upstream。
    pub upstream: Option<String>,
    /// 本地领先 upstream 的提交数。
    pub ahead: u32,
    /// 本地落后 upstream 的提交数。
    pub behind: u32,
    /// 仓库中是否存在未解决冲突。
    pub has_conflicts: bool,
}

/// 前端消费的完整仓库状态快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepositorySnapshot {
    /// 仓库和当前分支元数据。
    pub repository: RepositoryInfo,
    /// 当前工作树变更列表。
    pub changes: Vec<FileChange>,
}

/// 解析 `git status --porcelain=v2 --branch -z` 的原始输出。
pub fn parse_status(input: &[u8]) -> RepositorySnapshot {
    let mut branch = String::new();
    let mut upstream = None;
    let mut ahead = 0;
    let mut behind = 0;
    let mut changes = Vec::new();
    let records: Vec<&[u8]> = input.split(|byte| *byte == 0).collect();
    let mut index = 0;

    while index < records.len() {
        let record = String::from_utf8_lossy(records[index]);
        if record.is_empty() {
            index += 1;
            continue;
        }

        if let Some(header) = record.strip_prefix("# ") {
            if let Some(value) = header.strip_prefix("branch.head ") {
                branch = value.to_string();
            } else if let Some(value) = header.strip_prefix("branch.upstream ") {
                upstream = Some(value.to_string());
            } else if let Some(value) = header.strip_prefix("branch.ab ") {
                let mut values = value.split_whitespace();
                ahead = values
                    .next()
                    .and_then(|part| part.strip_prefix('+'))
                    .and_then(|part| part.parse().ok())
                    .unwrap_or(0);
                behind = values
                    .next()
                    .and_then(|part| part.strip_prefix('-'))
                    .and_then(|part| part.parse().ok())
                    .unwrap_or(0);
            }
            index += 1;
            continue;
        }

        if let Some(path) = record.strip_prefix("? ") {
            changes.push(FileChange {
                path: path.to_string(),
                original_path: None,
                index_status: "?".to_string(),
                worktree_status: "?".to_string(),
                staged: false,
                unstaged: true,
                conflicted: false,
                kind: FileChangeKind::Untracked,
            });
        } else if let Some(path) = record.strip_prefix("! ") {
            changes.push(FileChange {
                path: path.to_string(),
                original_path: None,
                index_status: "!".to_string(),
                worktree_status: "!".to_string(),
                staged: false,
                unstaged: false,
                conflicted: false,
                kind: FileChangeKind::Unknown,
            });
        } else if record.starts_with("1 ") {
            let mut fields = record.splitn(9, ' ');
            let _record_type = fields.next();
            let status = fields.next().unwrap_or("..");
            let _submodule = fields.next();
            for _ in 0..5 {
                let _ = fields.next();
            }
            if let Some(path) = fields.next() {
                changes.push(regular_change(path, status, None));
            }
        } else if record.starts_with("2 ") {
            let mut fields = record.splitn(10, ' ');
            let _record_type = fields.next();
            let status = fields.next().unwrap_or("..");
            for _ in 0..7 {
                let _ = fields.next();
            }
            if let Some(path) = fields.next() {
                let original_path = records
                    .get(index + 1)
                    .map(|value| String::from_utf8_lossy(value).into_owned());
                changes.push(regular_change(path, status, original_path));
                index += 1;
            }
        } else if record.starts_with("u ") {
            let mut fields = record.splitn(11, ' ');
            let _record_type = fields.next();
            let status = fields.next().unwrap_or("UU");
            for _ in 0..8 {
                let _ = fields.next();
            }
            if let Some(path) = fields.next() {
                changes.push(FileChange {
                    path: path.to_string(),
                    original_path: None,
                    index_status: status.chars().next().unwrap_or('U').to_string(),
                    worktree_status: status.chars().nth(1).unwrap_or('U').to_string(),
                    staged: false,
                    unstaged: false,
                    conflicted: true,
                    kind: FileChangeKind::Conflicted,
                });
            }
        }

        index += 1;
    }

    let has_conflicts = changes.iter().any(|change| change.conflicted);
    RepositorySnapshot {
        repository: RepositoryInfo {
            root: String::new(),
            name: String::new(),
            branch,
            upstream,
            ahead,
            behind,
            has_conflicts,
        },
        changes,
    }
}

fn regular_change(path: &str, status: &str, original_path: Option<String>) -> FileChange {
    let index_status = status.chars().next().unwrap_or('.');
    let worktree_status = status.chars().nth(1).unwrap_or('.');
    let staged = index_status != '.';
    let unstaged = worktree_status != '.';
    let kind = if index_status == 'R' || worktree_status == 'R' {
        FileChangeKind::Renamed
    } else if index_status == 'A' || worktree_status == 'A' {
        FileChangeKind::Added
    } else if index_status == 'D' || worktree_status == 'D' {
        FileChangeKind::Deleted
    } else if matches!(index_status, 'M' | 'T') || matches!(worktree_status, 'M' | 'T') {
        FileChangeKind::Modified
    } else {
        FileChangeKind::Unknown
    };

    FileChange {
        path: path.to_string(),
        original_path,
        index_status: index_status.to_string(),
        worktree_status: worktree_status.to_string(),
        staged,
        unstaged,
        conflicted: false,
        kind,
    }
}
