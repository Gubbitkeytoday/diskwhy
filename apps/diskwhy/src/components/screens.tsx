import { Badge, Button, Card, CardContent, cn, Kbd, Skeleton } from "@nova/ui";
import { FolderSearchIcon, HardDriveIcon, ScanSearchIcon, SquareIcon, UsbIcon } from "lucide-react";
import { formatBytes, formatDuration, formatNumber, percent } from "../lib/format";
import type { DriveInfo, ProgressEvent } from "../lib/types";

export function UsageBar({ used, total, className }: { used: number; total: number; className?: string }) {
  const p = percent(used, total);
  const tone = p >= 90 ? "bg-destructive" : p >= 75 ? "bg-warning" : "bg-primary";
  return (
    <div className={cn("h-2.5 overflow-hidden rounded-full bg-muted", className)} role="meter" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(p)} aria-label="พื้นที่ที่ใช้ไป">
      <div className={cn("h-full rounded-full transition-[width] duration-500", tone)} style={{ width: `${p}%` }} />
    </div>
  );
}

export function HomeScreen({
  drives,
  loading,
  onScan,
  onPick,
  demo,
}: {
  drives: DriveInfo[];
  loading: boolean;
  onScan: (path: string) => void;
  onPick: () => void;
  demo: boolean;
}) {
  return (
    <div className="mx-auto grid w-full max-w-4xl gap-8 px-6 py-10">
      <div className="text-center">
        <div className="mx-auto mb-4 grid size-14 place-items-center rounded-2xl bg-primary text-primary-foreground shadow-lg">
          <ScanSearchIcon className="size-7" aria-hidden />
        </div>
        <h1 className="text-3xl font-semibold tracking-tight">ดิสก์เต็มเพราะอะไร?</h1>
        <p className="mx-auto mt-2 max-w-lg text-muted-foreground">
          เลือกไดรฟ์ที่ต้องการตรวจ แล้ว DiskWhy จะบอกว่าอะไรกินพื้นที่ อะไรลบได้อย่างปลอดภัย และควรจัดการอย่างไร
        </p>
        {demo && (
          <Badge variant="warning" className="mt-3">
            โหมดสาธิต — ข้อมูลจำลอง (เปิดเป็นแอปเดสก์ท็อปเพื่อสแกนเครื่องจริง)
          </Badge>
        )}
      </div>

      <section aria-label="ไดรฟ์ในเครื่อง" className="grid gap-3 sm:grid-cols-2">
        {loading
          ? Array.from({ length: 2 }, (_, i) => <Skeleton key={i} className="h-36 rounded-xl" />)
          : drives.map((d) => {
              const used = d.total - d.available;
              const p = percent(used, d.total);
              const Icon = d.removable ? UsbIcon : HardDriveIcon;
              return (
                <button
                  key={d.mount}
                  type="button"
                  onClick={() => onScan(d.mount)}
                  className="group cursor-pointer rounded-xl text-left focus-visible:ring-[3px] focus-visible:ring-ring/45 focus-visible:outline-none"
                >
                  <Card className="gap-3 px-5 py-5 transition-[border-color,box-shadow] group-hover:border-primary/60 group-hover:shadow-md">
                    <div className="flex items-center gap-3">
                      <span className="grid size-10 place-items-center rounded-lg bg-accent text-accent-foreground">
                        <Icon className="size-5" aria-hidden />
                      </span>
                      <div className="min-w-0 flex-1">
                        <div className="font-semibold">
                          {d.name || "Local Disk"} ({d.mount.replace(/\\$/, "")})
                        </div>
                        <div className="text-xs text-muted-foreground">{d.fileSystem}</div>
                      </div>
                      {p >= 90 && <Badge variant="destructive">ใกล้เต็ม</Badge>}
                    </div>
                    <UsageBar used={used} total={d.total} />
                    <div className="flex justify-between text-sm">
                      <span>
                        ว่าง <strong className="tabular-nums">{formatBytes(d.available)}</strong>
                      </span>
                      <span className="text-muted-foreground tabular-nums">
                        ใช้ {formatBytes(used)} จาก {formatBytes(d.total)}
                      </span>
                    </div>
                    <span className="text-sm font-medium text-muted-foreground transition-colors group-hover:text-primary group-focus-visible:text-primary">
                      สแกนไดรฟ์นี้ →
                    </span>
                  </Card>
                </button>
              );
            })}
      </section>

      <div className="flex flex-col items-center gap-2">
        <Button variant="outline" size="lg" onClick={onPick}>
          <FolderSearchIcon /> เลือกโฟลเดอร์เอง…
        </Button>
        <p className="text-xs text-muted-foreground">
          แอปอ่านข้อมูลอย่างเดียว ไม่ส่งข้อมูลออกนอกเครื่อง และลบไฟล์ได้เฉพาะเมื่อคุณยืนยัน (ย้ายไปถังขยะ กู้คืนได้)
        </p>
      </div>
    </div>
  );
}

export function ScanningScreen({ path, progress, onCancel }: { path: string; progress: ProgressEvent | null; onCancel: () => void }) {
  const rate = progress && progress.elapsedMs > 0 ? Math.round((progress.files / progress.elapsedMs) * 1000) : 0;
  return (
    <div className="mx-auto grid w-full max-w-2xl gap-6 px-6 py-16">
      <div className="text-center">
        <div className="relative mx-auto mb-5 size-16">
          <span className="absolute inset-0 animate-ping rounded-full bg-primary/25" />
          <span className="relative grid size-16 place-items-center rounded-full bg-primary text-primary-foreground">
            <ScanSearchIcon className="size-7" aria-hidden />
          </span>
        </div>
        <h1 className="text-2xl font-semibold">กำลังสแกน {path}</h1>
        <p className="mt-1 text-sm text-muted-foreground">ใช้ทุกคอร์ของ CPU — ดิสก์ใหญ่อาจใช้เวลาสักครู่</p>
      </div>
      <Card>
        <CardContent className="grid gap-5" aria-live="polite">
          <div className="grid grid-cols-3 gap-4 text-center">
            {[
              ["ไฟล์", formatNumber(progress?.files ?? 0)],
              ["ขนาดที่พบ", formatBytes(progress?.bytes ?? 0)],
              ["เวลา", formatDuration(progress?.elapsedMs ?? 0)],
            ].map(([label, value]) => (
              <div key={label}>
                <div className="text-2xl font-semibold tabular-nums">{value}</div>
                <div className="text-xs text-muted-foreground">{label}</div>
              </div>
            ))}
          </div>
          <div className="h-1.5 overflow-hidden rounded-full bg-muted">
            <div className="h-full w-1/3 animate-[nv-indeterminate_1.2s_ease-in-out_infinite] rounded-full bg-primary" />
          </div>
          <div className="truncate text-left font-mono text-xs text-muted-foreground" dir="rtl" title={progress?.current}>
            <bdi>{progress?.current || "…"}</bdi>
          </div>
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span className="tabular-nums">{formatNumber(rate)} ไฟล์/วินาที</span>
            <span className="tabular-nums">{formatNumber(progress?.dirs ?? 0)} โฟลเดอร์</span>
          </div>
        </CardContent>
      </Card>
      <div className="flex justify-center">
        <Button variant="outline" onClick={onCancel}>
          <SquareIcon /> หยุดสแกน <Kbd className="ml-1">Esc</Kbd>
        </Button>
      </div>
    </div>
  );
}
