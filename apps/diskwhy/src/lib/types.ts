// Mirrors the serde shapes in src-tauri/src/{lib,insights}.rs. Keep both sides in sync.

export interface DriveInfo {
  mount: string;
  name: string;
  fileSystem: string;
  total: number;
  available: number;
  removable: boolean;
}

export interface ScanStats {
  files: number;
  dirs: number;
  bytes: number;
  errors: number;
  cloudBytes: number;
  elapsedMs: number;
}

export interface ProgressEvent {
  scanId: number;
  files: number;
  dirs: number;
  bytes: number;
  errors: number;
  current: string;
  elapsedMs: number;
}

export interface DoneEvent {
  scanId: number;
  cancelled: boolean;
  stats: ScanStats;
}

export type Category =
  | "video"
  | "image"
  | "audio"
  | "archive"
  | "disk-image"
  | "installer"
  | "document"
  | "system"
  | "temp-log"
  | "code"
  | "game"
  | "database"
  | "other";

export interface CategoryStat {
  category: Category;
  bytes: number;
  count: number;
}

export interface ExtStat {
  ext: string;
  bytes: number;
  count: number;
}

export type Safety = "safe" | "review" | "system";

export interface Location {
  path: string;
  size: number;
  ids: number[];
  /** Unix seconds of the newest file modification inside (or of the file). null = unknown. */
  last_used: number | null;
  /** Short Thai explanation of why this location was flagged (e.g. "ไม่ได้แก้ไขมา 14 เดือน"). */
  note: string | null;
}

export interface Hog {
  id: string;
  label: string;
  advice: string;
  safety: Safety;
  size: number;
  count: number;
  locations: Location[];
}

export interface TopFile {
  path: string;
  name: string;
  size: number;
  category: Category;
  dir_ids: number[];
  /** Unix seconds, last modification. */
  modified: number;
}

export interface Report {
  rootPath: string;
  total: number;
  stats: ScanStats;
  categories: CategoryStat[];
  extensions: ExtStat[];
  hogs: Hog[];
  topFiles: TopFile[];
  drive: DriveInfo | null;
}

export interface ChildView {
  kind: "dir" | "file" | "small" | "rest";
  name: string;
  size: number;
  files: number;
  index: number | null;
  category: Category | null;
  denied: boolean;
  /** Unix seconds of the newest modification inside (dirs) or of the file. 0 = unknown. */
  newest: number;
}

export interface NodeView {
  ids: number[];
  path: string;
  name: string;
  size: number;
  files: number;
  dirs: number;
  crumbs: [string, number[]][];
  children: ChildView[];
}

export interface TrashTarget {
  path: string;
  dirIds: number[];
  fileName?: string;
}

/* ── AI advisor (src-tauri/src/ai.rs, serde rename_all = "camelCase") ───── */

export interface AiKeyStatus {
  configured: boolean;
  /** Where the key comes from. "keychain" = Windows Credential Manager, "env" = ANTHROPIC_API_KEY. */
  source: "keychain" | "env" | "none";
}

export interface AiProgress {
  stage: "prepare" | "classify" | "plan" | "done";
  done: number;
  total: number;
  message: string;
}

export type AiVerdict = "delete" | "review" | "keep" | "system_tool";
export type AiAction = "recycle_bin" | "uninstall_app" | "run_command" | "move_to_other_drive" | "compress" | "leave";

export interface AiItem {
  id: number;
  path: string;
  size: number;
  kind: "dir" | "file";
  /** Index path of the dir (kind=dir) or of the containing dir (kind=file); null if not in the tree. */
  dirIds: number[] | null;
  fileName: string | null;
  verdict: AiVerdict;
  confidence: "high" | "medium" | "low";
  reason: string;
  action: AiAction;
  command: string;
}

export interface AiCommand {
  label: string;
  command: string;
  shell: "powershell" | "cmd" | "settings";
  /** Needs an elevated (administrator) prompt. */
  admin: boolean;
}

export interface AiStep {
  title: string;
  detail: string;
  risk: "low" | "medium" | "high";
  estBytes: number;
  itemIds: number[];
  commands: AiCommand[];
}

export interface AiPlan {
  headline: string;
  diagnosis: string;
  reclaimSafeBytes: number;
  steps: AiStep[];
  warnings: string[];
  keep: string[];
}

export interface AiUsage {
  inputTokens: number;
  outputTokens: number;
  costUsd: number;
  models: string[];
}

export interface AiResult {
  items: AiItem[];
  plan: AiPlan;
  usage: AiUsage;
  anonymized: boolean;
}

export type SystemTool = "storage-sense" | "disk-cleanup" | "apps-features" | "recycle-bin";
