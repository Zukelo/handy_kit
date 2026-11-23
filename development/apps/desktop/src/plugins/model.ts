export interface SyncRepository {
  name: string;
  branch: string;
  mergeBranch: string;
  url: string;
  directory: string;
}
export interface SyncConfig {
  workspace: string;
  svnDir: string;
  git: string;
  svn: string;
  svnUsername: string;
  svnCredentialFile: string;
  gitName: string;
  gitEmail: string;
  svnTimeout: number;
  commandTimeout: number;
  repositories: SyncRepository[];
}
export interface SyncProfile {
  revision: number;
  config: SyncConfig;
}
export interface Workflow {
  perRepository: string[];
  afterRepositories: string[];
}
export interface SyncOutcome {
  name: string;
  directory: string;
  status: string;
  gitPush: string;
  gitCommit?: string;
  svnCommit: string;
  archive: string;
  message: string;
}
export interface SyncRun {
  id: string;
  pluginId: string;
  status: string;
  startedAt: string;
  finishedAt: string | null;
  logs: string[];
  repositories: SyncOutcome[];
  message: string;
  configRevision: number;
}
export interface PluginManifest {
  id: string;
  name: string;
  description: string;
  version: string;
  operations: string[];
  workflow?: Workflow;
}
export interface InstalledPlugin {
  manifest: PluginManifest;
  enabled: boolean;
}
export interface PluginCheck {
  name: string;
  status: string;
  detail: string;
}
export interface PluginReport {
  pluginId: string;
  pluginVersion: string;
  platform: string;
  checks: PluginCheck[];
  scope: string;
}
