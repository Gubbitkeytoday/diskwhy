// Pure helpers for the AI advisor UI (no React) so the trickier rules are unit-testable.
import type { AiAction, AiItem, AiResult, AiVerdict, SystemTool, TrashTarget } from "./types";

/** A trash request as the results screen understands it: the backend target plus what the confirm dialog shows. */
export interface TrashRequest extends TrashTarget {
  name: string;
  size: number;
}

/** Backend rejections arrive as plain strings (Tauri) or Errors; always show the Thai message untouched. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

export const baseName = (path: string) => path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || path;

/** Case-insensitive, slash-normalised, no trailing separator — good enough to compare Windows paths. */
export const normPath = (path: string) => path.replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase();

/** Which built-in Windows tool a command/URI refers to (storagesense, cleanmgr, appsfeatures, RecycleBin), if any. */
export function systemToolFor(command: string): SystemTool | null {
  // shell "settings" commands from ai.rs are exactly one of the tool ids.
  const exact = command.trim().toLowerCase();
  if (exact === "storage-sense" || exact === "disk-cleanup" || exact === "apps-features" || exact === "recycle-bin") return exact;
  const c = exact.replace(/[^a-z0-9]/g, "");
  if (c.includes("cleanmgr") || c.includes("diskcleanup")) return "disk-cleanup";
  if (c.includes("storagesense")) return "storage-sense";
  if (c.includes("appsfeatures")) return "apps-features";
  if (c.includes("recyclebin")) return "recycle-bin";
  return null;
}

export const SYSTEM_TOOL_LABEL: Record<SystemTool, string> = {
  "storage-sense": "เปิด Storage Sense",
  "disk-cleanup": "เปิด Disk Cleanup",
  "apps-features": "เปิดแอปและคุณลักษณะ",
  "recycle-bin": "เปิดถังขยะ",
};

export const VERDICT_ORDER: AiVerdict[] = ["delete", "review", "keep", "system_tool"];

export const ACTION_LABEL: Record<AiAction, string | null> = {
  recycle_bin: null,
  leave: null,
  uninstall_app: "ถอนการติดตั้งแอป",
  run_command: "ใช้คำสั่ง",
  move_to_other_drive: "ย้ายไปไดรฟ์อื่น",
  compress: "บีบอัด",
};

/** Maps an item to a backend trash target; null when it is not addressable in the scan tree. */
export function trashTargetFor(item: AiItem): TrashRequest | null {
  if (!item.dirIds) return null;
  if (item.kind === "dir") {
    if (item.dirIds.length === 0) return null; // the scan root itself is never offered
    return { path: item.path, dirIds: item.dirIds, name: baseName(item.path), size: item.size };
  }
  const fileName = item.fileName ?? baseName(item.path);
  return { path: item.path, dirIds: item.dirIds, fileName, name: fileName, size: item.size };
}

/** Trash is only ever offered for delete/review verdicts whose action is the Recycle Bin or "decide yourself". Never for system tools. */
export function canTrashItem(item: AiItem): boolean {
  if (item.verdict !== "delete" && item.verdict !== "review") return false;
  if (item.action !== "recycle_bin" && item.action !== "leave") return false;
  return trashTargetFor(item) !== null;
}

/** Result plus the ids already moved to the Recycle Bin during this session. */
export interface AiView {
  result: AiResult;
  done: ReadonlySet<number>;
}

/**
 * Keeps the result consistent after something was moved to the Recycle Bin (from any tab):
 * items inside the removed path become "done", and — because the Rust tree removes a directory with
 * `Vec::remove` — later sibling directory indices shift down by one, so other items' `dirIds` are re-indexed.
 */
export function applyRemoval(view: AiView, removed: TrashTarget): AiView {
  const gone = normPath(removed.path);
  const dir = removed.fileName ? null : removed.dirIds; // a removed file never shifts directory indices
  const parent = dir && dir.length > 0 ? dir.slice(0, -1) : null;
  const removedIdx = dir && dir.length > 0 ? (dir[dir.length - 1] as number) : -1;

  const done = new Set(view.done);
  const items = view.result.items.map((item) => {
    const p = normPath(item.path);
    if (p === gone || p.startsWith(`${gone}\\`)) {
      done.add(item.id);
      return item;
    }
    if (!parent || !item.dirIds || item.dirIds.length <= parent.length) return item;
    if (!parent.every((v, i) => item.dirIds?.[i] === v)) return item;
    const at = item.dirIds[parent.length] as number;
    if (at === removedIdx) {
      done.add(item.id); // lives inside the removed directory
      return item;
    }
    if (at > removedIdx) {
      const dirIds = item.dirIds.slice();
      dirIds[parent.length] = at - 1;
      return { ...item, dirIds };
    }
    return item;
  });
  return { result: { ...view.result, items }, done };
}
