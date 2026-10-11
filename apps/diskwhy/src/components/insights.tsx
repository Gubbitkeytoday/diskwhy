import { Badge, Button, Card, CardContent, CardDescription, CardHeader, CardTitle, cn } from "@nova/ui";
import { ChevronDownIcon, ClockIcon, FolderIcon, MapIcon, SparklesIcon } from "lucide-react";
import { useState } from "react";
import { CATEGORY_META, formatAge, formatBytes, formatNumber, percent, SAFETY_META } from "../lib/format";
import type { Hog, Location, Report } from "../lib/types";

export function Verdict({ report }: { report: Report }) {
  const safe = report.hogs.filter((h) => h.safety === "safe").reduce((s, h) => s + h.size, 0);
  const top = report.hogs[0];
  const topCat = report.categories.find((c) => c.category !== "other"); // "other" says nothing useful
  return (
    <Card className="gap-3 border-primary/30 bg-gradient-to-br from-accent to-card">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-lg">
          <SparklesIcon className="size-5 text-primary" aria-hidden /> สรุป: ดิสก์เต็มเพราะอะไร?
        </CardTitle>
      </CardHeader>
      <CardContent className="grid gap-2 text-sm leading-relaxed">
        {top && (
          <p>
            ตัวการอันดับ 1 คือ <strong>{top.label}</strong> ใช้ไป <strong className="tabular-nums">{formatBytes(top.size)}</strong> (
            {percent(top.size, report.total).toFixed(0)}% ของที่สแกน)
          </p>
        )}
        {topCat && (
          <p>
            ประเภทไฟล์ที่กินที่มากที่สุดคือ <strong>{CATEGORY_META[topCat.category].label}</strong> รวม{" "}
            <strong className="tabular-nums">{formatBytes(topCat.bytes)}</strong>
          </p>
        )}
        <p>
          คืนพื้นที่ได้อย่างปลอดภัยทันทีประมาณ <strong className="text-success tabular-nums">{formatBytes(safe)}</strong> จากแคช ไฟล์ชั่วคราว
          และไฟล์ที่สร้างใหม่ได้
        </p>
      </CardContent>
    </Card>
  );
}

/** Hogs that mean "not used for a long time" (found by last-modified age, not by what they are). */
const STALE_HOG_IDS = new Set(["stale-projects", "stale-large-files", "old-installers", "orphaned-appdata"]);

function HogCard({ hog, total, onShow, onReveal }: { hog: Hog; total: number; onShow: (l: Location) => void; onReveal: (path: string) => void }) {
  const [open, setOpen] = useState(false);
  const meta = SAFETY_META[hog.safety];
  return (
    <li className="rounded-lg border bg-card">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className="flex w-full cursor-pointer items-start gap-3 rounded-lg p-4 text-left hover:bg-muted/40 focus-visible:ring-[3px] focus-visible:ring-ring/45 focus-visible:outline-none"
      >
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-medium">{hog.label}</span>
            <Badge variant={meta.variant} title={meta.hint}>
              {meta.label}
            </Badge>
            {STALE_HOG_IDS.has(hog.id) && (
              <Badge variant="outline" title="ตรวจพบจากวันที่ใช้งานล่าสุด">
                <ClockIcon /> ไม่ได้ใช้นาน
              </Badge>
            )}
            {hog.count > 1 && <span className="text-xs text-muted-foreground">{formatNumber(hog.count)} ตำแหน่ง</span>}
          </div>
          <p className="mt-1 text-sm text-muted-foreground">{hog.advice}</p>
          <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-muted">
            <div className="h-full rounded-full bg-primary" style={{ width: `${Math.max(1, percent(hog.size, total))}%` }} />
          </div>
        </div>
        <div className="shrink-0 text-right">
          <div className="text-lg font-semibold tabular-nums">{formatBytes(hog.size)}</div>
          <ChevronDownIcon className={cn("ml-auto size-4 text-muted-foreground transition-transform", open && "rotate-180")} aria-hidden />
        </div>
      </button>
      {open && (
        <ul className="divide-y border-t text-sm">
          {hog.locations.map((l) => (
            <li key={l.path} className="flex items-center gap-2 px-4 py-2">
              <span className="min-w-0 flex-1">
                <span className="block truncate font-mono text-xs" title={l.path}>
                  {l.path}
                </span>
                {(l.last_used || l.note) && (
                  <span className="mt-0.5 block text-xs text-muted-foreground">
                    {l.last_used ? `ใช้ล่าสุด ${formatAge(l.last_used)}` : null}
                    {l.last_used && l.note ? " · " : null}
                    {l.note}
                  </span>
                )}
              </span>
              <span className="w-20 shrink-0 text-right text-xs tabular-nums">{formatBytes(l.size)}</span>
              {l.ids.length > 0 && (
                <Button size="sm" variant="ghost" onClick={() => onShow(l)}>
                  <MapIcon /> ดูในแผนผัง
                </Button>
              )}
              <Button size="icon-sm" variant="ghost" aria-label={`แสดง ${l.path} ใน File Explorer`} onClick={() => onReveal(l.path)}>
                <FolderIcon />
              </Button>
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}

export function HogList({ report, onShow, onReveal }: { report: Report; onShow: (l: Location) => void; onReveal: (p: string) => void }) {
  if (!report.hogs.length) {
    return <p className="rounded-lg border p-6 text-center text-sm text-muted-foreground">ไม่พบตัวการที่รู้จักในโฟลเดอร์นี้ — ลองดูแท็บ "สำรวจ" หรือ "ไฟล์ใหญ่สุด"</p>;
  }
  return (
    <ul className="grid gap-3">
      {report.hogs.map((h) => (
        <HogCard key={`${h.id}-${h.label}`} hog={h} total={report.total} onShow={onShow} onReveal={onReveal} />
      ))}
    </ul>
  );
}

export function CategoryBreakdown({ report }: { report: Report }) {
  const cats = report.categories.filter((c) => c.bytes > 0);
  const sum = cats.reduce((s, c) => s + c.bytes, 0);
  return (
    <Card className="gap-4">
      <CardHeader>
        <CardTitle className="text-base">แยกตามประเภทไฟล์</CardTitle>
        <CardDescription>ดูว่าพื้นที่หมดไปกับวิดีโอ รูป เกม หรืออะไร</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="flex h-3 overflow-hidden rounded-full" role="img" aria-label="สัดส่วนประเภทไฟล์">
          {cats.map((c) => (
            <div key={c.category} style={{ width: `${percent(c.bytes, sum)}%`, background: CATEGORY_META[c.category].color }} title={CATEGORY_META[c.category].label} />
          ))}
        </div>
        <ul className="grid gap-2 text-sm">
          {cats.map((c) => (
            <li key={c.category} className="flex items-center gap-2">
              <span className="size-2.5 shrink-0 rounded-sm" style={{ background: CATEGORY_META[c.category].color }} />
              <span className="flex-1 truncate">{CATEGORY_META[c.category].label}</span>
              <span className="text-xs text-muted-foreground tabular-nums">{formatNumber(c.count)} ไฟล์</span>
              <span className="w-20 text-right font-medium tabular-nums">{formatBytes(c.bytes)}</span>
            </li>
          ))}
        </ul>
      </CardContent>
    </Card>
  );
}
