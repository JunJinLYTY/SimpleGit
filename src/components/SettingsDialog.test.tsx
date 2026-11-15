import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { AppSettings, SimpleGitClient } from "../types";
import { SettingsDialog } from "./SettingsDialog";

const initialSettings: AppSettings = {
  version: 1,
  recentRepositories: ["/repo"],
  activeModelId: "model-1",
  models: [
    {
      id: "model-1",
      name: "Local",
      baseUrl: "http://localhost:11434/v1",
      model: "qwen",
      timeoutSeconds: 60,
      isDefault: true,
      hasApiKey: true
    },
    {
      id: "model-2",
      name: "Cloud",
      baseUrl: "https://example.test/v1",
      model: "cloud-model",
      timeoutSeconds: 90,
      isDefault: false,
      hasApiKey: false
    }
  ],
  commitRule: "feat(scope): 中文摘要"
};

function makeClient(overrides: Partial<SimpleGitClient> = {}) {
  const diagnostics = {
    gitVersion: "git version 2.52.0",
    gitPath: "C:\\Program Files\\Git\\cmd\\git.exe",
    appVersion: "0.1.0"
  };
  const client = {
    openRepository: vi.fn(),
    repositoryStatus: vi.fn(),
    stageSelected: vi.fn(),
    stageAll: vi.fn(),
    unstageSelected: vi.fn(),
    readDiff: vi.fn(),
    commit: vi.fn(),
    pull: vi.fn(),
    push: vi.fn(),
    cancelOperation: vi.fn(),
    loadSettings: vi.fn(async () => initialSettings),
    saveModelProfile: vi.fn(async () => initialSettings),
    deleteModelProfile: vi.fn(async () => initialSettings),
    setActiveModel: vi.fn(async () => initialSettings),
    saveCommitRule: vi.fn(async () => initialSettings),
    testModelProfile: vi.fn(async () => ({ output: "test(ai): 连接成功" })),
    generateCommit: vi.fn(),
    recordRecentRepository: vi.fn(),
    diagnostics: vi.fn(async () => diagnostics),
    ...overrides
  } as unknown as SimpleGitClient;
  return client;
}

async function renderDialog(
  client = makeClient(),
  onSettingsChange = vi.fn(),
  onClose = vi.fn()
) {
  render(
    <SettingsDialog
      client={client}
      settings={initialSettings}
      onSettingsChange={onSettingsChange}
      onClose={onClose}
    />
  );
  await act(async () => {
    await Promise.resolve();
  });
  return { client, onSettingsChange, onClose };
}

describe("SettingsDialog", () => {
  it("shows profiles and key state without exposing a saved key", async () => {
    await renderDialog();

    expect(screen.getByRole("button", { name: "编辑 Local" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "编辑 Cloud" })).toBeInTheDocument();
    expect(screen.getAllByText("已配置").length).toBeGreaterThan(0);
    expect(screen.getByLabelText("API Key")).toHaveValue("");
    expect(await screen.findByText("git version 2.52.0")).toBeInTheDocument();
  });

  it("validates required fields before creating a profile", async () => {
    const { client } = await renderDialog();
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "新增模型" }));
    await user.click(screen.getByRole("button", { name: "保存模型" }));

    expect(client.saveModelProfile).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("配置名称不能为空");
  });

  it("creates a profile with all typed values", async () => {
    const createdSettings: AppSettings = {
      ...initialSettings,
      models: [
        ...initialSettings.models,
        {
          id: "model-3",
          name: "New",
          baseUrl: "https://new.test/v1",
          model: "new-model",
          timeoutSeconds: 45,
          isDefault: false,
          hasApiKey: true
        }
      ]
    };
    const saveModelProfile = vi.fn(async () => createdSettings);
    const { onSettingsChange } = await renderDialog(
      makeClient({ saveModelProfile })
    );
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "新增模型" }));
    await user.type(screen.getByLabelText("配置名称"), "New");
    await user.type(screen.getByLabelText("Base URL"), "https://new.test/v1");
    await user.type(screen.getByLabelText("Model ID"), "new-model");
    await user.clear(screen.getByLabelText("超时秒数"));
    await user.type(screen.getByLabelText("超时秒数"), "45");
    await user.type(screen.getByLabelText("API Key"), "new-secret");
    await user.click(screen.getByRole("button", { name: "保存模型" }));

    expect(saveModelProfile).toHaveBeenCalledWith({
      id: null,
      name: "New",
      baseUrl: "https://new.test/v1",
      model: "new-model",
      timeoutSeconds: 45,
      isDefault: false,
      apiKey: "new-secret",
      clearApiKey: false
    });
    expect(onSettingsChange).toHaveBeenCalledWith(createdSettings);
  });

  it("omits a blank key when editing and clears a replacement after save", async () => {
    const updated = {
      ...initialSettings,
      models: initialSettings.models.map((profile) =>
        profile.id === "model-1" ? { ...profile, name: "Local Renamed" } : profile
      )
    };
    const saveModelProfile = vi.fn(async () => updated);
    const { client } = await renderDialog(makeClient({ saveModelProfile }));
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "编辑 Local" }));
    await user.clear(screen.getByLabelText("配置名称"));
    await user.type(screen.getByLabelText("配置名称"), "Local Renamed");
    await user.click(screen.getByRole("button", { name: "保存模型" }));

    expect(saveModelProfile).toHaveBeenCalledWith({
      id: "model-1",
      name: "Local Renamed",
      baseUrl: "http://localhost:11434/v1",
      model: "qwen",
      timeoutSeconds: 60,
      isDefault: true,
      clearApiKey: false
    });
    expect(screen.getByLabelText("API Key")).toHaveValue("");

    await user.click(screen.getByRole("button", { name: "编辑 Cloud" }));
    await user.type(screen.getByLabelText("API Key"), "replacement");
    await user.click(screen.getByRole("button", { name: "保存模型" }));
    expect(saveModelProfile).toHaveBeenLastCalledWith({
      id: "model-2",
      name: "Cloud",
      baseUrl: "https://example.test/v1",
      model: "cloud-model",
      timeoutSeconds: 90,
      isDefault: false,
      apiKey: "replacement",
      clearApiKey: false
    });
    expect(screen.getByLabelText("API Key")).toHaveValue("");
    expect(client.saveModelProfile).toHaveBeenCalledTimes(2);
  });

  it("tests a saved profile and displays success or failure", async () => {
    const testModelProfile = vi
      .fn()
      .mockResolvedValueOnce({ output: "test(ai): 连接成功" })
      .mockRejectedValueOnce({
        code: "MODEL_REQUEST_FAILED",
        message: "模型请求超时"
      });
    await renderDialog(makeClient({ testModelProfile }));
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "测试连接" }));
    expect(testModelProfile).toHaveBeenCalledWith("model-1");
    expect(await screen.findByText("连接成功")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "测试连接" }));
    expect(await screen.findByText("模型请求超时")).toBeInTheDocument();
  });

  it("deletes the active profile and accepts the returned selection", async () => {
    const remaining: AppSettings = {
      ...initialSettings,
      activeModelId: "model-2",
      models: [initialSettings.models[1]]
    };
    const deleteModelProfile = vi.fn(async () => remaining);
    const { onSettingsChange } = await renderDialog(
      makeClient({ deleteModelProfile })
    );
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "删除模型" }));

    expect(deleteModelProfile).toHaveBeenCalledWith("model-1");
    expect(onSettingsChange).toHaveBeenCalledWith(remaining);
  });

  it("saves the exact global commit rule", async () => {
    const saveCommitRule = vi.fn(async () => initialSettings);
    await renderDialog(makeClient({ saveCommitRule }));
    const user = userEvent.setup();
    const rule = screen.getByLabelText("全局提交规则");

    await user.clear(rule);
    await user.type(rule, "fix(git): 中文规则");
    await user.click(screen.getByRole("button", { name: "保存规则" }));

    expect(saveCommitRule).toHaveBeenCalledWith("fix(git): 中文规则");
  });

  it("confirms before discarding unsaved edits", async () => {
    const { onClose } = await renderDialog();
    const user = userEvent.setup();

    await user.type(screen.getByLabelText("全局提交规则"), " 修改");
    await user.click(screen.getByRole("button", { name: "关闭设置" }));

    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "放弃修改" }));
    expect(onClose).toHaveBeenCalled();
  });
});
