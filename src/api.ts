import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  CommandError,
  CommitResult,
  ConnectionTestResult,
  Diagnostics,
  InvokeFunction,
  ModelProfileInput,
  OperationResult,
  RepositorySnapshot,
  SimpleGitClient
} from "./types";

export class SimpleGitCommandError extends Error {
  readonly code: string;
  readonly details?: string;

  constructor(error: CommandError) {
    super(error.message);
    this.name = "SimpleGitCommandError";
    this.code = error.code;
    this.details = error.details;
  }
}

export function createSimpleGitClient(
  invoke: InvokeFunction = tauriInvoke as InvokeFunction
): SimpleGitClient {
  const call = async <T>(
    command: string,
    args?: Record<string, unknown>
  ): Promise<T> => {
    try {
      return args
        ? await invoke<T>(command, args)
        : await invoke<T>(command);
    } catch (error) {
      throw new SimpleGitCommandError(toCommandError(error));
    }
  };

  return {
    openRepository: (path) =>
      call<RepositorySnapshot>("open_repository", { path }),
    repositoryStatus: (path) =>
      call<RepositorySnapshot>("repository_status", { path }),
    stageSelected: (path, paths) =>
      call<RepositorySnapshot>("stage_selected", { path, paths }),
    stageAll: (path) => call<RepositorySnapshot>("stage_all", { path }),
    unstageSelected: (path, paths) =>
      call<RepositorySnapshot>("unstage_selected", { path, paths }),
    readDiff: (path, filePath, staged) =>
      call<string>("read_diff", { path, filePath, staged }),
    commit: (repositoryPath, message, operationId) =>
      call<CommitResult>("commit", {
        repositoryPath,
        message,
        operationId
      }),
    pull: (repositoryPath, operationId) =>
      call<OperationResult>("pull", { repositoryPath, operationId }),
    push: (repositoryPath, setUpstream, operationId) =>
      call<OperationResult>("push", {
        repositoryPath,
        setUpstream,
        operationId
      }),
    cancelOperation: (operationId) =>
      call<boolean>("cancel_operation", { operationId }),
    loadSettings: () => call<AppSettings>("load_settings"),
    saveModelProfile: (input: ModelProfileInput) =>
      call<AppSettings>("save_model_profile", { input }),
    deleteModelProfile: (profileId) =>
      call<AppSettings>("delete_model_profile", { profileId }),
    setActiveModel: (profileId) =>
      call<AppSettings>("set_active_model", { profileId }),
    saveCommitRule: (rule) =>
      call<AppSettings>("save_commit_rule", { rule }),
    testModelProfile: (profileId) =>
      call<ConnectionTestResult>("test_model_profile", { profileId }),
    generateCommit: (repositoryPath, modelId, operationId) =>
      call<string>("generate_commit", {
        repositoryPath,
        modelId,
        operationId
      }),
    recordRecentRepository: (path) =>
      call<AppSettings>("record_recent_repository", { path }),
    diagnostics: () => call<Diagnostics>("diagnostics")
  };
}

export const tauriClient = createSimpleGitClient();

function toCommandError(error: unknown): CommandError {
  if (isCommandError(error)) {
    return error;
  }
  if (error instanceof Error) {
    return { code: "UNKNOWN", message: error.message };
  }
  if (typeof error === "string") {
    return { code: "UNKNOWN", message: error };
  }
  return { code: "UNKNOWN", message: "未知的桌面命令错误" };
}

function isCommandError(error: unknown): error is CommandError {
  if (!error || typeof error !== "object") {
    return false;
  }
  const candidate = error as Partial<CommandError>;
  return (
    typeof candidate.code === "string" &&
    typeof candidate.message === "string"
  );
}
