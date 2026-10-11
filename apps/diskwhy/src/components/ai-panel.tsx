import {
  Alert,
  AlertDescription,
  AlertTitle,
  Badge,
  type BadgeProps,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  cn,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Field,
  Input,
  Label,
  Skeleton,
  Spinner,
  Switch,
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
import {
  CircleCheckIcon,
  CircleIcon,
  CopyIcon,
  EyeOffIcon,
  FileIcon,
  FolderIcon,
  KeyRoundIcon,
  LockKeyholeIcon,
  MapIcon,
  RotateCwIcon,
  SettingsIcon,
  ShieldAlertIcon,
  ShieldCheckIcon,
  SparklesIcon,
  TerminalIcon,
  Trash2Icon,
} from "lucide-react";
import { type FormEvent, useCallback, useEffect, useId, useRef, useState } from "react";
import {
  ACTION_LABEL,
  type AiView,
  applyRemoval,
  baseName,
  canTrashItem,
  errorText,
  SYSTEM_TOOL_LABEL,
  systemToolFor,
  type TrashRequest,
  trashTargetFor,
} from "../lib/ai-logic";
import type { Backend } from "../lib/backend";
import { formatBytes, formatNumber } from "../lib/format";
import type {
  AiCommand,
  AiItem,
  AiKeyStatus,
  AiProgress,
  AiResult,
  AiStep,
  AiVerdict,
  Report,
  SystemTool,
  TrashTarget,
} from "../lib/types";

const KEYS_URL = "https://console.anthropic.com/settings/keys";

/* ── State (lives in ResultsScreen so switching tabs keeps the last result) ─────────────────────── */

export interface AiAdvisor {
  keyStatus: AiKeyStatus | null;
  keyError: string | null;
  reloadKey: () => void;
  saveKey: (key: string) => Promise<boolean>;
  clearKey: () => Promise<boolean>;
  anonymize: boolean;
  setAnonymize: (v: boolean) => void;
  running: boolean;
  progress: AiProgress | null;
  /** Epoch ms when the current run started (drives the elapsed-seconds counter). */
  startedAt: number | null;
  error: string | null;
  view: AiView | null;
  run: () => void;
  /** Back to the consent card (drops the shown result). */
  reset: () => void;
  /** Call after ANY successful move to the Recycle Bin so rows are marked done and tree indices stay valid. */
  applyRemoval: (removed: TrashTarget) => void;
}

export function useAiAdvisor(backend: Backend): AiAdvisor {
  const [keyStatus, setKeyStatus] = useState<AiKeyStatus | null>(null);
  const [keyError, setKeyError] = useState<string | null>(null);
  const [anonymize, setAnonymize] = useState(true);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<AiProgress | null>(null);
  const [startedAt, setStartedAt] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<AiView | null>(null);
  const busy = useRef(false);

  const reloadKey = useCallback(() => {
    setKeyError(null);
    backend
      .aiKeyStatus()
      .then(setKeyStatus)
      .catch((e: unknown) => setKeyError(errorText(e)));
  }, [backend]);

  useEffect(() => {
    reloadKey();
  }, [reloadKey]);

  const saveKey = useCallback(
    async (key: string) => {
      try {
        setKeyStatus(await backend.aiSetKey(key));
        toast.success("บันทึก API key แล้ว", { description: "เก็บไว้ใน Windows Credential Manager" });
        return true;
      } catch (e) {
        toast.error(errorText(e));
        return false;
      }
    },
    [backend],
  );

  const clearKey = useCallback(async () => {
    try {
      setKeyStatus(await backend.aiClearKey());
      toast.success("ลบ API key ออกจาก Credential Manager แล้ว");
      return true;
    } catch (e) {
      toast.error(errorText(e));
      return false;
    }
  }, [backend]);

  const run = useCallback(() => {
    if (busy.current) return;
    busy.current = true;
    setRunning(true);
    setError(null);
    setView(null);
    setStartedAt(Date.now());
    setProgress({ stage: "prepare", done: 0, total: 0, message: "กำลังเตรียมข้อมูล…" });
    backend
      .aiAnalyze({ anonymize }, setProgress)
      .then((result: AiResult) => setView({ result, done: new Set() }))
      .catch((e: unknown) => setError(errorText(e)))
      .finally(() => {
        busy.current = false;
        setRunning(false);
        setProgress(null);
      });
  }, [backend, anonymize]);

  const reset = useCallback(() => {
    setView(null);
    setError(null);
  }, []);

  const removed = useCallback((target: TrashTarget) => setView((v) => (v ? applyRemoval(v, target) : v)), []);

  return {
    keyStatus,
    keyError,
    reloadKey,
    saveKey,
    clearKey,
    anonymize,
    setAnonymize,
    running,
    progress,
    startedAt,
    error,
    view,
    run,
    reset,
    applyRemoval: removed,
  };
}

/* ── Small helpers ────────────────────────────────────────────────────────────────────────────── */

async function copyText(text: string, what: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success(`คัดลอก${what}แล้ว`);
  } catch {
    toast.error("คัดลอกไม่สำเร็จ — เลือกข้อความแล้วกด Ctrl+C แทน");
  }
}

type BadgeVariant = BadgeProps["variant"];

const RISK_META: Record<AiStep["risk"], { label: string; variant: BadgeVariant }> = {
  low: { label: "เสี่ยงต่ำ", variant: "success" },
  medium: { label: "ปานกลาง", variant: "warning" },
  high: { label: "สูง", variant: "destructive" },
};

const VERDICT_META: Record<AiVerdict, { label: string; variant: BadgeVariant }> = {
  delete: { label: "ลบได้", variant: "success" },
  review: { label: "ตรวจสอบ", variant: "warning" },
  keep: { label: "เก็บไว้", variant: "secondary" },
  system_tool: { label: "เครื่องมือ Windows", variant: "info" },
};

const CONFIDENCE_LABEL: Record<AiItem["confidence"], string> = { high: "สูง", medium: "ปานกลาง", low: "ต่ำ" };
const SHELL_LABEL: Record<AiCommand["shell"], string> = { powershell: "PowerShell", cmd: "Command Prompt", settings: "Windows Settings" };

function Elapsed({ startedAt }: { startedAt: number | null }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);
  const s = startedAt ? Math.max(0, Math.floor((now - startedAt) / 1000)) : 0;
  return <span className="tabular-nums">{formatNumber(s)} วินาที</span>;
}

/* ── API key: setup + status ──────────────────────────────────────────────────────────────────── */

function KeySetupCard({ advisor }: { advisor: AiAdvisor }) {
  const [value, setValue] = useState("");
  const [saving, setSaving] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const key = value.trim();
    if (!key || saving) return;
    setSaving(true);
    const ok = await advisor.saveKey(key);
    setSaving(false);
    if (ok) setValue("");
  };

  return (
    <Card className="gap-4">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <KeyRoundIcon className="size-5 text-primary" aria-hidden /> ตั้งค่า API key ของ Anthropic
        </CardTitle>
        <CardDescription>
          ผู้ช่วย AI ใช้ API key ของคุณเอง — DiskWhy ไม่มีเซิร์ฟเวอร์กลางและไม่คิดค่าบริการ ค่าใช้จ่ายคิดจากบัญชี Anthropic ของคุณโดยตรง
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="grid gap-1.5 text-sm">
          <span className="text-muted-foreground">สร้าง key ได้ที่ (คัดลอกไปเปิดในเบราว์เซอร์ของคุณ)</span>
          <div className="flex flex-wrap items-center gap-2">
            <code className="select-text rounded-md bg-muted px-2.5 py-1.5 font-mono text-xs">{KEYS_URL}</code>
            <Button size="sm" variant="outline" onClick={() => copyText(KEYS_URL, "ลิงก์")}>
              <CopyIcon /> คัดลอกลิงก์
            </Button>
          </div>
        </div>
        <form onSubmit={submit} className="flex flex-wrap items-end gap-2">
          <Field label="Anthropic API key" className="min-w-64 flex-1">
            <Input
              type="password"
              value={value}
              onChange={(e) => setValue(e.target.value)}
              placeholder="sk-ant-…"
              autoComplete="off"
              spellCheck={false}
              disabled={saving}
            />
          </Field>
          <Button type="submit" loading={saving} disabled={!value.trim()}>
            บันทึก
          </Button>
        </form>
        <p className="flex items-start gap-2 text-xs text-muted-foreground">
          <LockKeyholeIcon className="mt-0.5 size-3.5 shrink-0" aria-hidden />
          <span>
            key ถูกเก็บใน Windows Credential Manager และไม่ออกจากฝั่ง Rust ของแอป ยกเว้นส่งไปยัง api.anthropic.com เท่านั้น (หน้าจอส่วนนี้ไม่เก็บ key ไว้)
          </span>
        </p>
      </CardContent>
    </Card>
  );
}

function KeyStatusRow({ advisor }: { advisor: AiAdvisor }) {
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const source = advisor.keyStatus?.source;

  const clear = async () => {
    setBusy(true);
    const ok = await advisor.clearKey();
    setBusy(false);
    if (ok) setConfirm(false);
  };

  return (
    <div className="flex flex-wrap items-center gap-2 rounded-lg border bg-card px-4 py-2.5 text-sm">
      <KeyRoundIcon className="size-4 text-success" aria-hidden />
      <span className="font-medium">API key พร้อมใช้งาน</span>
      <Badge variant="outline">{source === "env" ? "ตัวแปร ANTHROPIC_API_KEY" : "Credential Manager"}</Badge>
      {source === "env" && <span className="text-xs text-muted-foreground">ลบได้ที่ตัวแปรสภาพแวดล้อมของ Windows</span>}
      {source === "keychain" && (
        <Button size="sm" variant="ghost" className="ml-auto" disabled={advisor.running} onClick={() => setConfirm(true)}>
          <Trash2Icon /> ลบ key
        </Button>
      )}
      <Dialog open={confirm} onOpenChange={(o) => !o && !busy && setConfirm(false)}>
        <DialogContent closeLabel="ปิด">
          <DialogHeader>
            <DialogTitle>ลบ API key ออกจากเครื่อง?</DialogTitle>
            <DialogDescription>จะลบ key ออกจาก Windows Credential Manager — ต้องใส่ใหม่หากต้องการใช้ผู้ช่วย AI อีกครั้ง</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirm(false)} disabled={busy}>
              ยกเลิก
            </Button>
            <Button variant="destructive" onClick={clear} loading={busy}>
              <Trash2Icon /> ลบ key
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

/* ── Consent ──────────────────────────────────────────────────────────────────────────────────── */

function ConsentCard({ advisor, report, configured }: { advisor: AiAdvisor; report: Report; configured: boolean }) {
  const anonId = useId();
  return (
    <Card className="gap-4">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <SparklesIcon className="size-5 text-primary" aria-hidden /> ให้ AI ช่วยวิเคราะห์ผลสแกน {report.rootPath}
        </CardTitle>
        <CardDescription>AI จะช่วยคัดแยกว่าอะไรลบได้ อะไรควรเก็บ แล้ววางแผนทำความสะอาดทีละขั้น — คุณเป็นคนตัดสินใจและกดยืนยันเองทุกครั้ง</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="rounded-lg border bg-muted/30 p-4 text-sm">
          <h3 className="mb-2 flex items-center gap-2 font-medium">
            <ShieldCheckIcon className="size-4 text-success" aria-hidden /> ข้อมูลที่จะถูกส่งไปยัง Anthropic
          </h3>
          <ul className="grid list-disc gap-1 pl-5 text-muted-foreground">
            <li>เส้นทาง (พาธ) ของโฟลเดอร์/ไฟล์ที่น่าสงสัย</li>
            <li>ขนาด และวันที่แก้ไขล่าสุด</li>
            <li>ประเภทไฟล์ และกฎที่ตรวจพบว่าเข้าข่าย</li>
          </ul>
          <p className="mt-2 font-medium text-foreground">ไม่ส่งเนื้อหาในไฟล์ใด ๆ ทั้งสิ้น</p>
        </div>

        <div className="flex items-start gap-3">
          <Switch id={anonId} checked={advisor.anonymize} onCheckedChange={advisor.setAnonymize} aria-describedby={`${anonId}-desc`} disabled={advisor.running} />
          <div className="grid gap-0.5">
            <Label htmlFor={anonId}>
              <EyeOffIcon className="size-4 text-muted-foreground" aria-hidden /> ซ่อนชื่อผู้ใช้ในพาธ
            </Label>
            <p id={`${anonId}-desc`} className="text-xs text-muted-foreground">
              เช่น C:\Users\somchai\Downloads จะถูกส่งเป็น C:\Users\&lt;user&gt;\Downloads (แนะนำให้เปิดไว้)
            </p>
          </div>
        </div>

        <dl className="grid gap-1 text-sm sm:grid-cols-[auto_1fr] sm:gap-x-4">
          <dt className="text-muted-foreground">โมเดลที่ใช้</dt>
          <dd>Claude Haiku 5.5 คัดแยก → Claude Sonnet 5.5 วางแผน</dd>
          <dt className="text-muted-foreground">ค่าใช้จ่าย</dt>
          <dd>โดยประมาณ $0.15–0.25 ต่อครั้ง (~5–9 บาท) สำหรับ ~300 รายการ คิดจากบัญชี API ของคุณ — ยอดจริงแสดงหลังวิเคราะห์เสร็จ</dd>
        </dl>

        <div className="flex flex-wrap items-center gap-3">
          <Button onClick={advisor.run} disabled={!configured || advisor.running} loading={advisor.running}>
            <SparklesIcon /> วิเคราะห์ด้วย AI
          </Button>
          {!configured && <span className="text-xs text-muted-foreground">ตั้งค่า API key ด้านบนก่อนจึงจะวิเคราะห์ได้</span>}
        </div>
      </CardContent>
    </Card>
  );
}

/* ── Progress ─────────────────────────────────────────────────────────────────────────────────── */

const STAGES: AiProgress["stage"][] = ["prepare", "classify", "plan", "done"];
const STAGE_LABEL: Record<AiProgress["stage"], string> = {
  prepare: "เตรียมข้อมูล",
  classify: "Haiku คัดแยก",
  plan: "Sonnet วางแผน",
  done: "เสร็จ",
};

function ProgressCard({ progress, startedAt }: { progress: AiProgress | null; startedAt: number | null }) {
  const stage = progress?.stage ?? "prepare";
  const current = STAGES.indexOf(stage);
  const classifying = progress?.stage === "classify" && progress.total > 0;
  return (
    <Card className="gap-4">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <Spinner label={null} className="size-5 text-primary" /> AI กำลังวิเคราะห์…
        </CardTitle>
        <CardDescription>
          ใช้เวลาประมาณครึ่งนาที — ผ่านไปแล้ว <Elapsed startedAt={startedAt} />
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div aria-live="polite" className="grid gap-4">
          <ol className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {STAGES.map((s, i) => {
              const state = stage === "done" || i < current ? "done" : i === current ? "active" : "todo";
              return (
                <li
                  key={s}
                  aria-current={state === "active" ? "step" : undefined}
                  className={cn("flex items-center gap-2 rounded-lg border px-3 py-2 text-sm", state === "active" && "border-primary/50 bg-accent", state === "todo" && "text-muted-foreground")}
                >
                  {state === "done" ? (
                    <CircleCheckIcon className="size-4 shrink-0 text-success" aria-hidden />
                  ) : state === "active" ? (
                    <Spinner label={null} className="text-primary" />
                  ) : (
                    <CircleIcon className="size-4 shrink-0" aria-hidden />
                  )}
                  <span className="min-w-0">
                    {STAGE_LABEL[s]}
                    {s === "classify" && classifying && progress && (
                      <span className="ml-1 text-xs text-muted-foreground tabular-nums">
                        ({formatNumber(progress.done)}/{formatNumber(progress.total)})
                      </span>
                    )}
                    <span className="sr-only">{state === "done" ? " — เสร็จแล้ว" : state === "active" ? " — กำลังทำ" : " — รอ"}</span>
                  </span>
                </li>
              );
            })}
          </ol>
          {classifying && progress && (
            <div className="h-1.5 overflow-hidden rounded-full bg-muted" role="progressbar" aria-label="ความคืบหน้าการคัดแยก" aria-valuemin={0} aria-valuemax={progress.total} aria-valuenow={progress.done}>
              <div className="h-full rounded-full bg-primary transition-[width] duration-300" style={{ width: `${(progress.done / progress.total) * 100}%` }} />
            </div>
          )}
          <p className="min-h-5 text-sm text-muted-foreground">{progress?.message}</p>
        </div>
      </CardContent>
    </Card>
  );
}

/* ── Result ───────────────────────────────────────────────────────────────────────────────────── */

interface ResultActions {
  backend: Backend;
  onTrash: (t: TrashRequest) => void;
  onReveal: (path: string) => void;
  onShowLocation: (ids: number[]) => void;
}

function useOpenTool(backend: Backend) {
  return (tool: SystemTool) => backend.openSystemTool(tool).catch((e: unknown) => toast.error(`เปิดเครื่องมือไม่สำเร็จ: ${errorText(e)}`));
}

function CommandBlock({ command, backend }: { command: AiCommand; backend: Backend }) {
  const openTool = useOpenTool(backend);
  const tool = systemToolFor(command.command);
  return (
    <div className="overflow-hidden rounded-lg border bg-muted/40">
      <div className="flex flex-wrap items-center gap-2 border-b px-3 py-1.5">
        <TerminalIcon className="size-3.5 text-muted-foreground" aria-hidden />
        <span className="text-xs font-medium">{command.label}</span>
        <Badge variant="outline">{SHELL_LABEL[command.shell]}</Badge>
        {command.admin && (
          <Badge variant="warning">
            <ShieldAlertIcon /> ต้องใช้สิทธิ์ผู้ดูแลระบบ
          </Badge>
        )}
        <span className="ml-auto flex items-center gap-1">
          {tool && (
            <Button size="sm" variant="outline" onClick={() => openTool(tool)}>
              <SettingsIcon /> {SYSTEM_TOOL_LABEL[tool]}
            </Button>
          )}
          <Button size="sm" variant="ghost" onClick={() => copyText(command.command, "คำสั่ง")} aria-label={`คัดลอกคำสั่ง ${command.label}`}>
            <CopyIcon /> คัดลอก
          </Button>
        </span>
      </div>
      <pre className="select-text whitespace-pre-wrap break-all px-3 py-2 font-mono text-xs leading-relaxed">
        <code>{command.command}</code>
      </pre>
    </div>
  );
}

function StepCard({ step, index, itemsById, done, actions }: { step: AiStep; index: number; itemsById: Map<number, AiItem>; done: ReadonlySet<number>; actions: ResultActions }) {
  const risk = RISK_META[step.risk];
  const items = step.itemIds.map((id) => itemsById.get(id)).filter((i): i is AiItem => i !== undefined);
  return (
    <li>
      <Card className="gap-3 px-5 py-4">
        <div className="flex items-start gap-3">
          <span className="grid size-7 shrink-0 place-items-center rounded-full bg-primary text-sm font-semibold text-primary-foreground tabular-nums" aria-hidden>
            {index + 1}
          </span>
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <h4 className="font-medium">
                <span className="sr-only">ขั้นที่ {index + 1}: </span>
                {step.title}
              </h4>
              <Badge variant={risk.variant}>ความเสี่ยง: {risk.label}</Badge>
            </div>
            <p className="mt-1 text-sm leading-relaxed text-muted-foreground">{step.detail}</p>
          </div>
          <div className="shrink-0 text-right">
            <div className="text-lg font-semibold tabular-nums">≈ {formatBytes(step.estBytes)}</div>
            <div className="text-xs text-muted-foreground">คาดว่าจะได้คืน</div>
          </div>
        </div>
        {items.length > 0 && (
          <div className="flex flex-wrap items-center gap-1.5 text-xs">
            <span className="text-muted-foreground">รายการที่เกี่ยวข้อง:</span>
            {items.map((item) => {
              const name = baseName(item.path);
              const isDone = done.has(item.id);
              const ids = item.dirIds;
              const cls = "inline-flex max-w-64 items-center gap-1 rounded-md border bg-muted/50 px-2 py-1";
              return ids && !isDone ? (
                <button
                  key={item.id}
                  type="button"
                  title={item.path}
                  aria-label={`ดู ${name} ในแผนผัง`}
                  onClick={() => actions.onShowLocation(ids)}
                  className={cn(cls, "cursor-pointer hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/45 focus-visible:outline-none")}
                >
                  <MapIcon className="size-3 shrink-0" aria-hidden />
                  <span className="truncate">{name}</span>
                </button>
              ) : (
                <span key={item.id} title={item.path} className={cn(cls, isDone && "text-muted-foreground line-through")}>
                  <span className="truncate">{name}</span>
                  {isDone && <span className="sr-only"> (ย้ายแล้ว)</span>}
                </span>
              );
            })}
          </div>
        )}
        {step.commands.length > 0 && (
          <div className="grid gap-2">
            {step.commands.map((c) => (
              <CommandBlock key={`${c.label}-${c.command}`} command={c} backend={actions.backend} />
            ))}
          </div>
        )}
      </Card>
    </li>
  );
}

type Filter = "all" | AiVerdict;
const FILTERS: { value: Filter; label: string }[] = [
  { value: "all", label: "ทั้งหมด" },
  { value: "delete", label: "ลบได้" },
  { value: "review", label: "ตรวจสอบ" },
  { value: "keep", label: "เก็บไว้" },
  { value: "system_tool", label: "ใช้เครื่องมือ Windows" },
];
const isFilter = (v: string): v is Filter => FILTERS.some((f) => f.value === v);

function ItemRow({ item, done, actions }: { item: AiItem; done: boolean; actions: ResultActions }) {
  const openTool = useOpenTool(actions.backend);
  const verdict = VERDICT_META[item.verdict];
  const name = baseName(item.path);
  const target = canTrashItem(item) ? trashTargetFor(item) : null;
  const tool = item.verdict === "system_tool" ? systemToolFor(item.command) : null;
  const actionLabel = ACTION_LABEL[item.action];
  return (
    <TableRow className={cn(done && "opacity-60")}>
      <TableCell className="w-full max-w-0">
        <div className="flex items-center gap-2">
          {item.kind === "dir" ? <FolderIcon className="size-4 shrink-0 text-muted-foreground" aria-hidden /> : <FileIcon className="size-4 shrink-0 text-muted-foreground" aria-hidden />}
          <div className="min-w-0">
            <div className="truncate font-medium">{name}</div>
            <div className="truncate font-mono text-xs text-muted-foreground" title={item.path}>
              {item.path}
            </div>
          </div>
        </div>
      </TableCell>
      <TableCell className="text-right font-semibold tabular-nums">{formatBytes(item.size)}</TableCell>
      <TableCell>
        <Badge variant={verdict.variant}>{verdict.label}</Badge>
        <div className="mt-1 text-xs text-muted-foreground">มั่นใจ{CONFIDENCE_LABEL[item.confidence]}</div>
      </TableCell>
      <TableCell className="min-w-60 max-w-xs whitespace-normal text-xs">
        <p className="line-clamp-3 text-muted-foreground" title={item.reason}>
          {item.reason}
        </p>
        {actionLabel && <p className="mt-0.5 font-medium">แนะนำ: {actionLabel}</p>}
      </TableCell>
      <TableCell>
        <div className="flex items-center justify-end gap-1">
          {done ? (
            <Badge variant="success">
              <CircleCheckIcon /> ย้ายแล้ว
            </Badge>
          ) : (
            <>
              {item.dirIds && (
                <Button size="icon-sm" variant="ghost" aria-label={`ดู ${name} ในแผนผัง`} title="ดูในแผนผัง" onClick={() => actions.onShowLocation(item.dirIds ?? [])}>
                  <MapIcon />
                </Button>
              )}
              <Button size="icon-sm" variant="ghost" aria-label={`แสดง ${name} ใน File Explorer`} title="แสดงใน File Explorer" onClick={() => actions.onReveal(item.path)}>
                <FolderIcon />
              </Button>
              {tool && (
                <Button size="icon-sm" variant="ghost" aria-label={SYSTEM_TOOL_LABEL[tool]} title={SYSTEM_TOOL_LABEL[tool]} onClick={() => openTool(tool)}>
                  <SettingsIcon />
                </Button>
              )}
              {target && (
                <Button size="sm" variant="outline" aria-label={`ย้าย ${name} ไปถังขยะ`} onClick={() => actions.onTrash(target)}>
                  <Trash2Icon /> ย้ายไปถังขยะ
                </Button>
              )}
            </>
          )}
        </div>
      </TableCell>
    </TableRow>
  );
}

function ItemsSection({ items, done, actions }: { items: AiItem[]; done: ReadonlySet<number>; actions: ResultActions }) {
  const [filter, setFilter] = useState<Filter>("all");
  const count = (f: Filter) => (f === "all" ? items.length : items.filter((i) => i.verdict === f).length);
  const rows = items.filter((i) => filter === "all" || i.verdict === filter).sort((a, b) => b.size - a.size);
  return (
    <section className="grid gap-3" aria-labelledby="ai-items-title">
      <h3 id="ai-items-title" className="text-base font-semibold">
        รายการที่ AI ตรวจแล้ว
      </h3>
      <Tabs value={filter} onValueChange={(v) => isFilter(v) && setFilter(v)}>
        <TabsList className="h-auto flex-wrap justify-start">
          {FILTERS.map((f) => (
            <TabsTrigger key={f.value} value={f.value} className="h-8 flex-none">
              {f.label}
              <span className="text-xs text-muted-foreground tabular-nums">({formatNumber(count(f.value))})</span>
            </TabsTrigger>
          ))}
        </TabsList>
        <TabsContent value={filter}>
          <Card className="py-0">
            {rows.length === 0 ? (
              <p className="p-6 text-center text-sm text-muted-foreground">ไม่มีรายการในหมวดนี้</p>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow className="hover:bg-transparent">
                    <TableHead>เส้นทาง</TableHead>
                    <TableHead className="text-right">ขนาด</TableHead>
                    <TableHead>ผลวิเคราะห์</TableHead>
                    <TableHead>เหตุผล</TableHead>
                    <TableHead className="text-right">
                      <span className="sr-only">การทำงาน</span>
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {rows.map((item) => (
                    <ItemRow key={item.id} item={item} done={done.has(item.id)} actions={actions} />
                  ))}
                </TableBody>
              </Table>
            )}
          </Card>
        </TabsContent>
      </Tabs>
    </section>
  );
}

function ResultView({ advisor, actions }: { advisor: AiAdvisor; actions: ResultActions }) {
  const view = advisor.view;
  if (!view) return null;
  const { plan, usage, items, anonymized } = view.result;
  const itemsById = new Map(items.map((i) => [i.id, i]));
  return (
    <div className="grid gap-5">
      <Card className="gap-4 border-primary/30 bg-gradient-to-br from-accent to-card">
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-sm font-medium text-muted-foreground">
            <SparklesIcon className="size-4 text-primary" aria-hidden /> ผู้ช่วย AI สรุปให้
          </CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-[1fr_auto] md:items-center">
          <div className="min-w-0">
            <h2 className="text-2xl leading-snug font-semibold tracking-tight">{plan.headline}</h2>
            <p className="mt-2 text-sm leading-relaxed text-muted-foreground">{plan.diagnosis}</p>
          </div>
          <div className="rounded-xl border bg-card px-5 py-4 text-center md:min-w-60">
            <p className="text-xs text-muted-foreground">คืนพื้นที่ได้อย่างปลอดภัย ≈</p>
            <p className="mt-1 text-3xl font-semibold text-success tabular-nums">{formatBytes(plan.reclaimSafeBytes)}</p>
          </div>
        </CardContent>
      </Card>

      {plan.steps.length > 0 && (
        <section className="grid gap-3" aria-labelledby="ai-steps-title">
          <h3 id="ai-steps-title" className="text-base font-semibold">
            แผนการจัดการทีละขั้น
          </h3>
          <ol className="grid gap-3">
            {plan.steps.map((s, i) => (
              <StepCard key={`${i}-${s.title}`} step={s} index={i} itemsById={itemsById} done={view.done} actions={actions} />
            ))}
          </ol>
        </section>
      )}

      <ItemsSection items={items} done={view.done} actions={actions} />

      {plan.warnings.length > 0 && (
        <Alert variant="warning">
          <AlertTitle>ข้อควรระวัง</AlertTitle>
          <AlertDescription>
            <ul className="list-disc pl-5 text-foreground">
              {plan.warnings.map((w) => (
                <li key={w}>{w}</li>
              ))}
            </ul>
          </AlertDescription>
        </Alert>
      )}

      {plan.keep.length > 0 && (
        <Card className="gap-3 px-5 py-4">
          <h3 className="flex items-center gap-2 text-base font-semibold">
            <ShieldCheckIcon className="size-4 text-success" aria-hidden /> สิ่งที่ควรเก็บไว้
          </h3>
          <ul className="grid list-disc gap-1 pl-5 text-sm text-muted-foreground">
            {plan.keep.map((k) => (
              <li key={k}>{k}</li>
            ))}
          </ul>
        </Card>
      )}

      <footer className="flex flex-wrap items-center gap-x-6 gap-y-2 rounded-lg border bg-card px-4 py-3 text-xs text-muted-foreground">
        <span>
          โทเคน เข้า <strong className="text-foreground tabular-nums">{formatNumber(usage.inputTokens)}</strong> / ออก{" "}
          <strong className="text-foreground tabular-nums">{formatNumber(usage.outputTokens)}</strong>
        </span>
        <span>
          ค่าใช้จ่าย <strong className="text-foreground tabular-nums">${usage.costUsd.toFixed(4)}</strong>
        </span>
        <span>โมเดล: {usage.models.length ? usage.models.join(", ") : "—"}</span>
        <span>ซ่อนชื่อผู้ใช้ในพาธ: {anonymized ? "ใช่" : "ไม่"}</span>
        <Button size="sm" variant="outline" className="ml-auto" onClick={advisor.reset}>
          <RotateCwIcon /> วิเคราะห์ใหม่
        </Button>
      </footer>
    </div>
  );
}

/* ── Panel ────────────────────────────────────────────────────────────────────────────────────── */

export interface AiPanelProps {
  backend: Backend;
  report: Report;
  /** State from `useAiAdvisor(backend)` owned by ResultsScreen. */
  advisor: AiAdvisor;
  /** Opens the existing confirm-to-trash dialog. */
  onTrash: (target: TrashRequest) => void;
  onReveal: (path: string) => void;
  /** Jump to the treemap at this directory index path. */
  onShowLocation: (ids: number[]) => void;
}

export function AiPanel({ backend, report, advisor, onTrash, onReveal, onShowLocation }: AiPanelProps) {
  const { keyStatus, keyError, running, error, view } = advisor;
  const configured = keyStatus?.configured === true;
  const actions: ResultActions = { backend, onTrash, onReveal, onShowLocation };

  return (
    <div className="grid gap-4">
      {keyError && (
        <Alert variant="destructive">
          <AlertTitle>ตรวจสอบสถานะ API key ไม่สำเร็จ</AlertTitle>
          <AlertDescription>
            <p>{keyError}</p>
            <Button size="sm" variant="outline" onClick={advisor.reloadKey}>
              <RotateCwIcon /> ลองอีกครั้ง
            </Button>
          </AlertDescription>
        </Alert>
      )}
      {!keyStatus && !keyError && (
        <div role="status">
          <span className="sr-only">กำลังตรวจสอบ API key…</span>
          <Skeleton className="h-40 w-full rounded-xl" />
        </div>
      )}
      {keyStatus && !configured && <KeySetupCard advisor={advisor} />}
      {keyStatus && configured && <KeyStatusRow advisor={advisor} />}

      {running && <ProgressCard progress={advisor.progress} startedAt={advisor.startedAt} />}

      {!running && error && (
        <Alert variant="destructive">
          <AlertTitle>วิเคราะห์ไม่สำเร็จ</AlertTitle>
          <AlertDescription>
            <p>{error}</p>
            <Button size="sm" variant="outline" onClick={advisor.run} disabled={!configured}>
              <RotateCwIcon /> ลองอีกครั้ง
            </Button>
          </AlertDescription>
        </Alert>
      )}

      {!running && !view && keyStatus && <ConsentCard advisor={advisor} report={report} configured={configured} />}
      {!running && view && <ResultView advisor={advisor} actions={actions} />}
    </div>
  );
}
