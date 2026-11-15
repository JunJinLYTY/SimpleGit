import { useEffect, useMemo, useState } from "react";
import type {
  AppSettings,
  Diagnostics,
  ModelProfileInput,
  SimpleGitClient
} from "../types";
import { ModelProfileForm } from "./ModelProfileForm";

interface SettingsDialogProps {
  client: SimpleGitClient;
  settings: AppSettings;
  onSettingsChange: (settings: AppSettings) => void;
  onClose: () => void;
}

export function SettingsDialog({
  client,
  settings,
  onSettingsChange,
  onClose
}: SettingsDialogProps) {
  const [selectedId, setSelectedId] = useState<string | null>(
    settings.activeModelId ?? settings.models[0]?.id ?? null
  );
  const [rule, setRule] = useState(settings.commitRule);
  const [busy, setBusy] = useState(false);
  const [formDirty, setFormDirty] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [diagnostics, setDiagnostics] = useState<Diagnostics | null>(null);
  const [confirmClose, setConfirmClose] = useState(false);

  const selectedProfile = useMemo(
    () => settings.models.find((profile) => profile.id === selectedId) ?? null,
    [selectedId, settings.models]
  );
  const ruleDirty = rule !== settings.commitRule;
  const dirty = formDirty || ruleDirty;

  useEffect(() => {
    let active = true;
    void client
      .diagnostics()
      .then((value) => {
        if (active) {
          setDiagnostics(value);
        }
      })
      .catch(() => {
        if (active) {
          setDiagnostics(null);
        }
      });
    return () => {
      active = false;
    };
  }, [client]);

  useEffect(() => {
    if (!ruleDirty) {
      setRule(settings.commitRule);
    }
  }, [ruleDirty, settings.commitRule]);

  async function saveModel(input: ModelProfileInput) {
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const next = await client.saveModelProfile(input);
      const chosen =
        input.id ??
        next.models.find(
          (profile) =>
            profile.name === input.name && profile.model === input.model
        )?.id ??
        next.activeModelId ??
        next.models[0]?.id ??
        null;
      setSelectedId(chosen);
      onSettingsChange(next);
      setStatus("模型配置已保存");
    } catch (saveError) {
      setError(errorMessage(saveError));
      throw saveError;
    } finally {
      setBusy(false);
    }
  }

  async function testProfile() {
    if (!selectedProfile) {
      return;
    }
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      await client.testModelProfile(selectedProfile.id);
      setStatus("连接成功");
    } catch (testError) {
      setError(errorMessage(testError));
    } finally {
      setBusy(false);
    }
  }

  async function deleteProfile() {
    if (!selectedProfile) {
      return;
    }
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const next = await client.deleteModelProfile(selectedProfile.id);
      onSettingsChange(next);
      setSelectedId(
        next.activeModelId ??
          next.models.find((profile) => profile.isDefault)?.id ??
          next.models[0]?.id ??
          null
      );
      setStatus("模型配置已删除");
    } catch (deleteError) {
      setError(errorMessage(deleteError));
    } finally {
      setBusy(false);
    }
  }

  async function saveRule() {
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const next = await client.saveCommitRule(rule);
      onSettingsChange(next);
      setRule(next.commitRule);
      setStatus("提交规则已保存");
    } catch (ruleError) {
      setError(errorMessage(ruleError));
    } finally {
      setBusy(false);
    }
  }

  function requestClose() {
    if (dirty) {
      setConfirmClose(true);
    } else {
      onClose();
    }
  }

  function addProfile() {
    setSelectedId(null);
    setStatus(null);
    setError(null);
  }

  return (
    <div className="dialog-backdrop">
      <section
        className="dialog-panel settings-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="设置"
      >
        <header className="dialog-heading">
          <div>
            <h2>设置</h2>
            <p>管理模型接入、系统密钥和唯一一套提交规则。</p>
          </div>
          <button
            className="button button-quiet"
            type="button"
            aria-label="关闭设置"
            onClick={requestClose}
          >
            关闭
          </button>
        </header>

        {(error || status) && (
          <div
            className={error ? "settings-status status-error" : "settings-status"}
            role={error ? "alert" : "status"}
          >
            {error ?? status}
          </div>
        )}

        <div className="settings-layout">
          <aside className="profile-list" aria-label="模型档案列表">
            <div className="profile-list-heading">
              <h3>模型配置</h3>
              <button
                className="button button-quiet"
                type="button"
                onClick={addProfile}
              >
                新增模型
              </button>
            </div>
            <div className="profile-list-items">
              {settings.models.map((profile) => (
                <button
                  className={
                    profile.id === selectedId
                      ? "profile-list-item is-selected"
                      : "profile-list-item"
                  }
                  type="button"
                  key={profile.id}
                  aria-label={`编辑 ${profile.name}`}
                  onClick={() => setSelectedId(profile.id)}
                >
                  <span>
                    <strong>{profile.name}</strong>
                    <small>{profile.model}</small>
                  </span>
                  <span className="profile-badges">
                    {profile.isDefault && <em>默认</em>}
                    {profile.hasApiKey && <em>已配置</em>}
                  </span>
                </button>
              ))}
              {settings.models.length === 0 && (
                <p className="profile-list-empty">尚未配置模型</p>
              )}
            </div>
          </aside>

          <div className="settings-main">
            <ModelProfileForm
              profile={selectedProfile}
              busy={busy}
              onSave={saveModel}
              onTest={() => void testProfile()}
              onDelete={() => void deleteProfile()}
              onDirtyChange={setFormDirty}
            />

            <section className="rule-editor" aria-label="提交规则">
              <div className="rule-editor-heading">
                <div>
                  <h3>全局提交规则</h3>
                  <p>所有模型共用这一套规范，留空时由后端使用默认规范。</p>
                </div>
                <button
                  className="button button-secondary"
                  type="button"
                  onClick={() => void saveRule()}
                  disabled={busy}
                >
                  保存规则
                </button>
              </div>
              <label htmlFor="commit-rule">全局提交规则</label>
              <textarea
                id="commit-rule"
                aria-label="全局提交规则"
                value={rule}
                rows={7}
                onChange={(event) => setRule(event.target.value)}
              />
            </section>

            <section className="diagnostics" aria-label="诊断信息">
              <h3>诊断</h3>
              {diagnostics ? (
                <dl>
                  <div>
                    <dt>Git</dt>
                    <dd>{diagnostics.gitVersion}</dd>
                  </div>
                  <div>
                    <dt>路径</dt>
                    <dd>{diagnostics.gitPath}</dd>
                  </div>
                  <div>
                    <dt>SimpleGit</dt>
                    <dd>{diagnostics.appVersion}</dd>
                  </div>
                </dl>
              ) : (
                <p>正在读取 Git 与版本信息…</p>
              )}
            </section>
          </div>
        </div>
      </section>

      {confirmClose && (
        <div className="dialog-backdrop confirm-layer">
          <section
            className="confirm-dialog"
            role="alertdialog"
            aria-modal="true"
            aria-label="确认关闭设置"
          >
            <h3>有未保存的修改</h3>
            <p>关闭后，当前表单和规则修改会丢失。</p>
            <div className="confirm-actions">
              <button
                className="button button-danger"
                type="button"
                onClick={onClose}
              >
                放弃修改
              </button>
              <button
                className="button button-secondary"
                type="button"
                onClick={() => setConfirmClose(false)}
              >
                继续编辑
              </button>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string") {
      return message;
    }
  }
  return "设置操作失败";
}
