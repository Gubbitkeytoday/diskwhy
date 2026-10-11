// Mirrors the serde shapes in src-tauri/src/{lib,mcp_setup}.rs and crates/core. Keep both sides in sync.

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

/** A trash request as the results screen understands it: the backend target plus what the confirm dialog shows. */
export interface TrashRequest extends TrashTarget {
  name: string;
  size: number;
}

/* ── Claude (MCP) connection (src-tauri/src/mcp_setup.rs, serde rename_all = "camelCase") ───── */

export type McpTarget = "claude-code" | "claude-desktop";

export interface McpInfo {
  /** Absolute path of the bundled diskwhy-cli.exe; null when it cannot be found (dev build without `pnpm cli:build`). */
  cliPath: string | null;
  cliFound: boolean;
  /** claude.exe / claude.cmd of Claude Code; null = not installed. */
  claudeCodePath: string | null;
  claudeCodeRegistered: boolean;
  /** Config file Claude Desktop reads; null = Claude Desktop not found. */
  claudeDesktopConfigPath: string | null;
  claudeDesktopRegistered: boolean;
  /** Command to paste in a terminal to register DiskWhy with Claude Code by hand. */
  claudeCodeCommand: string;
  /** JSON to merge into claude_desktop_config.json by hand. */
  desktopSnippet: string;
  /** Thai message after register/unregister (for example "restart Claude Desktop"); null from `mcpInfo`. */
  note: string | null;
}

export type SystemTool = "storage-sense" | "disk-cleanup" | "apps-features" | "recycle-bin";
