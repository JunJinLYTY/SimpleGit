import { useEffect, useRef } from "react";
import { DiffView } from "./DiffView";

interface DiffPreview {
  /** 文件路径与暂存状态组成的预览标识。 */
  key: string;
  /** 相对于仓库根目录的文件路径。 */
  path: string;
  /** 当前预览读取的是否为已暂存版本。 */
  staged: boolean;
  /** Git 返回的原始统一 diff 文本。 */
  diff: string;
  /** 是否仍在读取 diff。 */
  loading: boolean;
  /** 读取 diff 失败时展示的可读错误。 */
  error?: string;
}

interface DiffDialogProps {
  /** 当前要展示的 diff 及其读取状态。 */
  preview: DiffPreview;
  /** 请求关闭预览窗口。 */
  onClose: () => void;
}

/** 以模态窗口展示单个文件的完整 diff，避免挤压更改列表。 */
export function DiffDialog({ preview, onClose }: DiffDialogProps) {
  const closeButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    closeButtonRef.current?.focus();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  return (
    <div
      className="dialog-backdrop diff-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
    >
      <section
        className="dialog-panel diff-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="diff-dialog-title"
      >
        <header className="diff-dialog-heading">
          <div>
            <h2 id="diff-dialog-title">Diff 预览</h2>
            <p>此窗口只展示差异，不会修改暂存区或工作区。</p>
          </div>
          <button
            ref={closeButtonRef}
            className="button button-secondary"
            type="button"
            onClick={onClose}
          >
            关闭
          </button>
        </header>

        <div className="diff-dialog-meta">
          <span className={preview.staged ? "diff-state is-staged" : "diff-state"}>
            {preview.staged ? "已暂存" : "未暂存"}
          </span>
          <code title={preview.path}>{preview.path}</code>
        </div>

        <div className="diff-dialog-body">
          <DiffView
            diff={preview.diff}
            loading={preview.loading}
            error={preview.error}
          />
        </div>
      </section>
    </div>
  );
}
