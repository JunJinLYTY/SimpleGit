import { describe, expect, it, vi } from "vitest";
import { createSimpleGitClient } from "./api";
import type { InvokeFunction, ModelProfileInput } from "./types";

describe("SimpleGit API client", () => {
  it("invokes every backend command with the typed camelCase arguments", async () => {
    const invoke = vi.fn(async (_command: string, _args?: Record<string, unknown>) =>
      ({
        repository: {
          root: "/repo",
          name: "repo",
          branch: "main",
          upstream: null,
          ahead: 0,
          behind: 0,
          hasConflicts: false
        },
        changes: []
      })
    );
    const client = createSimpleGitClient(invoke as InvokeFunction);
    const input: ModelProfileInput = {
      id: null,
      name: "Local",
      baseUrl: "http://localhost:11434/v1",
      model: "qwen",
      timeoutSeconds: 60,
      isDefault: true,
      apiKey: "secret",
      clearApiKey: false
    };

    await client.openRepository("/repo");
    await client.repositoryStatus("/repo");
    await client.stageSelected("/repo", ["a.ts", "b.ts"]);
    await client.stageAll("/repo");
    await client.unstageSelected("/repo", ["a.ts"]);
    await client.readDiff("/repo", "a.ts", true);
    await client.commit("/repo", "feat: 中文", "commit-op");
    await client.pull("/repo", "pull-op");
    await client.push("/repo", true, "push-op");
    await client.cancelOperation("push-op");
    await client.loadSettings();
    await client.saveModelProfile(input);
    await client.deleteModelProfile("profile-1");
    await client.setActiveModel("profile-2");
    await client.saveCommitRule("自定义规则");
    await client.testModelProfile("profile-2");
    await client.generateCommit("/repo", "profile-2", "generate-op");
    await client.recordRecentRepository("/repo");
    await client.diagnostics();

    expect(invoke.mock.calls).toEqual([
      ["open_repository", { path: "/repo" }],
      ["repository_status", { path: "/repo" }],
      ["stage_selected", { path: "/repo", paths: ["a.ts", "b.ts"] }],
      ["stage_all", { path: "/repo" }],
      ["unstage_selected", { path: "/repo", paths: ["a.ts"] }],
      ["read_diff", { path: "/repo", filePath: "a.ts", staged: true }],
      [
        "commit",
        { repositoryPath: "/repo", message: "feat: 中文", operationId: "commit-op" }
      ],
      ["pull", { repositoryPath: "/repo", operationId: "pull-op" }],
      [
        "push",
        { repositoryPath: "/repo", setUpstream: true, operationId: "push-op" }
      ],
      ["cancel_operation", { operationId: "push-op" }],
      ["load_settings"],
      ["save_model_profile", { input }],
      ["delete_model_profile", { profileId: "profile-1" }],
      ["set_active_model", { profileId: "profile-2" }],
      ["save_commit_rule", { rule: "自定义规则" }],
      ["test_model_profile", { profileId: "profile-2" }],
      [
        "generate_commit",
        {
          repositoryPath: "/repo",
          modelId: "profile-2",
          operationId: "generate-op"
        }
      ],
      ["record_recent_repository", { path: "/repo" }],
      ["diagnostics"]
    ]);
  });

  it("normalizes unknown invocation failures into command errors", async () => {
    const client = createSimpleGitClient(async () => {
      throw new Error("bridge disconnected");
    });

    await expect(client.diagnostics()).rejects.toMatchObject({
      code: "UNKNOWN",
      message: "bridge disconnected"
    });
  });
});
