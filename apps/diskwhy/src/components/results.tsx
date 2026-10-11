import {
  Badge,
  Button,
  Card,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Skeleton,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  toast,
} from "@nova/ui";
import { ArrowLeftIcon, ChevronRightIcon, CloudIcon, FolderIcon, MapIcon, RotateCwIcon, ShieldAlertIcon, SparklesIcon, Trash2Icon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { TrashRequest } from "../lib/ai-logic";
import type { Backend } from "../lib/backend";
import { CATEGORY_META, formatAge, formatBytes, formatDuration, formatNumber, percent } from "../lib/format";
import type { ChildView, Location, NodeView, Report, TopFile } from "../lib/types";
import { AiPanel, useAiAdvisor } from "./ai-panel";
import { CategoryBreakdown, HogList, Verdict } from "./insights";
import { UsageBar } from "./screens";
import { SizeList } from "./size-list";
import { Treemap } from "./treemap";

type PendingTrash = TrashRequest;

export function ResultsScreen({
  backend,
  report,
  onRescan,
  onHome,
  onReportChange,
}: {
  backend: Backend;
  report: Report;
  onRescan: () => void;
  onHome: () => void;
  onReportChange: () => void;
}) {
  const [tab, setTab] = useState("why");
  const [ids, setIds] = useState<number[]>([]);
  const [node, setNode] = useState<NodeView | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [pending, setPending] = useState<PendingTrash | null>(null);
  const [busy, setBusy] = useState(false);
  // Lifted here so the AI result, progress and "moved" marks survive switching tabs.
  const advisor = useAiAdvisor(backend);

  const load = useCallback(
    async (next: number[]) => {
      try {
        setNode(await backend.getNode(next));
        setIds(next);
        setSelected(null);
      } catch (e) {
        toast.error(String(e));
      }
    },
    [backend],
  );

  useEffect(() => {
    void load([]);
  }, [load]);

  // Backspace / Alt+← goes up one level in the explorer.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;
      if (tab === "explore" && !typing && ids.length && (e.key === "Backspace" || (e.altKey && e.key === "ArrowLeft"))) {
        e.preventDefault();
        void load(ids.slice(0, -1));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [tab, ids, load]);

  const reveal = (path: string) => backend.reveal(path).catch((e) => toast.error(`เปิด Explorer ไม่สำเร็จ: ${e}`));
  const childPath = (c: ChildView) => `${node?.path.replace(/\\$/, "")}\\${c.name}`;

  const showIds = (locationIds: number[]) => {
    setTab("explore");
    void load(locationIds);
  };
  const showLocation = (l: Location) => showIds(l.ids);

  const askTrash = (c: ChildView) => {
    if (!node) return;
    setPending(
      c.kind === "dir"
        ? { path: childPath(c), dirIds: [...ids, c.index ?? 0], name: c.name, size: c.size }
        : { path: childPath(c), dirIds: ids, fileName: c.name, name: c.name, size: c.size },
    );
  };

  const askTrashFile = (f: TopFile) => setPending({ path: f.path, dirIds: f.dir_ids, fileName: f.name, name: f.name, size: f.size });

  const confirmTrash = async () => {
    if (!pending) return;
    setBusy(true);
    try {
      const freed = await backend.moveToTrash(pending);
      toast.success(`ย้าย "${pending.name}" ไปถังขยะแล้ว`, { description: `คืนพื้นที่ ${formatBytes(freed)} (กู้คืนได้จาก Recycle Bin)` });
      setPending(null);
      advisor.applyRemoval(pending); // marks AI rows as moved and re-indexes the others (directory indices shift)
      onReportChange();
      // Indices may shift after removal: reload the parent of what was removed.
      const parent = pending.fileName ? pending.dirIds : pending.dirIds.slice(0, -1);
      await load(parent);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  };

  const drive = report.drive;
  const used = drive ? drive.total - drive.available : 0;

  return (
    <div className="mx-auto grid w-full max-w-7xl gap-5 px-6 py-6">
      <div className="flex flex-wrap items-center gap-3">
        <Button variant="ghost" size="sm" onClick={onHome}>
          <ArrowLeftIcon /> เลือกไดรฟ์อื่น
        </Button>
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-xl font-semibold tracking-tight">ผลการสแกน {report.rootPath}</h1>
          <p className="text-xs text-muted-foreground">
            {formatNumber(report.stats.files)} ไฟล์ · {formatNumber(report.stats.dirs)} โฟลเดอร์ · ใช้เวลา {formatDuration(report.stats.elapsedMs)}
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={onRescan}>
          <RotateCwIcon /> สแกนใหม่
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-[2fr_1fr_1fr]">
        <Card className="gap-3 px-5 py-4">
          <div className="flex items-baseline justify-between text-sm">
            <span className="font-medium">{drive ? `ไดรฟ์ ${drive.mount.replace(/\\$/, "")}` : "โฟลเดอร์ที่สแกน"}</span>
            {drive && (
              <span className="text-muted-foreground tabular-nums">
                ว่าง <strong className="text-foreground">{formatBytes(drive.available)}</strong> จาก {formatBytes(drive.total)}
              </span>
            )}
          </div>
          {drive ? <UsageBar used={used} total={drive.total} /> : null}
          <div className="text-xs text-muted-foreground">
            สแกนพบ <strong className="text-foreground tabular-nums">{formatBytes(report.total)}</strong>
            {drive && used > report.total * 1.05 && ` · ส่วนต่าง ${formatBytes(used - report.total)} มาจากไฟล์ที่ไม่มีสิทธิ์อ่าน ไฟล์ระบบ หรือ shadow copy`}
          </div>
        </Card>
        <Card className="flex-row items-center gap-3 px-5 py-4">
          <ShieldAlertIcon className="size-5 shrink-0 text-warning-foreground dark:text-warning" aria-hidden />
          <div className="text-sm">
            <div className="font-medium tabular-nums">{formatNumber(report.stats.errors)} โฟลเดอร์</div>
            <div className="text-xs text-muted-foreground">อ่านไม่ได้ (ไม่มีสิทธิ์) — รันแบบผู้ดูแลระบบเพื่อดูครบ</div>
          </div>
        </Card>
        <Card className="flex-row items-center gap-3 px-5 py-4">
          <CloudIcon className="size-5 shrink-0 text-info" aria-hidden />
          <div className="text-sm">
            <div className="font-medium tabular-nums">{formatBytes(report.stats.cloudBytes)}</div>
            <div className="text-xs text-muted-foreground">ไฟล์บนคลาวด์ (OneDrive) — ไม่กินพื้นที่เครื่อง</div>
          </div>
        </Card>
      </div>

      <Tabs value={tab} onValueChange={setTab}>
        <TabsList>
          <TabsTrigger value="why">ทำไมเต็ม</TabsTrigger>
          <TabsTrigger value="explore">สำรวจ (แผนผัง)</TabsTrigger>
          <TabsTrigger value="files">ไฟล์ใหญ่สุด</TabsTrigger>
          <TabsTrigger value="ai">
            <SparklesIcon aria-hidden /> ผู้ช่วย AI
          </TabsTrigger>
        </TabsList>

        <TabsContent value="why" className="grid gap-5 lg:grid-cols-[1fr_22rem]">
          <div className="grid content-start gap-4">
            <Verdict report={report} />
            <h2 className="mt-2 text-base font-semibold">ตัวการที่พบ เรียงจากใหญ่ไปเล็ก</h2>
            <HogList report={report} onShow={showLocation} onReveal={reveal} />
          </div>
          <div className="grid content-start gap-4">
            <CategoryBreakdown report={report} />
          </div>
        </TabsContent>

        <TabsContent value="explore">
          <nav aria-label="ตำแหน่งปัจจุบัน" className="mb-3 flex flex-wrap items-center gap-1 text-sm">
            {(node?.crumbs ?? [[report.rootPath, []]]).map(([name, crumbIds], i, arr) => (
              <span key={crumbIds.join("/") || "root"} className="flex items-center gap-1">
                {i > 0 && <ChevronRightIcon className="size-3.5 text-muted-foreground" aria-hidden />}
                {i === arr.length - 1 ? (
                  <span className="font-medium" aria-current="page">
                    {name}
                  </span>
                ) : (
                  <button type="button" onClick={() => load(crumbIds)} className="cursor-pointer rounded px-1 text-muted-foreground hover:text-foreground hover:underline">
                    {name}
                  </button>
                )}
              </span>
            ))}
            {node && (
              <span className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
                {formatBytes(node.size)} · {formatNumber(node.files)} ไฟล์
                <Button size="sm" variant="ghost" onClick={() => reveal(node.path)}>
                  <FolderIcon /> เปิดใน Explorer
                </Button>
              </span>
            )}
          </nav>
          <div className="grid gap-4 lg:grid-cols-[1fr_26rem]">
            <Card className="h-[34rem] p-3">
              {node ? (
                <Treemap items={node.children} total={node.size} onOpen={(c) => c.index !== null && load([...ids, c.index])} onSelect={(c) => setSelected(c.name)} selected={selected} />
              ) : (
                <Skeleton className="h-full w-full" />
              )}
            </Card>
            <Card className="h-[34rem] overflow-y-auto py-0">
              {node && (
                <SizeList
                  items={node.children}
                  total={node.size}
                  selected={selected}
                  onSelect={(c) => setSelected(c.name)}
                  onOpen={(c) => c.index !== null && load([...ids, c.index])}
                  onReveal={(c) => reveal(childPath(c))}
                  onTrash={askTrash}
                />
              )}
            </Card>
          </div>
        </TabsContent>

        <TabsContent value="files">
          <Card className="py-0">
            <Table>
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead>ไฟล์</TableHead>
                  <TableHead>ประเภท</TableHead>
                  <TableHead className="text-right">ขนาด</TableHead>
                  <TableHead>แก้ไขล่าสุด</TableHead>
                  <TableHead className="w-28" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {report.topFiles.slice(0, 100).map((f) => (
                  <TableRow key={f.path}>
                    <TableCell className="max-w-0 w-full">
                      <div className="truncate font-medium">{f.name}</div>
                      <div className="truncate font-mono text-xs text-muted-foreground" title={f.path}>
                        {f.path}
                      </div>
                    </TableCell>
                    <TableCell>
                      <Badge variant="outline" className="gap-1.5">
                        <span className="size-2 rounded-full" style={{ background: CATEGORY_META[f.category].color }} />
                        {CATEGORY_META[f.category].label}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-right font-semibold tabular-nums">
                      {formatBytes(f.size)}
                      <div className="text-xs font-normal text-muted-foreground">{percent(f.size, report.total).toFixed(1)}%</div>
                    </TableCell>
                    <TableCell className="text-sm text-muted-foreground">{formatAge(f.modified)}</TableCell>
                    <TableCell>
                      <div className="flex justify-end gap-1">
                        <Button size="icon-sm" variant="ghost" aria-label={`ดู ${f.name} ในแผนผัง`} onClick={() => showIds(f.dir_ids)}>
                          <MapIcon />
                        </Button>
                        <Button size="icon-sm" variant="ghost" aria-label={`แสดง ${f.name} ใน File Explorer`} onClick={() => reveal(f.path)}>
                          <FolderIcon />
                        </Button>
                        <Button size="icon-sm" variant="ghost" aria-label={`ย้าย ${f.name} ไปถังขยะ`} onClick={() => askTrashFile(f)}>
                          <Trash2Icon />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Card>
        </TabsContent>

        <TabsContent value="ai">
          <AiPanel backend={backend} report={report} advisor={advisor} onTrash={setPending} onReveal={reveal} onShowLocation={showIds} />
        </TabsContent>
      </Tabs>

      <Dialog open={Boolean(pending)} onOpenChange={(o) => !o && !busy && setPending(null)}>
        <DialogContent closeLabel="ปิด">
          <DialogHeader>
            <DialogTitle>ย้าย “{pending?.name}” ไปถังขยะ?</DialogTitle>
            <DialogDescription>
              จะคืนพื้นที่ประมาณ <strong className="text-foreground">{formatBytes(pending?.size ?? 0)}</strong> — กู้คืนได้จาก Recycle Bin
              จนกว่าจะล้างถังขยะ
            </DialogDescription>
          </DialogHeader>
          <p className="break-all rounded-md bg-muted px-3 py-2 font-mono text-xs">{pending?.path}</p>
          <DialogFooter>
            <Button variant="outline" onClick={() => setPending(null)} disabled={busy}>
              ยกเลิก
            </Button>
            <Button variant="destructive" onClick={confirmTrash} loading={busy}>
              <Trash2Icon /> ย้ายไปถังขยะ
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
