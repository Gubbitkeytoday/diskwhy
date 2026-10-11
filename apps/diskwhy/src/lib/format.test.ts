import { describe, expect, it } from "vitest";
import { createDemoBackend } from "./demo";
import { formatBytes, percent } from "./format";

describe("formatBytes", () => {
  it("uses binary units like Windows Explorer", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1023)).toBe("1,023 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(476 * 1024 ** 3)).toBe("476 GB");
    expect(percent(1, 0)).toBe(0);
  });
});

describe("demo backend", () => {
  it("scans, reports hogs and frees space on trash", async () => {
    const b = createDemoBackend();
    const progress: number[] = [];
    const done = await b.scan("C:\\", (p) => progress.push(p.files));
    expect(done.cancelled).toBe(false);
    expect(progress.length).toBeGreaterThan(5);

    const report = await b.getReport();
    const ids = report.hogs.map((h) => h.id);
    expect(ids).toEqual(expect.arrayContaining(["temp", "node-modules", "recycle-bin", "virtual-disks", "windows-old", "system-file"]));
    const nm = report.hogs.find((h) => h.id === "node-modules");
    expect(nm?.count).toBe(5);

    const loc = report.hogs.find((h) => h.id === "temp" && h.locations[0]?.path.includes("AppData"))?.locations[0];
    expect(loc).toBeDefined();
    const before = (await b.getNode([])).size;
    const freed = await b.moveToTrash({ path: loc?.path ?? "", dirIds: loc?.ids ?? [] });
    expect(freed).toBe(loc?.size);
    expect((await b.getNode([])).size).toBe(before - freed);
  });
});
