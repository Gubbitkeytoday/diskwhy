import { Button, Sheet, SheetBody, SheetContent, SheetDescription, SheetHeader, SheetTitle, toast, useTheme } from "@nova/ui";
import { MoonIcon, PlugIcon, ScanSearchIcon, SunIcon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { McpPanel } from "./components/mcp-panel";
import { UpdateNotice } from "./components/update-notice";
import { ResultsScreen } from "./components/results";
import { HomeScreen, NotInAppScreen, ScanningScreen, StartupErrorScreen } from "./components/screens";
import { type Backend, getBackend } from "./lib/backend";
import { formatBytes, formatDuration } from "./lib/format";
import type { DriveInfo, ProgressEvent, Report } from "./lib/types";

type View = { name: "home" } | { name: "scanning"; path: string } | { name: "results"; path: string };

/** Message `getBackend()` rejects with when the page runs outside the desktop app. */
const NOT_IN_APP = "NOT_IN_APP";

/** Tauri rejects with plain strings; `getBackend()` rejects with an Error. */
const errorMessage = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function App() {
  const [backend, setBackend] = useState<Backend | null>(null);
  const [startupError, setStartupError] = useState<string | null>(null);
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [view, setView] = useState<View>({ name: "home" });
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [report, setReport] = useState<Report | null>(null);
  const [mcpOpen, setMcpOpen] = useState(false);
  const { resolvedTheme, setTheme } = useTheme();

  useEffect(() => {
    getBackend()
      .then(async (b) => {
        setBackend(b);
        setDrives(await b.listDrives());
      })
      .catch((e: unknown) => setStartupError(errorMessage(e)));
  }, []);

  const refreshReport = useCallback(async () => {
    if (backend) setReport(await backend.getReport());
  }, [backend]);

  const scan = useCallback(
    async (path: string) => {
      if (!backend) return;
      setProgress(null);
      setView({ name: "scanning", path });
      try {
        const done = await backend.scan(path, setProgress);
        if (done.cancelled) {
          toast("หยุดสแกนแล้ว");
          setView({ name: "home" });
          return;
        }
        setReport(await backend.getReport());
        setView({ name: "results", path });
        toast.success("สแกนเสร็จแล้ว", { description: `${formatBytes(done.stats.bytes)} ใน ${formatDuration(done.stats.elapsedMs)}` });
      } catch (e) {
        toast.error(String(e));
        setView({ name: "home" });
      }
    },
    [backend],
  );

  const cancel = useCallback(() => void backend?.cancelScan(), [backend]);

  useEffect(() => {
    if (view.name !== "scanning") return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && cancel();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view.name, cancel]);

  const pick = async () => {
    const folder = await backend?.pickFolder();
    if (folder) void scan(folder);
  };

  return (
    <div className="flex min-h-full flex-col bg-muted/30">
      <header className="sticky top-0 z-20 flex h-12 items-center gap-2 border-b bg-background/85 px-4 backdrop-blur-md">
        <span className="grid size-7 place-items-center rounded-lg bg-primary text-primary-foreground">
          <ScanSearchIcon className="size-4" aria-hidden />
        </span>
        <span className="font-semibold tracking-tight">DiskWhy</span>
        <span className="text-sm text-muted-foreground">· ดิสก์เต็มเพราะอะไร</span>
        {backend && (
          <Button className="ml-auto" size="sm" variant="outline" onClick={() => setMcpOpen(true)}>
            <PlugIcon /> เชื่อมต่อกับ Claude
          </Button>
        )}
        <Button
          className={backend ? undefined : "ml-auto"}
          size="icon-sm"
          variant="ghost"
          aria-label={resolvedTheme === "dark" ? "เปลี่ยนเป็นธีมสว่าง" : "เปลี่ยนเป็นธีมมืด"}
          onClick={() => setTheme(resolvedTheme === "dark" ? "light" : "dark")}
        >
          {resolvedTheme === "dark" ? <SunIcon /> : <MoonIcon />}
        </Button>
      </header>
      <UpdateNotice />
      <main className="flex flex-1">
        {startupError === NOT_IN_APP ? (
          <NotInAppScreen />
        ) : startupError !== null ? (
          <StartupErrorScreen message={startupError} />
        ) : (
          <>
            {view.name === "home" && <HomeScreen drives={drives} loading={!backend} onScan={scan} onPick={pick} />}
            {view.name === "scanning" && <ScanningScreen path={view.path} progress={progress} onCancel={cancel} />}
            {view.name === "results" && backend && report && (
              <ResultsScreen
                backend={backend}
                report={report}
                onRescan={() => scan(view.path)}
                onHome={() => {
                  setView({ name: "home" });
                  void backend.listDrives().then(setDrives);
                }}
                onReportChange={refreshReport}
              />
            )}
          </>
        )}
      </main>
      {backend && (
        <Sheet open={mcpOpen} onOpenChange={setMcpOpen}>
          <SheetContent closeLabel="ปิด" className="w-[min(44rem,calc(100%-3rem))]">
            <SheetHeader>
              <SheetTitle>เชื่อมต่อ Claude (MCP)</SheetTitle>
              <SheetDescription>ให้ Claude ที่คุณใช้อยู่ช่วยตรวจและเคลียร์ดิสก์ผ่าน DiskWhy</SheetDescription>
            </SheetHeader>
            <SheetBody>
              <McpPanel backend={backend} />
            </SheetBody>
          </SheetContent>
        </Sheet>
      )}
    </div>
  );
}
