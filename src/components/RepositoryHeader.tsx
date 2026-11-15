import type { RepositoryInfo } from "../types";

interface RepositoryHeaderProps {
  repository: RepositoryInfo | null;
  recentRepositories: string[];
  loading: boolean;
  onOpenRepository: () => void;
  onOpenRecentRepository: (path: string) => void;
  onRefresh: () => void;
  onOpenSettings: () => void;
}

export function RepositoryHeader({
  repository,
  recentRepositories,
  loading,
  onOpenRepository,
  onOpenRecentRepository,
  onRefresh,
  onOpenSettings
}: RepositoryHeaderProps) {
  return (
    <header className="repository-header">
      <div className="brand-block">
        <h1>SimpleGit</h1>
        <span>专注提交，不打断思路</span>
      </div>

      {repository ? (
        <div className="repository-identity">
          <strong className="repository-name">{repository.name}</strong>
          <span className="repository-path" title={repository.root}>
            {repository.root}
          </span>
        </div>
      ) : (
        <p className="repository-placeholder">尚未打开仓库</p>
      )}

      <div className="header-actions">
        {repository && (
          <>
            <span className="branch-pill">{repository.branch}</span>
            <span className="sync-pill">领先 {repository.ahead}</span>
            <span className="sync-pill">落后 {repository.behind}</span>
            <button
              className="button button-quiet"
              type="button"
              onClick={onRefresh}
              disabled={loading}
            >
              刷新
            </button>
          </>
        )}
        {recentRepositories.length > 0 && (
          <select
            className="recent-repository-select"
            aria-label="最近仓库"
            value=""
            onChange={(event) => {
              if (event.target.value) {
                onOpenRecentRepository(event.target.value);
              }
            }}
            disabled={loading}
          >
            <option value="">最近仓库</option>
            {recentRepositories.map((path) => (
              <option key={path} value={path}>
                {path}
              </option>
            ))}
          </select>
        )}
        <button
          className="button button-secondary"
          type="button"
          onClick={onOpenRepository}
          disabled={loading}
        >
          打开仓库
        </button>
        <button
          className="button button-quiet"
          type="button"
          onClick={onOpenSettings}
        >
          设置
        </button>
      </div>
    </header>
  );
}
