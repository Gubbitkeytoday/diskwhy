// One interface, two implementations: the real Rust backend (inside Tauri) and an in-browser demo
// (for UI development, tests and the shareable preview). UI code never imports Tauri directly.
import type { DoneEvent, DriveInfo, NodeView, ProgressEvent, Report, TrashTarget } from "./types";

export interface Backend {
  kind: "native" | "demo";
  listDrives(): Promise<DriveInfo[]>;
  pickFolder(): Promise<string | null>;
  /** Resolves when the scan finishes or is cancelled. */
  scan(path: string, onProgress: (p: ProgressEvent) => void): Promise<DoneEvent>;
  cancelScan(): Promise<void>;
  getReport(): Promise<Report>;
  getNode(ids: number[]): Promise<NodeView>;
  reveal(path: string): Promise<void>;
  /** Moves to the Recycle Bin. Returns bytes freed. */
  moveToTrash(target: TrashTarget): Promise<number>;
}

export const isNative = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function createNative(): Promise<Backend> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
  const { open } = await import("@tauri-apps/plugin-dialog");

  return {
    kind: "native",
    listDrives: () => invoke<DriveInfo[]>("list_drives"),
    async pickFolder() {
      const picked = await open({ directory: true, multiple: false, title: "เลือกโฟลเดอร์ที่จะสแกน" });
      return typeof picked === "string" ? picked : null;
    },
    async scan(path, onProgress) {
      let scanId = -1;
      const unlistenProgress = await listen<ProgressEvent>("scan-progress", (e) => {
        if (scanId === -1 || e.payload.scanId === scanId) onProgress(e.payload);
      });
      try {
        let unlistenDone: (() => void) | undefined;
        try {
          return await new Promise<DoneEvent>((resolve, reject) => {
            const early: DoneEvent[] = []; // a tiny folder can finish before start_scan even returns its id
            listen<DoneEvent>("scan-done", (e) => {
              if (scanId === -1) early.push(e.payload);
              else if (e.payload.scanId === scanId) resolve(e.payload);
            })
              .then((u) => {
                unlistenDone = u;
                return invoke<number>("start_scan", { path });
              })
              .then((id) => {
                scanId = id;
                const hit = early.find((d) => d.scanId === id);
                if (hit) resolve(hit);
              })
              .catch(reject);
          });
        } finally {
          unlistenDone?.();
        }
      } finally {
        unlistenProgress();
      }
    },
    cancelScan: () => invoke("cancel_scan"),
    getReport: () => invoke<Report>("get_report"),
    getNode: (ids) => invoke<NodeView>("get_node", { ids }),
    reveal: (path) => revealItemInDir(path),
    moveToTrash: (t) => invoke<number>("move_to_trash", { path: t.path, dirIds: t.dirIds, fileName: t.fileName ?? null }),
  };
}

let instance: Promise<Backend> | null = null;
export function getBackend(): Promise<Backend> {
  instance ??= isNative ? createNative() : import("./demo").then((m) => m.createDemoBackend());
  return instance;
}
