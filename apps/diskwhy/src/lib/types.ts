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
