import type { ModelProfile } from "../types";

interface CommitComposerProps {
  models: ModelProfile[];
  activeModelId: string | null;
  message: string;
  hasStagedChanges: boolean;
  hasConflicts: boolean;
  generating: boolean;
  committing: boolean;
  onModelChange: (profileId: string) => void;
  onMessageChange: (message: string) => void;
  onGenerate: () => void;
  onCommit: () => void;
  onConfigureModel: () => void;
}

export function CommitComposer({
  models,
  activeModelId,
  message,
  hasStagedChanges,
  hasConflicts,
  generating,
  committing,
  onModelChange,
  onMessageChange,
  onGenerate,
  onCommit,
  onConfigureModel
}: CommitComposerProps) {
  const canUseGeneration =
    hasStagedChanges && !hasConflicts && models.length > 0 && !committing;

  return (
    <section className="commit-composer" aria-label="提交">
      <div className="composer-heading">
        <div>
          <h2>提交说明</h2>
          <p>请先填写提交说明；内容为空时无法提交。</p>
        </div>
        <div className="model-selector">
          <label htmlFor="model-profile">模型配置</label>
          {models.length > 0 ? (
            <select
              id="model-profile"
              value={activeModelId ?? models[0].id}
              onChange={(event) => onModelChange(event.target.value)}
              disabled={generating || committing}
            >
              {models.map((profile) => (
                <option key={profile.id} value={profile.id}>
                  {profile.name}
                </option>
              ))}
            </select>
          ) : (
            <button
              className="button button-quiet"
              type="button"
              onClick={onConfigureModel}
            >
              配置模型
            </button>
          )}
        </div>
      </div>

      {hasConflicts && (
        <p className="composer-warning">
          存在未解决冲突，暂存、生成和提交已禁用。
        </p>
      )}

      <label className="message-label" htmlFor="commit-message">
        提交说明
      </label>
      <textarea
        id="commit-message"
        aria-label="提交说明"
        value={message}
        rows={5}
        placeholder="输入提交说明，或点击 AI 生成"
        onChange={(event) => onMessageChange(event.target.value)}
        disabled={committing}
      />

      <div className="composer-actions">
        <button
          className="button button-secondary"
          type="button"
          onClick={onGenerate}
          disabled={!canUseGeneration || generating}
        >
          {generating ? "生成中…" : "AI 生成"}
        </button>
        <button
          className="button button-commit"
          type="button"
          onClick={onCommit}
          disabled={
            !hasStagedChanges ||
            hasConflicts ||
            !message.trim() ||
            committing ||
            generating
          }
        >
          {committing ? "提交中…" : "Commit"}
        </button>
      </div>
    </section>
  );
}
