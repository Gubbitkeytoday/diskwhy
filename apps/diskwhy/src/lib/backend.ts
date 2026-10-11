// The only data source is the Rust backend running inside the DiskWhy desktop app (Tauri).
// There is deliberately no mock/demo implementation: every number on screen comes from this machine.
import type {
  DoneEvent,
  DriveInfo,
  McpInfo,
  McpTarget,
  NodeView,
  ProgressEvent,
  Report,
  SystemTool,
  TrashTarget,
} from "./types";

export interface Backend {
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
  /** Opens a built-in Windows tool (Storage Sense, Disk Cleanup, Apps & features, Recycle Bin). */
  openSystemTool(tool: SystemTool): Promise<void>;

  /** What is installed (Claude Code / Claude Desktop / bundled CLI) and what is already registered. Read-only. */
  mcpInfo(): Promise<McpInfo>;
  /** Registers DiskWhy's MCP server with that Claude app. Only call from an explicit user action. */
  mcpRegister(target: McpTarget): Promise<McpInfo>;
  /** Removes the registration again. Only call from an explicit user action. */
  mcpUnregister(target: McpTarget): Promise<McpInfo>;
}

export const isNative = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function createNative(): Promise<Backend> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
  const { open } = await import("@tauri-apps/plugin-dialog");

  return {
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
        unlistenProgress();
      }
    },
    cancelScan: () => invoke("cancel_scan"),
    getReport: () => invoke<Report>("get_report"),
    getNode: (ids) => invoke<NodeView>("get_node", { ids }),
    reveal: (path) => revealItemInDir(path),
    moveToTrash: (t) => invoke<number>("move_to_trash", { path: t.path, dirIds: t.dirIds, fileName: t.fileName ?? null }),
    openSystemTool: (tool) => invoke("open_system_tool", { tool }),

    mcpInfo: () => invoke<McpInfo>("mcp_info"),
    mcpRegister: (target) => invoke<McpInfo>("mcp_register", { target }),
    mcpUnregister: (target) => invoke<McpInfo>("mcp_unregister", { target }),
  };
}

let instance: Promise<Backend> | null = null;

/** Rejects outside the desktop app — callers show the "open in DiskWhy" screen instead of fake data. */
export function getBackend(): Promise<Backend> {
  if (!isNative) return Promise.reject(new Error("NOT_IN_APP"));
  instance ??= createNative();
  return instance;
}
