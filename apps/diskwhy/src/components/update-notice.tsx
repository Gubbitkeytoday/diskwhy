import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  toast,
} from "@nova/ui";
import { DownloadIcon, RotateCwIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { isNative } from "../lib/backend";
import { formatBytes } from "../lib/format";

type UpdateHandle = Awaited<ReturnType<typeof import("@tauri-apps/plugin-updater").check>>;

/**
 * Checks GitHub Releases for a newer signed version once per launch. If one exists, offers it in a dialog;
 * the updater verifies the signature before installing, then the app restarts on the new version.
 * Offline or no update → silent.
 */
export function UpdateNotice() {
  const [update, setUpdate] = useState<NonNullable<UpdateHandle> | null>(null);
  const [open, setOpen] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [installing, setInstalling] = useState(false);

  useEffect(() => {
    if (!isNative) return;
    let cancelled = false;
    // Let the first screen settle before touching the network.
    const t = setTimeout(async () => {
      try {
        const { check } = await import("@tauri-apps/plugin-updater");
        const found = await check();
        if (!cancelled && found) {
          setUpdate(found);
          setOpen(true);
        }
      } catch {
        /* offline / endpoint unreachable — try again next launch */
      }
    }, 3000);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, []);

  const install = async () => {
    if (!update) return;
    setInstalling(true);
    let done = 0;
    let total = 0;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        if (event.event === "Progress") done += event.data.chunkLength;
        setProgress({ done, total });
      });
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch (e) {
      setInstalling(false);
      toast.error("อัปเดตไม่สำเร็จ", { description: String(e) });
    }
  };

  if (!update) return null;
  const pct = progress?.total ? Math.round((progress.done / progress.total) * 100) : null;

  return (
    <Dialog open={open} onOpenChange={(o) => !installing && setOpen(o)}>
      <DialogContent closeLabel="ปิด" hideClose={installing}>
        <DialogHeader>
          <DialogTitle>มี DiskWhy เวอร์ชันใหม่ {update.version}</DialogTitle>
          <DialogDescription>
            เวอร์ชันที่ใช้อยู่ {update.currentVersion} — อัปเดตใช้เวลาไม่นาน แอปจะตรวจลายเซ็นไฟล์ก่อนติดตั้ง แล้วเปิดใหม่ให้อัตโนมัติ
          </DialogDescription>
        </DialogHeader>
        {update.body && (
          <div className="max-h-48 overflow-y-auto whitespace-pre-wrap rounded-md bg-muted px-3 py-2 text-sm">{update.body}</div>
        )}
        {installing && (
          <div className="grid gap-1.5" aria-live="polite">
            <div className="h-2 overflow-hidden rounded-full bg-muted">
              <div className="h-full rounded-full bg-primary transition-[width]" style={{ width: `${pct ?? 15}%` }} />
            </div>
            <span className="text-xs text-muted-foreground tabular-nums">
              {progress?.total ? `${formatBytes(progress.done)} / ${formatBytes(progress.total)} (${pct}%)` : "กำลังดาวน์โหลด…"}
            </span>
          </div>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)} disabled={installing}>
            ไว้ทีหลัง
          </Button>
          <Button onClick={install} loading={installing}>
            {installing ? <RotateCwIcon /> : <DownloadIcon />} อัปเดตและเปิดใหม่
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
