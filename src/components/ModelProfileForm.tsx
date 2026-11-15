import { useEffect, useState } from "react";
import type {
  ModelProfile,
  ModelProfileInput
} from "../types";

interface ModelProfileFormProps {
  profile: ModelProfile | null;
  busy: boolean;
  onSave: (input: ModelProfileInput) => Promise<void>;
  onTest: () => void;
  onDelete: () => void;
  onDirtyChange: (dirty: boolean) => void;
}

export function ModelProfileForm({
  profile,
  busy,
  onSave,
  onTest,
  onDelete,
  onDirtyChange
}: ModelProfileFormProps) {
  const [name, setName] = useState("");
  const [baseUrl, setBaseUrl] = useState("https://api.openai.com/v1");
  const [model, setModel] = useState("");
  const [timeoutSeconds, setTimeoutSeconds] = useState("60");
  const [isDefault, setIsDefault] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setName(profile?.name ?? "");
    setBaseUrl(profile?.baseUrl ?? "");
    setModel(profile?.model ?? "");
    setTimeoutSeconds(String(profile?.timeoutSeconds ?? 60));
    setIsDefault(profile?.isDefault ?? false);
    setApiKey("");
    setError(null);
    onDirtyChange(false);
  }, [profile?.id, onDirtyChange]);

  function markDirty() {
    onDirtyChange(true);
    setError(null);
  }

  async function handleSave() {
    const trimmedName = name.trim();
    const trimmedModel = model.trim();
    const trimmedUrl = baseUrl.trim();
    const parsedTimeout = Number(timeoutSeconds);
    if (!trimmedName) {
      setError("配置名称不能为空");
      return;
    }
    if (!trimmedUrl.startsWith("http://") && !trimmedUrl.startsWith("https://")) {
      setError("Base URL 必须以 http:// 或 https:// 开头");
      return;
    }
    if (!trimmedModel) {
      setError("Model ID 不能为空");
      return;
    }
    if (!Number.isFinite(parsedTimeout) || parsedTimeout < 1) {
      setError("超时时间必须大于 0 秒");
      return;
    }

    const input: ModelProfileInput = {
      id: profile?.id ?? null,
      name: trimmedName,
      baseUrl: trimmedUrl,
      model: trimmedModel,
      timeoutSeconds: parsedTimeout,
      isDefault,
      clearApiKey: false,
      ...(apiKey.trim() ? { apiKey: apiKey.trim() } : {})
    };
    try {
      await onSave(input);
      setApiKey("");
      onDirtyChange(false);
      setError(null);
    } catch (saveError) {
      setError(errorMessage(saveError));
    }
  }

  return (
    <section className="profile-form" aria-label="模型配置表单">
      <div className="profile-form-heading">
        <div>
          <h3>{profile ? profile.name : "新增模型"}</h3>
          <p>
            {profile?.hasApiKey
              ? "已配置"
              : profile
                ? "未配置密钥"
                : "支持 OpenAI Chat Completions 兼容接口"}
          </p>
        </div>
        {profile && <span className="default-badge">{profile.isDefault ? "默认" : "可选"}</span>}
      </div>

      <div className="form-grid">
        <label>
          配置名称
          <input
            value={name}
            onChange={(event) => {
              setName(event.target.value);
              markDirty();
            }}
            placeholder="例如：本地 Qwen"
          />
        </label>
        <label>
          Base URL
          <input
            value={baseUrl}
            onChange={(event) => {
              setBaseUrl(event.target.value);
              markDirty();
            }}
            placeholder="https://api.example.com/v1"
            spellCheck={false}
          />
        </label>
        <label>
          Model ID
          <input
            value={model}
            onChange={(event) => {
              setModel(event.target.value);
              markDirty();
            }}
            placeholder="qwen3-coder"
            spellCheck={false}
          />
        </label>
        <label>
          超时秒数
          <input
            min={1}
            max={600}
            type="number"
            value={timeoutSeconds}
            onChange={(event) => {
              setTimeoutSeconds(event.target.value);
              markDirty();
            }}
          />
        </label>
        <label>
          API Key
          <input
            type="password"
            value={apiKey}
            autoComplete="new-password"
            placeholder={
              profile?.hasApiKey ? "留空保留已保存密钥" : "本地模型可留空"
            }
            onChange={(event) => {
              setApiKey(event.target.value);
              markDirty();
            }}
          />
        </label>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={isDefault}
            onChange={(event) => {
              setIsDefault(event.target.checked);
              markDirty();
            }}
          />
          设为默认模型
        </label>
      </div>

      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      <div className="profile-form-actions">
        <button
          className="button button-primary"
          type="button"
          onClick={() => void handleSave()}
          disabled={busy}
        >
          保存模型
        </button>
        <button
          className="button button-secondary"
          type="button"
          onClick={onTest}
          disabled={busy || !profile}
        >
          测试连接
        </button>
        <button
          className="button button-danger"
          type="button"
          onClick={onDelete}
          disabled={busy || !profile}
        >
          删除模型
        </button>
      </div>
    </section>
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
  return "保存模型失败";
}
