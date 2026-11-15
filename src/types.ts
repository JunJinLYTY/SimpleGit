export type FileChangeKind =
  | "added"
  | "modified"
  | "deleted"
  | "renamed"
  | "untracked"
  | "conflicted"
  | "unknown";

export interface CommandError {
  code: string;
  message: string;
  details?: string;
}

export interface RepositoryInfo {
  root: string;
  name: string;
  branch: string;
  upstream: string | null;
  ahead: number;
  behind: number;
  hasConflicts: boolean;
}

export interface FileChange {
  path: string;
  originalPath: string | null;
  indexStatus: string;
  worktreeStatus: string;
  staged: boolean;
  unstaged: boolean;
  conflicted: boolean;
  kind: FileChangeKind;
}

export interface RepositorySnapshot {
  repository: RepositoryInfo;
  changes: FileChange[];
}

export interface CommitResult {
  hash: string;
  summary: string;
}

export interface OperationResult {
  output: string;
}

export interface ModelProfile {
  id: string;
  name: string;
  baseUrl: string;
  model: string;
  timeoutSeconds: number;
  isDefault: boolean;
  hasApiKey: boolean;
}

export interface ModelProfileInput {
  id?: string | null;
  name: string;
  baseUrl: string;
  model: string;
  timeoutSeconds: number;
  isDefault: boolean;
  apiKey?: string;
  clearApiKey?: boolean;
}

export interface AppSettings {
  version: number;
  recentRepositories: string[];
  activeModelId: string | null;
  models: ModelProfile[];
  commitRule: string;
}

export interface ConnectionTestResult {
  output: string;
}

export interface Diagnostics {
  gitVersion: string;
  gitPath: string;
  appVersion: string;
}

export type InvokeFunction = <T>(
  command: string,
  args?: Record<string, unknown>
) => Promise<T>;

export interface SimpleGitClient {
  openRepository(path: string): Promise<RepositorySnapshot>;
  repositoryStatus(path: string): Promise<RepositorySnapshot>;
  stageSelected(path: string, paths: string[]): Promise<RepositorySnapshot>;
  stageAll(path: string): Promise<RepositorySnapshot>;
  unstageSelected(path: string, paths: string[]): Promise<RepositorySnapshot>;
  readDiff(path: string, filePath: string, staged: boolean): Promise<string>;
  commit(
    repositoryPath: string,
    message: string,
    operationId: string
  ): Promise<CommitResult>;
  pull(repositoryPath: string, operationId: string): Promise<OperationResult>;
  push(
    repositoryPath: string,
    setUpstream: boolean,
    operationId: string
  ): Promise<OperationResult>;
  cancelOperation(operationId: string): Promise<boolean>;
  loadSettings(): Promise<AppSettings>;
  saveModelProfile(input: ModelProfileInput): Promise<AppSettings>;
  deleteModelProfile(profileId: string): Promise<AppSettings>;
  setActiveModel(profileId: string | null): Promise<AppSettings>;
  saveCommitRule(rule: string): Promise<AppSettings>;
  testModelProfile(profileId: string): Promise<ConnectionTestResult>;
  generateCommit(
    repositoryPath: string,
    modelId: string,
    operationId: string
  ): Promise<string>;
  recordRecentRepository(path: string): Promise<AppSettings>;
  diagnostics(): Promise<Diagnostics>;
}
