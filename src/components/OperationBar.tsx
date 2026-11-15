export type OperationKind = "commit" | "generate" | "pull" | "push";

export interface PendingOperation {
  id: string;
  kind: OperationKind;
}

interface OperationBarProps {
  canPull: boolean;
  pending: PendingOperation | null;
  confirmUpstream: boolean;
  onPull: () => void;
  onPush: () => void;
  onCancelOperation: () => void;
  onConfirmUpstream: () => void;
  onCancelUpstream: () => void;
}

const operationLabels: Record<OperationKind, string> = {
  commit: "提交",
  generate: "生成提交说明",
  pull: "拉取",
  push: "推送"
};

export function OperationBar({
  canPull,
  pending,
  confirmUpstream,
  onPull,
  onPush,
  onCancelOperation,
  onConfirmUpstream,
  onCancelUpstream
}: OperationBarProps) {
  if (confirmUpstream) {
    return (
      <section className="operation-bar operation-confirm" aria-label="同步操作">
        <p>当前分支还没有 upstream。是否设置 origin 为上游并推送？</p>
        <div className="operation-actions">
          <button
            className="button button-primary"
            type="button"
            onClick={onConfirmUpstream}
          >
            设置 origin 上游并推送
          </button>
          <button
            className="button button-quiet"
            type="button"
            onClick={onCancelUpstream}
          >
            取消
          </button>
        </div>
      </section>
    );
  }

  return (
    <section className="operation-bar" aria-label="同步操作">
      <div className="operation-status" role="status">
        {pending
          ? `正在${operationLabels[pending.kind]}…`
          : "所有 Git 操作仅作用于当前仓库和当前分支"}
      </div>
      <div className="operation-actions">
        {pending ? (
          <button
            className="button button-danger"
            type="button"
            onClick={onCancelOperation}
          >
            取消操作
          </button>
        ) : (
          <>
            <button
              className="button button-quiet"
              type="button"
              onClick={onPull}
              disabled={!canPull}
              title={canPull ? undefined : "当前分支未配置 upstream"}
            >
              Pull
            </button>
            <button
              className="button button-secondary"
              type="button"
              onClick={onPush}
            >
              Push
            </button>
          </>
        )}
      </div>
    </section>
  );
}
