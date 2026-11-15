import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { tauriClient } from "./api";
import { ChangesPanel } from "./components/ChangesPanel";
import { CommitComposer } from "./components/CommitComposer";
import { DiffDialog } from "./components/DiffDialog";
import { OperationBar, type PendingOperation } from "./components/OperationBar";
import { RepositoryHeader } from "./components/RepositoryHeader";
import { SettingsDialog } from "./components/SettingsDialog";
import type {
  AppSettings,
  RepositorySnapshot,
  SimpleGitClient
} from "./types";

interface AppProps {
  client?: SimpleGitClient;
  pickRepository?: () => Promise<string | null>;
}

interface UiError {
  code: string;
  message: string;
  details?: string;
}

/** 独立 Diff 预览窗口需要展示的文件与读取状态。 */
interface DiffPreview {
  /** 区分同一路径在已暂存和未暂存版本中的唯一标识。 */
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

async function pickRepositoryDirectory(): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择 Git 仓库"
  });
  return typeof selected === "string" ? selected : null;
}

export default function App({
  client = tauriClient,
  pickRepository = pickRepositoryDirectory
}: AppProps) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [activeModelId, setActiveModelId] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<RepositorySnapshot | null>(null);
  const [selectedUnstaged, setSelectedUnstaged] = useState<Set<string>>(
    new Set()
  );
  const [selectedStaged, setSelectedStaged] = useState<Set<string>>(new Set());
  const [diffPreview, setDiffPreview] = useState<DiffPreview | null>(null);
  const [message, setMessage] = useState("");
  const [pending, setPending] = useState<PendingOperation | null>(null);
  const [generating, setGenerating] = useState(false);
  const [busy, setBusy] = useState(false);
  const [confirmUpstream, setConfirmUpstream] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void client
      .loadSettings()
      .then((loaded) => {
        if (!active) {
          return;
        }
        setSettings(loaded);
        setActiveModelId(selectActiveModelId(loaded));
      })
      .catch((loadError) => {
        if (active) {
          setError(toUiError(loadError));
        }
      });
    return () => {
      active = false;
    };
  }, [client]);

  const models = settings?.models ?? [];
  const activeModel = useMemo(
    () =>
      models.find((profile) => profile.id === activeModelId) ??
      models.find((profile) => profile.isDefault) ??
      models[0] ??
      null,
    [activeModelId, models]
  );
  const hasStagedChanges = Boolean(
    snapshot?.changes.some((change) => change.staged)
  );
  const hasConflicts = snapshot?.repository.hasConflicts ?? false;
  const repositoryRoot = snapshot?.repository.root ?? null;

  useEffect(() => {
    if (!repositoryRoot) {
      return;
    }
    const refreshOnFocus = () => {
      if (pending) {
        return;
      }
      void client
        .repositoryStatus(repositoryRoot)
        .then(setSnapshot)
        .catch((refreshError) => setError(toUiError(refreshError)));
    };
    window.addEventListener("focus", refreshOnFocus);
    return () => window.removeEventListener("focus", refreshOnFocus);
  }, [client, pending, repositoryRoot]);

  async function handleOpenRepository() {
    let path: string | null = null;
    try {
      path = await pickRepository();
    } catch (pickError) {
      setError(toUiError(pickError));
      return;
    }
    if (path) {
      await openRepositoryPath(path);
    }
  }

  async function openRepositoryPath(path: string) {
    setError(null);
    setNotice(null);
    setBusy(true);
    try {
      const next = await client.openRepository(path);
      setSnapshot(next);
      resetRepositoryUi();
      try {
        const nextSettings = await client.recordRecentRepository(
          next.repository.root
        );
        setSettings(nextSettings);
        setActiveModelId(selectActiveModelId(nextSettings));
      } catch (recentError) {
        setError(toUiError(recentError));
      }
    } catch (openError) {
      setError(toUiError(openError));
    } finally {
      setBusy(false);
    }
  }

  async function refreshStatus(root = repositoryRoot) {
    if (!root) {
      return;
    }
    const next = await client.repositoryStatus(root);
    setSnapshot(next);
  }

  async function handleRefresh() {
    if (!repositoryRoot) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await refreshStatus(repositoryRoot);
    } catch (refreshError) {
      setError(toUiError(refreshError));
    } finally {
      setBusy(false);
    }
  }

  async function handleStage() {
    if (!repositoryRoot) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const selected = [...selectedUnstaged];
      const next = selected.length
        ? await client.stageSelected(repositoryRoot, selected)
        : await client.stageAll(repositoryRoot);
      setSnapshot(next);
      clearSelections();
      setDiffPreview(null);
    } catch (stageError) {
      setError(toUiError(stageError));
    } finally {
      setBusy(false);
    }
  }

  async function handleUnstage() {
    if (!repositoryRoot || selectedStaged.size === 0) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const next = await client.unstageSelected(repositoryRoot, [
        ...selectedStaged
      ]);
      setSnapshot(next);
      clearSelections();
      setDiffPreview(null);
    } catch (unstageError) {
      setError(toUiError(unstageError));
    } finally {
      setBusy(false);
    }
  }

  /** 在独立对话框中加载并展示指定文件的 diff。 */
  async function handleOpenDiff(path: string, staged: boolean) {
    if (!repositoryRoot) {
      return;
    }
    const key = `${staged ? "staged" : "unstaged"}:${path}`;
    setDiffPreview({
      key,
      path,
      staged,
      diff: "",
      loading: true
    });
    try {
      const diff = await client.readDiff(repositoryRoot, path, staged);
      setDiffPreview((current) =>
        current?.key === key
          ? { ...current, diff, loading: false, error: undefined }
          : current
      );
    } catch (diffError) {
      setDiffPreview((current) =>
        current?.key === key
          ? {
              ...current,
              diff: "",
              loading: false,
              error: errorText(diffError)
            }
          : current
      );
    }
  }

  /** 关闭 diff 预览，不影响仓库或暂存状态。 */
  const closeDiffPreview = useCallback(() => {
    setDiffPreview(null);
  }, []);

  async function handleGenerate(): Promise<boolean> {
    if (!repositoryRoot || !activeModel) {
      setError({
        code: "MODEL_NOT_CONFIGURED",
        message: "请先在设置中配置并选择一个模型"
      });
      return false;
    }
    if (!hasStagedChanges || hasConflicts) {
      return false;
    }
    setGenerating(true);
    setError(null);
    setNotice(null);
    const operationId = createOperationId();
    setPending({ id: operationId, kind: "generate" });
    try {
      const generated = await client.generateCommit(
        repositoryRoot,
        activeModel.id,
        operationId
      );
      setMessage(generated);
      setNotice("提交说明已生成，请检查后点击 Commit。");
      return true;
    } catch (generateError) {
      if (isCancelled(generateError)) {
        setNotice("模型生成已取消。");
      } else {
        setError(toUiError(generateError));
      }
      return false;
    } finally {
      setGenerating(false);
      setPending(null);
    }
  }

  async function handleCommit() {
    if (!repositoryRoot || !hasStagedChanges || hasConflicts) {
      return;
    }
    if (!message.trim()) {
      return;
    }

    const operationId = createOperationId();
    setPending({ id: operationId, kind: "commit" });
    setError(null);
    setNotice(null);
    try {
      const result = await client.commit(
        repositoryRoot,
        message,
        operationId
      );
      setMessage("");
      clearSelections();
      setDiffPreview(null);
      await refreshStatus(repositoryRoot);
      setNotice(`已提交：${result.summary}`);
    } catch (commitError) {
      if (isCancelled(commitError)) {
        setNotice("提交已取消，暂存内容已保留。");
      } else {
        setError(toUiError(commitError));
      }
    } finally {
      setPending(null);
    }
  }

  async function runPull() {
    if (!repositoryRoot || !snapshot?.repository.upstream) {
      return;
    }
    const operationId = createOperationId();
    setPending({ id: operationId, kind: "pull" });
    setError(null);
    setNotice(null);
    try {
      const result = await client.pull(repositoryRoot, operationId);
      await refreshStatus(repositoryRoot);
      setNotice(result.output || "拉取完成。");
    } catch (pullError) {
      if (isCancelled(pullError)) {
        setNotice("拉取已取消。");
      } else {
        setError(toUiError(pullError));
      }
    } finally {
      setPending(null);
    }
  }

  async function runPush(setUpstream: boolean) {
    if (!repositoryRoot) {
      return;
    }
    const operationId = createOperationId();
    setConfirmUpstream(false);
    setPending({ id: operationId, kind: "push" });
    setError(null);
    setNotice(null);
    try {
      const result = await client.push(
        repositoryRoot,
        setUpstream,
        operationId
      );
      await refreshStatus(repositoryRoot);
      setNotice(result.output || "推送完成。");
    } catch (pushError) {
      if (isCancelled(pushError)) {
        setNotice("推送已取消。");
      } else {
        setError(toUiError(pushError));
      }
    } finally {
      setPending(null);
    }
  }

  async function handleCancelOperation() {
    if (!pending) {
      return;
    }
    try {
      await client.cancelOperation(pending.id);
      setNotice("正在取消操作…");
    } catch (cancelError) {
      setError(toUiError(cancelError));
    }
  }

  async function handleModelChange(profileId: string) {
    setActiveModelId(profileId);
    try {
      const next = await client.setActiveModel(profileId);
      setSettings(next);
    } catch (modelError) {
      setError(toUiError(modelError));
    }
  }

  function handlePush() {
    if (!snapshot?.repository.upstream) {
      setConfirmUpstream(true);
      return;
    }
    void runPush(false);
  }

  function resetRepositoryUi() {
    clearSelections();
    setDiffPreview(null);
    setMessage("");
    setPending(null);
    setConfirmUpstream(false);
    setNotice(null);
  }

  function clearSelections() {
    setSelectedUnstaged(new Set());
    setSelectedStaged(new Set());
  }

  return (
    <div className="app-shell">
      <RepositoryHeader
        repository={snapshot?.repository ?? null}
        recentRepositories={settings?.recentRepositories ?? []}
        loading={busy || Boolean(pending)}
        onOpenRepository={handleOpenRepository}
        onOpenRecentRepository={(path) => void openRepositoryPath(path)}
        onRefresh={handleRefresh}
        onOpenSettings={() => setSettingsOpen(true)}
      />

      <div className="notification-layer">
        {error && (
          <div className="top-banner top-banner-error" role="alert">
            <span className="banner-icon" aria-hidden="true">
              !
            </span>
            <div className="banner-copy">
              <strong>{error.message}</strong>
              {error.details && <pre>{error.details}</pre>}
            </div>
            <button
              className="banner-close"
              type="button"
              aria-label="关闭错误提示"
              onClick={() => setError(null)}
            >
              ×
            </button>
          </div>
        )}
        {notice && (
          <div className="top-banner top-banner-notice" role="status">
            <span className="banner-icon" aria-hidden="true">
              ✓
            </span>
            <div className="banner-copy">
              <strong>{notice}</strong>
            </div>
            <button
              className="banner-close"
              type="button"
              aria-label="关闭操作提示"
              onClick={() => setNotice(null)}
            >
              ×
            </button>
          </div>
        )}
      </div>

      <main className="workspace">
        {snapshot ? (
          <>
            <ChangesPanel
              snapshot={snapshot}
              selectedUnstaged={selectedUnstaged}
              selectedStaged={selectedStaged}
              loading={busy || Boolean(pending)}
              onToggleUnstaged={(path) =>
                setSelectedUnstaged((current) => togglePath(current, path))
              }
              onToggleStaged={(path) =>
                setSelectedStaged((current) => togglePath(current, path))
              }
              onStage={handleStage}
              onUnstage={handleUnstage}
              onOpenDiff={handleOpenDiff}
            />

            <div className="commit-dock">
              <CommitComposer
                models={models}
                activeModelId={activeModel?.id ?? null}
                message={message}
                hasStagedChanges={hasStagedChanges}
                hasConflicts={hasConflicts}
                generating={generating}
                committing={pending?.kind === "commit"}
                onModelChange={handleModelChange}
                onMessageChange={setMessage}
                onGenerate={() => void handleGenerate()}
                onCommit={() => void handleCommit()}
                onConfigureModel={() => setSettingsOpen(true)}
              />
              <OperationBar
                canPull={Boolean(snapshot.repository.upstream)}
                pending={pending}
                confirmUpstream={confirmUpstream}
                onPull={() => void runPull()}
                onPush={handlePush}
                onCancelOperation={() => void handleCancelOperation()}
                onConfirmUpstream={() => void runPush(true)}
                onCancelUpstream={() => setConfirmUpstream(false)}
              />
            </div>
          </>
        ) : (
          <section className="empty-workspace">
            <div className="empty-mark" aria-hidden="true">
              +/
            </div>
            <h2>打开一个已有 Git 仓库</h2>
            <p>查看更改、选择性暂存、生成提交说明，然后提交并同步。</p>
            <p className="empty-hint">使用右上角的“打开仓库”开始。</p>
          </section>
        )}
      </main>

      {diffPreview && (
        <DiffDialog preview={diffPreview} onClose={closeDiffPreview} />
      )}

      {settingsOpen && settings && (
        <SettingsDialog
          client={client}
          settings={settings}
          onSettingsChange={(next) => {
            setSettings(next);
            setActiveModelId(selectActiveModelId(next));
          }}
          onClose={() => setSettingsOpen(false)}
        />
      )}
    </div>
  );
}

function togglePath(current: Set<string>, path: string): Set<string> {
  const next = new Set(current);
  if (next.has(path)) {
    next.delete(path);
  } else {
    next.add(path);
  }
  return next;
}

function selectActiveModelId(settings: AppSettings): string | null {
  return (
    settings.activeModelId ??
    settings.models.find((profile) => profile.isDefault)?.id ??
    settings.models[0]?.id ??
    null
  );
}

function createOperationId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function toUiError(error: unknown): UiError {
  if (error && typeof error === "object") {
    const candidate = error as Partial<UiError>;
    return {
      code:
        typeof candidate.code === "string" ? candidate.code : "UNKNOWN",
      message:
        typeof candidate.message === "string"
          ? candidate.message
          : "操作失败",
      details:
        typeof candidate.details === "string" ? candidate.details : undefined
    };
  }
  return {
    code: "UNKNOWN",
    message: errorText(error)
  };
}

function errorText(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return typeof error === "string" ? error : "未知错误";
}

function isCancelled(error: unknown): boolean {
  return (
    Boolean(error) &&
    typeof error === "object" &&
    (error as { code?: string }).code === "OPERATION_CANCELLED"
  );
}
