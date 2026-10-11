import type { Category, Safety } from "./types";

const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"];

/** Binary units (1 KB = 1024 B) — matches what Windows Explorer shows. */
export function formatBytes(bytes: number, digits = 1): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const i = Math.min(UNITS.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const v = bytes / 1024 ** i;
  return `${v.toLocaleString("th-TH", { maximumFractionDigits: i === 0 ? 0 : v >= 100 ? 0 : digits })} ${UNITS[i]}`;
}

export const formatNumber = (n: number) => n.toLocaleString("th-TH");

export function formatDuration(ms: number): string {
  const s = Math.round(ms / 100) / 10;
  if (s < 60) return `${s.toLocaleString("th-TH")} วินาที`;
  const m = Math.floor(s / 60);
  return `${m} นาที ${Math.round(s % 60)} วินาที`;
}

export const percent = (part: number, whole: number) => (whole > 0 ? (part / whole) * 100 : 0);

export const CATEGORY_META: Record<Category, { label: string; color: string }> = {
  video: { label: "วิดีโอ", color: "oklch(0.62 0.2 25)" },
  image: { label: "รูปภาพ", color: "oklch(0.7 0.17 70)" },
  audio: { label: "เสียง/เพลง", color: "oklch(0.68 0.17 330)" },
  archive: { label: "ไฟล์บีบอัด", color: "oklch(0.6 0.16 300)" },
  "disk-image": { label: "ดิสก์อิมเมจ / VM", color: "oklch(0.55 0.18 277)" },
  installer: { label: "ไฟล์ติดตั้ง", color: "oklch(0.62 0.15 245)" },
  document: { label: "เอกสาร", color: "oklch(0.65 0.15 152)" },
  system: { label: "ไฟล์ระบบ", color: "oklch(0.55 0.02 262)" },
  "temp-log": { label: "ชั่วคราว / ล็อก", color: "oklch(0.75 0.13 95)" },
  code: { label: "โค้ด / นักพัฒนา", color: "oklch(0.62 0.13 200)" },
  game: { label: "เกม", color: "oklch(0.6 0.19 145)" },
  database: { label: "ฐานข้อมูล / อีเมล", color: "oklch(0.58 0.12 40)" },
  other: { label: "อื่น ๆ", color: "oklch(0.7 0.01 262)" },
};

export const SAFETY_META: Record<Safety, { label: string; variant: "success" | "warning" | "secondary"; hint: string }> = {
  safe: { label: "ลบได้ปลอดภัย", variant: "success", hint: "สร้างใหม่หรือดาวน์โหลดใหม่ได้" },
  review: { label: "ตรวจสอบก่อน", variant: "warning", hint: "อาจเป็นข้อมูลส่วนตัว" },
  system: { label: "ใช้เครื่องมือ Windows", variant: "secondary", hint: "ห้ามลบเอง" },
};

/** Colour for a treemap tile. */
export function tileColor(kind: string, category: Category | null, index: number): string {
  if (kind === "file" && category) return CATEGORY_META[category].color;
  if (kind === "small" || kind === "rest") return "var(--nv-color-muted)";
  const palette = ["var(--nv-color-chart-1)", "var(--nv-color-chart-4)", "var(--nv-color-chart-2)", "var(--nv-color-chart-3)", "var(--nv-color-chart-5)"];
  return palette[index % palette.length] as string;
}
