// In-browser demo backend: a realistic simulated Windows C: drive. Mirrors the Rust logic closely enough
// that every screen (progress, insights, treemap, trash) behaves like the real app.
import type { Backend } from "./backend";
import type { Category, ChildView, Hog, NodeView, Report, Safety, TopFile } from "./types";

const GB = 1024 ** 3;
const MB = 1024 ** 2;

interface DNode {
  name: string;
  size: number;
  files: number;
  dirs: number;
  children: DNode[];
  bigFiles: { name: string; size: number }[];
  smallCount: number;
  smallSize: number;
}

type Spec = { [name: string]: Spec | number | [number, number] };
// number  → a big file of that size
// [bytes, count] under key "*" → aggregated small files

function build(name: string, spec: Spec): DNode {
  const node: DNode = { name, size: 0, files: 0, dirs: 0, children: [], bigFiles: [], smallCount: 0, smallSize: 0 };
  for (const [k, v] of Object.entries(spec)) {
    if (k === "*" && Array.isArray(v)) {
      node.smallSize += v[0];
      node.smallCount += v[1];
    } else if (typeof v === "number") node.bigFiles.push({ name: k, size: v });
    else if (!Array.isArray(v)) node.children.push(build(k, v));
  }
  finalize(node);
  return node;
}

function finalize(n: DNode) {
  n.children.sort((a, b) => b.size - a.size);
  n.bigFiles.sort((a, b) => b.size - a.size);
  n.size = n.smallSize + n.bigFiles.reduce((s, f) => s + f.size, 0) + n.children.reduce((s, c) => s + c.size, 0);
  n.files = n.smallCount + n.bigFiles.length + n.children.reduce((s, c) => s + c.files, 0);
  n.dirs = n.children.length + n.children.reduce((s, c) => s + c.dirs, 0);
}

const nodeModules = (mb: number): Spec => ({ "*": [mb * MB, Math.round(mb * 180)], ".pnpm": { "*": [mb * 0.4 * MB, Math.round(mb * 90)] } });

const DEMO: Spec = {
  "pagefile.sys": 12 * GB,
  "hiberfil.sys": 6.4 * GB,
  "swapfile.sys": 256 * MB,
  "*": [2 * MB, 14],
  Windows: {
    WinSxS: { "*": [9.8 * GB, 84000] },
    System32: { "*": [6.1 * GB, 21000], DriverStore: { "*": [3.2 * GB, 9000] } },
    SoftwareDistribution: { Download: { "*": [3.4 * GB, 2100] } },
    Temp: { "*": [1.1 * GB, 3400] },
    Installer: { "*": [4.2 * GB, 1200] },
    "*": [2.3 * GB, 12000],
  },
  "Windows.old": { "*": [18.6 * GB, 140000] },
  "Program Files": {
    "Adobe": { "*": [8.4 * GB, 31000] },
    "Microsoft Office": { "*": [4.1 * GB, 8000] },
    "Google": { "*": [1.2 * GB, 900] },
    "*": [5.6 * GB, 22000],
  },
  "Program Files (x86)": {
    Steam: { steamapps: { common: { "Cyberpunk 2077": { "*": [71 * GB, 4100] }, "Counter-Strike 2": { "*": [34 * GB, 2900] } } } },
    "*": [3.2 * GB, 11000],
  },
  ProgramData: { "*": [6.8 * GB, 18000] },
  "$Recycle.Bin": { "*": [9.3 * GB, 640] },
  Users: {
    somchai: {
      Downloads: {
        "Windows11_24H2.iso": 6.1 * GB,
        "Office_Setup.exe": 4.2 * GB,
        "DaVinci_Resolve_19.zip": 3.6 * GB,
        "vacation-2025-raw.zip": 2.8 * GB,
        "*": [5.4 * GB, 1830],
      },
      Videos: {
        "งานแต่ง-พี่หมิว-4K.mp4": 21.4 * GB,
        "trip-chiangmai.mov": 8.7 * GB,
        "*": [12.6 * GB, 320],
      },
      Pictures: { "*": [24.3 * GB, 18400], "iPhone backup": { "*": [11.2 * GB, 9100] } },
      Documents: { "*": [6.2 * GB, 7400], "Outlook Files": { "somchai@company.pst": 7.9 * GB } },
      OneDrive: { "*": [3.1 * GB, 4200] },
      code: {
        "shop-web": { node_modules: nodeModules(890), ".next": { "*": [640 * MB, 2100] }, "*": [42 * MB, 600] },
        "admin-panel": { node_modules: nodeModules(1240), "*": [31 * MB, 420] },
        "mobile-app": { node_modules: nodeModules(1630), android: { build: { "*": [2.1 * GB, 8000] } }, "*": [60 * MB, 900] },
        "old-projects": {
          "landing-2023": { node_modules: nodeModules(540) },
          "api-2022": { node_modules: nodeModules(720) },
        },
      },
      ".gradle": { caches: { "*": [6.3 * GB, 52000] } },
      ".nuget": { packages: { "*": [2.4 * GB, 18000] } },
      AppData: {
        Local: {
          Temp: { "*": [14.8 * GB, 46000] },
          "npm-cache": { "*": [4.6 * GB, 38000] },
          pnpm: { store: { "*": [7.2 * GB, 120000] } },
          pip: { cache: { "*": [1.9 * GB, 6100] } },
          Docker: { wsl: { disk: { "docker_data.vhdx": 38.4 * GB } } },
          Packages: { "CanonicalGroupLimited.Ubuntu": { LocalState: { "ext4.vhdx": 22.6 * GB } } },
          Google: { Chrome: { "User Data": { Default: { Cache: { "*": [2.7 * GB, 9000] }, "Code Cache": { "*": [810 * MB, 4000] } } } } },
          Microsoft: { Edge: { "User Data": { Default: { Cache: { "*": [1.4 * GB, 5200] } } } } },
          CrashDumps: { "*": [1.6 * GB, 12] },
          "*": [3.9 * GB, 21000],
        },
        Roaming: { LINE: { Data: { "*": [5.8 * GB, 22000] } }, "*": [2.6 * GB, 9000] },
      },
      "*": [120 * MB, 40],
    },
    Public: { "*": [900 * MB, 300] },
  },
};

interface Rule {
  id: string;
  label: string;
  advice: string;
  safety: Safety;
  test: (p: string, n: string) => boolean;
}

// Same rules as src-tauri/src/insights.rs (subset of wording).
const RULES: Rule[] = [
  { id: "recycle-bin", label: "ถังขยะ (Recycle Bin)", advice: "ไฟล์ที่ลบแล้วยังกินพื้นที่อยู่ — คลิกขวาที่ Recycle Bin แล้วเลือก Empty Recycle Bin", safety: "safe", test: (_, n) => n === "$recycle.bin" },
  { id: "temp", label: "ไฟล์ชั่วคราว (Temp)", advice: "ลบได้อย่างปลอดภัยผ่าน Settings › System › Storage › Temporary files หรือ Disk Cleanup", safety: "safe", test: (p, n) => n === "temp" && (p.endsWith("\\appdata\\local\\temp") || p.endsWith("\\windows\\temp")) },
  { id: "windows-update", label: "ไฟล์ดาวน์โหลดของ Windows Update", advice: "ล้างด้วย Disk Cleanup › Clean up system files › Windows Update Cleanup", safety: "system", test: (p) => p.endsWith("\\windows\\softwaredistribution\\download") },
  { id: "windows-old", label: "Windows เวอร์ชันเก่า (Windows.old)", advice: "ถ้าไม่ต้องการย้อนกลับเวอร์ชัน ให้ลบผ่าน Disk Cleanup › Previous Windows installation(s)", safety: "system", test: (p, n) => n === "windows.old" && p.length <= 15 },
  { id: "winsxs", label: "คลังคอมโพเนนต์ Windows (WinSxS)", advice: "ห้ามลบเอง — รัน 'DISM /Online /Cleanup-Image /StartComponentCleanup'", safety: "system", test: (p) => p.endsWith("\\windows\\winsxs") },
  { id: "node-modules", label: "node_modules (โปรเจกต์ JavaScript)", advice: "ติดตั้งใหม่ได้ด้วย npm/pnpm install — ลบของโปรเจกต์ที่ไม่ได้ใช้แล้วได้เลย", safety: "safe", test: (_, n) => n === "node_modules" },
  {
    id: "dev-caches",
    label: "แคชของเครื่องมือนักพัฒนา",
    advice: "ดาวน์โหลดใหม่ได้อัตโนมัติ — เช่น npm cache clean --force, pnpm store prune, pip cache purge",
    safety: "safe",
    test: (p) => ["\\appdata\\local\\npm-cache", "\\appdata\\local\\pnpm\\store", "\\appdata\\local\\pip\\cache", "\\.gradle\\caches", "\\.nuget\\packages"].some((s) => p.endsWith(s)),
  },
  {
    id: "browser-cache",
    label: "แคชเบราว์เซอร์",
    advice: "ล้างจากเมนูเบราว์เซอร์ (Ctrl+Shift+Delete › Cached images and files) ไม่กระทบรหัสผ่านหรือบุ๊กมาร์ก",
    safety: "safe",
    test: (p, n) => (n === "cache" || n === "code cache") && (p.includes("\\google\\chrome\\user data\\") || p.includes("\\microsoft\\edge\\user data\\")),
  },
  { id: "downloads", label: "โฟลเดอร์ดาวน์โหลด (Downloads)", advice: "มักมีไฟล์ติดตั้งและไฟล์ ZIP ที่ใช้แล้ว — เรียงตามขนาดแล้วลบสิ่งที่ไม่ต้องการ", safety: "review", test: (p, n) => n === "downloads" && p.includes("\\users\\") && p.split("\\").length === 4 },
  { id: "games", label: "เกม (Steam / Epic / Xbox)", advice: "ถอนการติดตั้งเกมที่ไม่ได้เล่นจากตัว launcher เพื่อให้ลบครบทุกไฟล์", safety: "review", test: (p) => p.endsWith("\\steamapps\\common") },
  { id: "crash-dumps", label: "ไฟล์ crash dump", advice: "ลบได้ผ่าน Disk Cleanup › System error memory dump files", safety: "safe", test: (p) => p.endsWith("\\appdata\\local\\crashdumps") },
];

const SYSTEM_FILES: Record<string, [string, string]> = {
  "hiberfil.sys": ["ไฟล์ไฮเบอร์เนต (hiberfil.sys)", "ถ้าไม่ใช้โหมด Hibernate รัน 'powercfg /h off' ในฐานะผู้ดูแลระบบ (ห้ามลบเอง)"],
  "pagefile.sys": ["หน่วยความจำเสมือน (pagefile.sys)", "Windows จัดการให้อัตโนมัติ ปรับได้ที่ System › Advanced › Performance › Virtual memory"],
  "swapfile.sys": ["ไฟล์ swap (swapfile.sys)", "Windows จัดการให้อัตโนมัติ — ไม่ควรลบ"],
};

export function categoryOf(name: string): Category {
  const ext = name.includes(".") && !name.startsWith(".") ? (name.split(".").pop() ?? "").toLowerCase() : "";
  if (["mp4", "mkv", "mov", "avi", "wmv", "webm"].includes(ext)) return "video";
  if (["jpg", "jpeg", "png", "heic", "raw", "psd"].includes(ext)) return "image";
  if (["zip", "rar", "7z"].includes(ext)) return "archive";
  if (["iso", "vhdx", "vhd", "vmdk"].includes(ext)) return "disk-image";
  if (["exe", "msi"].includes(ext)) return "installer";
  if (["pst", "ost", "db"].includes(ext)) return "database";
  if (["sys", "dll"].includes(ext)) return "system";
  return "other";
}

const ROOT_PATH = "C:\\";
const TOTAL = 476 * GB;

function findHogs(root: DNode): Hog[] {
  const hits = new Map<string, Hog>();
  const add = (r: { id: string; label: string; advice: string; safety: Safety }, path: string, size: number, ids: number[]) => {
    const h = hits.get(r.id) ?? { ...r, size: 0, count: 0, locations: [] };
    h.size += size;
    h.count += 1;
    h.locations.push({ path, size, ids });
    hits.set(r.id, h);
  };
  const visit = (n: DNode, path: string, ids: number[]) => {
    const rule = RULES.find((r) => r.test(path.toLowerCase(), n.name.toLowerCase()));
    if (rule) return add(rule, path, n.size, ids);
    n.children.forEach((c, i) => visit(c, `${path}\\${c.name}`, [...ids, i]));
  };
  root.children.forEach((c, i) => visit(c, `C:\\${c.name}`, [i]));
  for (const f of root.bigFiles) {
    const sys = SYSTEM_FILES[f.name.toLowerCase()];
    if (sys) hits.set(sys[0], { id: "system-file", label: sys[0], advice: sys[1], safety: "system", size: f.size, count: 1, locations: [{ path: `C:\\${f.name}`, size: f.size, ids: [] }] });
  }
  const vhd = (n: DNode, path: string, ids: number[]) => {
    for (const f of n.bigFiles)
      if (/\.(vhdx?|vmdk)$/i.test(f.name))
        add(
          { id: "virtual-disks", label: "ดิสก์เสมือน (WSL / Docker / VM)", advice: "ไฟล์ .vhdx ไม่หดเองเมื่อลบข้อมูลข้างใน — รัน 'wsl --shutdown' แล้วใช้ Optimize-VHD", safety: "review" },
          `${path}\\${f.name}`,
          f.size,
          ids,
        );
    n.children.forEach((c, i) => vhd(c, `${path}\\${c.name}`, [...ids, i]));
  };
  vhd(root, "C:", []);
  return [...hits.values()].map((h) => ({ ...h, locations: h.locations.sort((a, b) => b.size - a.size).slice(0, 20) })).sort((a, b) => b.size - a.size);
}

function topFiles(root: DNode): TopFile[] {
  const out: TopFile[] = [];
  const visit = (n: DNode, path: string, ids: number[]) => {
    for (const f of n.bigFiles) out.push({ path: `${path}\\${f.name}`, name: f.name, size: f.size, category: categoryOf(f.name), dir_ids: ids });
    n.children.forEach((c, i) => visit(c, `${path}\\${c.name}`, [...ids, i]));
  };
  visit(root, "C:", []);
  return out.sort((a, b) => b.size - a.size).slice(0, 200);
}

function categories(root: DNode) {
  const acc = new Map<Category, { bytes: number; count: number }>();
  const visit = (n: DNode) => {
    for (const f of n.bigFiles) {
      const c = categoryOf(f.name);
      const a = acc.get(c) ?? { bytes: 0, count: 0 };
      a.bytes += f.size;
      a.count += 1;
      acc.set(c, a);
    }
    const o = acc.get("other") ?? { bytes: 0, count: 0 };
    o.bytes += n.smallSize;
    o.count += n.smallCount;
    acc.set("other", o);
    n.children.forEach(visit);
  };
  visit(root);
  // Re-attribute well-known folders so the demo breakdown looks like a real machine.
  const shift = (from: Category, to: Category, bytes: number, count: number) => {
    const a = acc.get(from);
    if (!a) return;
    a.bytes -= bytes;
    a.count -= count;
    const b = acc.get(to) ?? { bytes: 0, count: 0 };
    b.bytes += bytes;
    b.count += count;
    acc.set(to, b);
  };
  shift("other", "game", 105 * GB, 7000);
  shift("other", "system", 30 * GB, 140000);
  shift("other", "image", 35 * GB, 27000);
  shift("other", "code", 18 * GB, 900000);
  shift("other", "temp-log", 16 * GB, 50000);
  shift("other", "video", 12 * GB, 300);
  return [...acc].map(([category, v]) => ({ category, ...v })).sort((a, b) => b.bytes - a.bytes);
}

function nodeView(root: DNode, ids: number[]): NodeView {
  let n = root;
  let path = "C:";
  const crumbs: [string, number[]][] = [[ROOT_PATH, []]];
  ids.forEach((i, d) => {
    const next = n.children[i];
    if (!next) throw new Error("ไม่พบโฟลเดอร์นี้ในผลสแกน");
    n = next;
    path += `\\${n.name}`;
    crumbs.push([n.name, ids.slice(0, d + 1)]);
  });
  let children: ChildView[] = [
    ...n.children.map((c, i) => ({ kind: "dir" as const, name: c.name, size: c.size, files: c.files, index: i, category: null, denied: false })),
    ...n.bigFiles.map((f) => ({ kind: "file" as const, name: f.name, size: f.size, files: 1, index: null, category: categoryOf(f.name), denied: false })),
  ];
  if (n.smallCount) children.push({ kind: "small", name: `ไฟล์ขนาดเล็ก ${n.smallCount.toLocaleString("th-TH")} ไฟล์`, size: n.smallSize, files: n.smallCount, index: null, category: null, denied: false });
  children = children.sort((a, b) => b.size - a.size);
  return { ids, path: ids.length ? path : ROOT_PATH, name: ids.length ? n.name : ROOT_PATH, size: n.size, files: n.files, dirs: n.dirs, crumbs, children };
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function createDemoBackend(): Backend {
  let root: DNode | null = null;
  let cancelled = false;
  let elapsed = 0;

  return {
    kind: "demo",
    async listDrives() {
      return [
        { mount: "C:\\", name: "Windows", fileSystem: "NTFS", total: TOTAL, available: 31.4 * GB, removable: false },
        { mount: "D:\\", name: "Data", fileSystem: "NTFS", total: 931 * GB, available: 402 * GB, removable: false },
        { mount: "E:\\", name: "USB Drive", fileSystem: "exFAT", total: 58 * GB, available: 21 * GB, removable: true },
      ];
    },
    async pickFolder() {
      return "C:\\Users\\somchai";
    },
    async scan(_path, onProgress) {
      cancelled = false;
      const full = build(ROOT_PATH, DEMO);
      const paths = ["C:\\Windows\\WinSxS\\amd64_microsoft-windows", "C:\\Program Files\\Adobe\\Photoshop", "C:\\Users\\somchai\\code\\mobile-app\\node_modules\\.pnpm", "C:\\Users\\somchai\\AppData\\Local\\Temp", "C:\\Users\\somchai\\Pictures\\iPhone backup", "C:\\Program Files (x86)\\Steam\\steamapps\\common"];
      const steps = 28;
      const started = performance.now();
      for (let i = 1; i <= steps; i++) {
        if (cancelled) return { scanId: 1, cancelled: true, stats: { files: 0, dirs: 0, bytes: 0, errors: 0, cloudBytes: 0, elapsedMs: 0 } };
        await sleep(90);
        const t = i / steps;
        const ease = 1 - (1 - t) ** 2;
        onProgress({
          scanId: 1,
          files: Math.round(full.files * ease),
          dirs: Math.round(full.dirs * ease),
          bytes: Math.round(full.size * ease),
          errors: Math.round(37 * ease),
          current: paths[i % paths.length] as string,
          elapsedMs: Math.round(performance.now() - started),
        });
      }
      root = full;
      elapsed = Math.round(performance.now() - started);
      return { scanId: 1, cancelled: false, stats: { files: full.files, dirs: full.dirs, bytes: full.size, errors: 37, cloudBytes: 3.1 * GB, elapsedMs: elapsed } };
    },
    async cancelScan() {
      cancelled = true;
    },
    async getReport(): Promise<Report> {
      if (!root) throw new Error("ยังไม่มีผลการสแกน");
      return {
        rootPath: ROOT_PATH,
        total: root.size,
        stats: { files: root.files, dirs: root.dirs, bytes: root.size, errors: 37, cloudBytes: 3.1 * GB, elapsedMs: elapsed },
        categories: categories(root),
        extensions: [],
        hogs: findHogs(root),
        topFiles: topFiles(root),
        drive: { mount: "C:\\", name: "Windows", fileSystem: "NTFS", total: TOTAL, available: TOTAL - root.size - 3 * GB, removable: false },
      };
    },
    async getNode(ids) {
      if (!root) throw new Error("ยังไม่มีผลการสแกน");
      return nodeView(root, ids);
    },
    async reveal() {
      /* no file manager in the browser demo */
    },
    async moveToTrash(t) {
      if (!root) return 0;
      await sleep(250);
      const chain: DNode[] = [root];
      for (const i of t.dirIds) {
        const next = chain.at(-1)?.children[i];
        if (!next) return 0;
        chain.push(next);
      }
      let freed = 0;
      if (t.fileName) {
        const dir = chain.at(-1) as DNode;
        const idx = dir.bigFiles.findIndex((f) => f.name === t.fileName);
        if (idx < 0) return 0;
        freed = dir.bigFiles.splice(idx, 1)[0]?.size ?? 0;
      } else {
        const removed = chain.pop() as DNode;
        const parent = chain.at(-1) as DNode;
        parent.children.splice(parent.children.indexOf(removed), 1);
        freed = removed.size;
      }
      for (const n of [...chain].reverse()) finalize(n);
      return freed;
    },
  };
}
