import type { FileChange, RepositorySnapshot } from "../types";

interface ChangesPanelProps {
  snapshot: RepositorySnapshot;
  selectedUnstaged: Set<string>;
  selectedStaged: Set<string>;
  loading: boolean;
  onToggleUnstaged: (path: string) => void;
  onToggleStaged: (path: string) => void;
  onStage: () => void;
  onUnstage: () => void;
  onOpenDiff: (path: string, staged: boolean) => void;
}

const kindLabels: Record<FileChange["kind"], string> = {
  added: "新增",
  modified: "修改",
  deleted: "删除",
  renamed: "重命名",
  untracked: "未跟踪",
  conflicted: "冲突",
  unknown: "变更"
};

export function ChangesPanel({
  snapshot,
  selectedUnstaged,
  selectedStaged,
  loading,
  onToggleUnstaged,
  onToggleStaged,
  onStage,
  onUnstage,
  onOpenDiff
}: ChangesPanelProps) {
  const unstaged = snapshot.changes.filter(
    (change) => change.unstaged || change.kind === "untracked" || change.conflicted
  );
  const staged = snapshot.changes.filter((change) => change.staged);

  const renderRows = (changes: FileChange[], isStaged: boolean) => {
    if (changes.length === 0) {
      return (
        <li className="empty-row">
          {isStaged ? "暂无已暂存文件" : "工作区干净"}
        </li>
      );
    }
    return changes.map((change) => {
      const selected = isStaged
        ? selectedStaged.has(change.path)
        : selectedUnstaged.has(change.path);
      return (
        <li
          className="file-row"
          key={`${isStaged ? "staged" : "unstaged"}:${change.path}`}
        >
          <div className="file-summary">
            <input
              aria-label={
                isStaged
                  ? `选择已暂存 ${change.path}`
                  : `选择 ${change.path}`
              }
              checked={selected}
              disabled={change.conflicted && !isStaged}
              type="checkbox"
              onChange={() =>
                isStaged
                  ? onToggleStaged(change.path)
                  : onToggleUnstaged(change.path)
              }
            />
            <span className={`change-kind kind-${change.kind}`}>
              {kindLabels[change.kind]}
            </span>
            <span className="file-path" title={change.originalPath ?? change.path}>
              {change.path}
            </span>
            <button
              className="diff-toggle"
              type="button"
              aria-haspopup="dialog"
              aria-label={`查看 ${change.path} 的${
                isStaged ? "已暂存" : "未暂存"
              } diff`}
              onClick={() => onOpenDiff(change.path, isStaged)}
              disabled={change.conflicted}
            >
              Diff
            </button>
          </div>
        </li>
      );
    });
  };

  const selectedUnstagedCount = selectedUnstaged.size;
  const stageLabel = selectedUnstagedCount
    ? `暂存选中的 ${selectedUnstagedCount} 个文件`
    : "暂存全部更改";

  return (
    <section className="changes-panel" aria-label="仓库更改">
      <div className="panel-toolbar">
        <div>
          <h2>更改</h2>
          <p>选择文件后可单独暂存，未选择时 Add 暂存全部。</p>
        </div>
        <div className="toolbar-actions">
          <button
            className="button button-primary"
            type="button"
            aria-label={stageLabel}
            disabled={loading || unstaged.length === 0}
            onClick={onStage}
          >
            {selectedUnstagedCount ? `Add ${selectedUnstagedCount}` : "Add"}
          </button>
          <button
            className="button button-quiet"
            type="button"
            disabled={loading || selectedStaged.size === 0}
            onClick={onUnstage}
          >
            移出暂存
          </button>
        </div>
      </div>

      {snapshot.repository.hasConflicts && (
        <div className="conflict-banner" role="note">
          存在未解决冲突，请先在编辑器中处理后再继续。
        </div>
      )}

      <div className="change-groups">
        <section className="change-group">
          <header>
            <h3>未暂存</h3>
            <span>{unstaged.length}</span>
          </header>
          <ul>{renderRows(unstaged, false)}</ul>
        </section>
        <section className="change-group">
          <header>
            <h3>已暂存</h3>
            <span>{staged.length}</span>
          </header>
          <ul>{renderRows(staged, true)}</ul>
        </section>
      </div>
    </section>
  );
}
