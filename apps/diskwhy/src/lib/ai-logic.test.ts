import { describe, expect, it } from "vitest";
import { applyRemoval, canTrashItem, errorText, systemToolFor, trashTargetFor } from "./ai-logic";
import type { AiItem, AiResult } from "./types";

const item = (over: Partial<AiItem>): AiItem => ({
  id: 1,
  path: "C:\\Users\\a\\Downloads\\old",
  size: 100,
  kind: "dir",
  dirIds: [0, 2],
  fileName: null,
  verdict: "delete",
  confidence: "high",
  reason: "",
  action: "recycle_bin",
  command: "",
  ...over,
});

const result = (items: AiItem[]): AiResult => ({
  items,
  plan: { headline: "", diagnosis: "", reclaimSafeBytes: 0, steps: [], warnings: [], keep: [] },
  usage: { inputTokens: 0, outputTokens: 0, costUsd: 0, models: [] },
  anonymized: true,
});

describe("systemToolFor", () => {
  it("recognises the built-in Windows tools", () => {
    expect(systemToolFor("ms-settings:storagesense")).toBe("storage-sense");
    expect(systemToolFor("cleanmgr /d C:")).toBe("disk-cleanup");
    expect(systemToolFor("ms-settings:appsfeatures")).toBe("apps-features");
    expect(systemToolFor("explorer shell:RecycleBinFolder")).toBe("recycle-bin");
    expect(systemToolFor("Clear-RecycleBin -DriveLetter C")).toBe("recycle-bin");
    expect(systemToolFor("Remove-Item C:\\temp")).toBeNull();
  });
});

describe("canTrashItem", () => {
  it("only offers trash for delete/review + recycle_bin/leave with a tree address", () => {
    expect(canTrashItem(item({}))).toBe(true);
    expect(canTrashItem(item({ verdict: "review", action: "leave" }))).toBe(true);
    expect(canTrashItem(item({ verdict: "keep" }))).toBe(false);
    expect(canTrashItem(item({ verdict: "system_tool", action: "recycle_bin" }))).toBe(false);
    expect(canTrashItem(item({ action: "uninstall_app" }))).toBe(false);
    expect(canTrashItem(item({ dirIds: null }))).toBe(false);
    expect(canTrashItem(item({ dirIds: [] }))).toBe(false);
  });
});

describe("trashTargetFor", () => {
  it("passes the dir path for dirs and the file name for files", () => {
    expect(trashTargetFor(item({}))).toEqual({ path: "C:\\Users\\a\\Downloads\\old", dirIds: [0, 2], name: "old", size: 100 });
    const f = trashTargetFor(item({ kind: "file", path: "C:\\x\\big.iso", dirIds: [3], fileName: "big.iso" }));
    expect(f).toMatchObject({ dirIds: [3], fileName: "big.iso", name: "big.iso" });
  });
});

describe("applyRemoval", () => {
  it("marks the removed item done and re-indexes later siblings", () => {
    const a = item({ id: 1, path: "C:\\d\\a", dirIds: [0, 1] });
    const b = item({ id: 2, path: "C:\\d\\b", dirIds: [0, 2] });
    const inner = item({ id: 3, path: "C:\\d\\a\\x", dirIds: [0, 1, 0] });
    const other = item({ id: 4, path: "C:\\e\\z", dirIds: [5] });
    const view = applyRemoval({ result: result([a, b, inner, other]), done: new Set() }, { path: "C:\\d\\a", dirIds: [0, 1] });
    expect([...view.done].sort()).toEqual([1, 3]);
    expect(view.result.items.find((i) => i.id === 2)?.dirIds).toEqual([0, 1]);
    expect(view.result.items.find((i) => i.id === 4)?.dirIds).toEqual([5]);
  });

  it("does not shift directory indices when a file is removed", () => {
    const f = item({ id: 1, kind: "file", path: "C:\\d\\f.bin", dirIds: [0], fileName: "f.bin" });
    const b = item({ id: 2, path: "C:\\d\\b", dirIds: [0, 2] });
    const view = applyRemoval({ result: result([f, b]), done: new Set() }, { path: "C:\\d\\f.bin", dirIds: [0], fileName: "f.bin" });
    expect([...view.done]).toEqual([1]);
    expect(view.result.items[1]?.dirIds).toEqual([0, 2]);
  });
});

describe("errorText", () => {
  it("keeps backend strings untouched", () => {
    expect(errorText("ไม่มี API key")).toBe("ไม่มี API key");
    expect(errorText(new Error("x"))).toBe("x");
  });
});
