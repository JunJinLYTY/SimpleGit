import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type {
  AppSettings,
  RepositorySnapshot,
  SimpleGitClient
} from "./types";

const settings: AppSettings = {
  version: 1,
  recentRepositories: [],
  activeModelId: "model-1",
  models: [
    {
      id: "model-1",
      name: "Local",
      baseUrl: "http://localhost:11434/v1",
      model: "qwen",
      timeoutSeconds: 60,
      isDefault: true,
      hasApiKey: false
    }
  ],
  commitRule: "Conventional Commits"
};

function snapshot(
  overrides: Partial<RepositorySnapshot["repository"]> = {}
): RepositorySnapshot {
  return {
    repository: {
      root: "/repo",
      name: "demo",
      branch: "main",
      upstream: "origin/main",
      ahead: 1,
      behind: 0,
      hasConflicts: false,
      ...overrides
    },
    changes: [
      {
        path: "src/a.ts",
        originalPath: null,
        indexStatus: ".",
        worktreeStatus: "M",
        staged: false,
        unstaged: true,
        conflicted: false,
        kind: "modified"
      },
      {
        path: "src/b.ts",
        originalPath: null,
        indexStatus: "M",
        worktreeStatus: ".",
        staged: true,
        unstaged: false,
        conflicted: false,
        kind: "modified"
      }
    ]
  };
}

function makeClient(overrides: Partial<SimpleGitClient> = {}) {
  const defaultSnapshot = snapshot();
  const client = {
    openRepository: vi.fn(async () => defaultSnapshot),
    repositoryStatus: vi.fn(async () => defaultSnapshot),
    stageSelected: vi.fn(async () => defaultSnapshot),
    stageAll: vi.fn(async () => defaultSnapshot),
    unstageSelected: vi.fn(async () => defaultSnapshot),
    readDiff: vi.fn(async () => "+ added line"),
    commit: vi.fn(async () => ({ hash: "abc", summary: "feat: test" })),
    pull: vi.fn(async () => ({ output: "Fast-forward" })),
    push: vi.fn(async () => ({ output: "Everything up-to-date" })),
    cancelOperation: vi.fn(async () => true),
    loadSettings: vi.fn(async () => settings),
    saveModelProfile: vi.fn(async () => settings),
    deleteModelProfile: vi.fn(async () => settings),
    setActiveModel: vi.fn(async () => settings),
    saveCommitRule: vi.fn(async () => settings),
    testModelProfile: vi.fn(async () => ({ output: "ok" })),
    generateCommit: vi.fn(async () => "feat(ui): 生成内容"),
    recordRecentRepository: vi.fn(async () => settings),
    diagnostics: vi.fn(async () => ({
      gitVersion: "git version 2",
      gitPath: "git",
      appVersion: "0.1.0"
    })),
    ...overrides
  } satisfies SimpleGitClient;
  return client;
}

async function openRepository(client: SimpleGitClient, repository = "/repo") {
  const user = userEvent.setup();
  render(
    <App
      client={client}
      pickRepository={vi.fn(async () => repository)}
    />
  );
  await user.click(screen.getByRole("button", { name: "打开仓库" }));
  await screen.findByText("demo");
  return user;
}

describe("SimpleGit main workspace", () => {
  beforeEach(() => {
    vi.stubGlobal("crypto", {
      randomUUID: vi.fn(() => "operation-id")
    });
  });

  it("starts with an open repository action and loads the chosen path", async () => {
    const client = makeClient();
    const pickRepository = vi.fn(async () => "/repo");
    const user = userEvent.setup();
    render(<App client={client} pickRepository={pickRepository} />);

    expect(
      screen.getByRole("heading", { name: "SimpleGit" })
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "打开仓库" }));

    await waitFor(() => {
      expect(client.openRepository).toHaveBeenCalledWith("/repo");
    });
    expect(await screen.findByText("demo")).toBeInTheDocument();
  });

  it("shows branch state and both staged and unstaged paths", async () => {
    const client = makeClient();
    await openRepository(client);

    expect(screen.getByText("main")).toBeInTheDocument();
    expect(screen.getByText("领先 1")).toBeInTheDocument();
    expect(screen.getByText("落后 0")).toBeInTheDocument();
    expect(screen.getByText("src/a.ts")).toBeInTheDocument();
    expect(screen.getByText("src/b.ts")).toBeInTheDocument();
  });

  it("refreshes repository state when the window regains focus", async () => {
    const client = makeClient();
    await openRepository(client);
    vi.mocked(client.repositoryStatus).mockClear();

    fireEvent.focus(window);

    await waitFor(() => {
      expect(client.repositoryStatus).toHaveBeenCalledWith("/repo");
    });
  });

  it("stages all changes when no file is selected", async () => {
    const client = makeClient();
    const user = await openRepository(client);

    await user.click(screen.getByRole("button", { name: "暂存全部更改" }));

    expect(client.stageAll).toHaveBeenCalledWith("/repo");
    expect(client.stageSelected).not.toHaveBeenCalled();
  });

  it("stages exactly the selected paths", async () => {
    const client = makeClient();
    const user = await openRepository(client);

    await user.click(screen.getByRole("checkbox", { name: "选择 src/a.ts" }));
    await user.click(
      screen.getByRole("button", { name: "暂存选中的 1 个文件" })
    );

    expect(client.stageSelected).toHaveBeenCalledWith("/repo", ["src/a.ts"]);
  });

  it("opens a file diff in a preview dialog", async () => {
    const client = makeClient();
    const user = await openRepository(client);

    await user.click(
      screen.getByRole("button", { name: "查看 src/a.ts 的未暂存 diff" })
    );

    expect(client.readDiff).toHaveBeenCalledWith("/repo", "src/a.ts", false);
    expect(
      await screen.findByRole("dialog", { name: "Diff 预览" })
    ).toBeInTheDocument();
    expect(await screen.findByText("+ added line")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "关闭" }));
    expect(screen.queryByRole("dialog", { name: "Diff 预览" })).toBeNull();
  });

  it("does not generate or commit when the message is empty", async () => {
    const client = makeClient();
    const user = await openRepository(client);
    const editor = screen.getByRole("textbox", { name: "提交说明" });
    const commitButton = screen.getByRole("button", { name: "Commit" });

    expect(commitButton).toBeDisabled();
    await user.click(commitButton);
    expect(client.generateCommit).not.toHaveBeenCalled();
    expect(client.commit).not.toHaveBeenCalled();

    await user.type(editor, "   ");
    expect(editor).toHaveValue("   ");
    expect(commitButton).toBeDisabled();
    expect(client.generateCommit).not.toHaveBeenCalled();
    expect(client.commit).not.toHaveBeenCalled();
  });

  it("shows only the configured model name in the commit selector", async () => {
    const client = makeClient();
    await openRepository(client);

    expect(screen.getByRole("option", { name: "Local" })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: "Local · qwen" })).toBeNull();
  });

  it("commits manually entered content without generating", async () => {
    const client = makeClient();
    const user = await openRepository(client);
    const editor = screen.getByRole("textbox", { name: "提交说明" });

    await user.type(editor, "fix(git): 手工说明");
    await user.click(screen.getByRole("button", { name: "Commit" }));

    expect(client.generateCommit).not.toHaveBeenCalled();
    expect(client.commit).toHaveBeenCalledWith(
      "/repo",
      "fix(git): 手工说明",
      "operation-id"
    );
  });

  it("disables generation and commit when conflicts exist", async () => {
    const conflictSnapshot = snapshot({ hasConflicts: true });
    const client = makeClient({
      openRepository: vi.fn(async () => conflictSnapshot)
    });
    await openRepository(client);

    expect(screen.getByRole("button", { name: "AI 生成" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Commit" })).toBeDisabled();
    expect(screen.getByRole("note")).toHaveTextContent("存在未解决冲突");
  });

  it("confirms before setting a missing origin upstream", async () => {
    const client = makeClient({
      openRepository: vi.fn(async () => snapshot({ upstream: null }))
    });
    const user = await openRepository(client);

    await user.click(screen.getByRole("button", { name: "Push" }));
    expect(client.push).not.toHaveBeenCalled();
    await user.click(
      screen.getByRole("button", { name: "设置 origin 上游并推送" })
    );

    expect(client.push).toHaveBeenCalledWith("/repo", true, "operation-id");
  });

  it("offers cancellation while a long operation is pending", async () => {
    let resolvePull: ((value: { output: string }) => void) | undefined;
    const client = makeClient({
      pull: vi.fn(
        () =>
          new Promise<{ output: string }>((resolve) => {
            resolvePull = resolve;
          })
      )
    });
    const user = await openRepository(client);

    await user.click(screen.getByRole("button", { name: "Pull" }));
    await user.click(screen.getByRole("button", { name: "取消操作" }));

    expect(client.cancelOperation).toHaveBeenCalledWith("operation-id");
    resolvePull?.({ output: "cancelled" });
  });

  it("keeps the editor content and shows Git details after a failed commit", async () => {
    const client = makeClient({
      commit: vi.fn(async () => {
        throw {
          code: "GIT_FAILED",
          message: "Git 操作失败",
          details: "pre-commit hook rejected"
        };
      })
    });
    const user = await openRepository(client);
    const editor = screen.getByRole("textbox", { name: "提交说明" });

    await user.type(editor, "feat: 保留内容");
    await user.click(screen.getByRole("button", { name: "Commit" }));

    expect(editor).toHaveValue("feat: 保留内容");
    expect(await screen.findByRole("alert")).toHaveTextContent("Git 操作失败");
    expect(screen.getByRole("alert")).toHaveTextContent(
      "pre-commit hook rejected"
    );
  });
});
