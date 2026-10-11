import {
  Alert,
  AlertDescription,
  AlertTitle,
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
  toast,
} from "@nova/ui";
import {
  CheckIcon,
  CircleCheckIcon,
  CircleXIcon,
  CopyIcon,
  MonitorIcon,
  PlugIcon,
  RefreshCwIcon,
  ShieldCheckIcon,
  TerminalIcon,
  UnplugIcon,
} from "lucide-react";
import { type ReactNode, useCallback, useEffect, useId, useRef, useState } from "react";
import type { Backend } from "../lib/backend";
import { copyText } from "../lib/clipboard";
import {
  CAPABILITIES,
  CLI_BUILD_COMMAND,
  CLI_USAGE,
  confirmPlan,
  EXAMPLE_PROMPTS,
  errorText,
  type McpAction,
  TARGET_LABEL,
  targetState,
} from "../lib/mcp-logic";
import type { McpInfo, McpTarget } from "../lib/types";

/** Copy-to-clipboard button that confirms visibly ("คัดลอกแล้ว") and to screen readers (toast is a live region). */
function CopyButton({ text, what }: { text: string; what: string }) {
  const [done, setDone] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);

  const copy = async () => {
    if (await copyText(text)) {
      setDone(true);
      toast.success(`คัดลอก${what}แล้ว`);
      clearTimeout(timer.current);
      timer.current = setTimeout(() => setDone(false), 1800);
    } else {
      toast.error("คัดลอกไม่สำเร็จ — เลือกข้อความแล้วกด Ctrl+C แทน");
    }
  };

  return (
    <Button type="button" size="sm" variant="ghost" onClick={copy} aria-label={`คัดลอก${what}`}>
      {done ? <CheckIcon /> : <CopyIcon />} {done ? "คัดลอกแล้ว" : "คัดลอก"}
    </Button>
  );
}

/** Monospace block with a Copy button, for commands and config snippets. */
function CodeBlock({ label, text, what }: { label: string; text: string; what: string }) {
  return (
    <div className="overflow-hidden rounded-lg border bg-muted/50">
      <div className="flex items-center justify-between gap-2 border-b pl-3 pr-1 text-xs text-muted-foreground">
        <span className="py-1.5">{label}</span>
        <CopyButton text={text} what={what} />
      </div>
      <pre className="px-3 py-2.5 font-mono text-xs leading-relaxed break-all whitespace-pre-wrap text-foreground">
        <code>{text}</code>
      </pre>
    </div>
  );
}

function StatusBadge({ ok, yes, no }: { ok: boolean; yes: string; no: string }) {
  return ok ? (
    <Badge variant="success">
      <CircleCheckIcon aria-hidden /> {yes}
    </Badge>
  ) : (
    <Badge variant="outline" className="text-muted-foreground">
      <CircleXIcon aria-hidden /> {no}
    </Badge>
  );
}

const CARD_META: Record<McpTarget, { icon: typeof TerminalIcon; blurb: string }> = {
  "claude-code": { icon: TerminalIcon, blurb: "ผู้ช่วยเขียนโค้ดที่ใช้ในเทอร์มินัลและ VS Code" },
  "claude-desktop": { icon: MonitorIcon, blurb: "แอป Claude บนเดสก์ท็อป" },
};

function ConnectCard({
  target,
  info,
  busy,
  onAction,
}: {
  target: McpTarget;
  info: McpInfo;
  busy: boolean;
  onAction: (target: McpTarget, action: McpAction) => void;
}) {
  const hintId = useId();
  const state = targetState(info, target);
  const { icon: Icon, blurb } = CARD_META[target];
  const foundPath = target === "claude-code" ? info.claudeCodePath : info.claudeDesktopConfigPath;
  const action: McpAction = state.registered ? "unregister" : "register";

  return (
    <Card className="gap-4 px-5 py-5" aria-labelledby={`${hintId}-title`} role="group">
      <div className="flex items-start gap-3">
        <span className="grid size-10 shrink-0 place-items-center rounded-lg bg-accent text-accent-foreground">
          <Icon className="size-5" aria-hidden />
        </span>
        <div className="min-w-0 flex-1">
          <h4 id={`${hintId}-title`} className="font-semibold">
            {TARGET_LABEL[target]}
          </h4>
          <p className="text-xs text-muted-foreground">{blurb}</p>
        </div>
        <div className="flex flex-wrap justify-end gap-1.5">
          <StatusBadge ok={state.installed} yes="พบ" no="ไม่พบ" />
          <StatusBadge ok={state.registered} yes="ลงทะเบียนแล้ว" no="ยังไม่ได้ลงทะเบียน" />
        </div>
      </div>

      {foundPath && (
        <p className="break-all font-mono text-xs text-muted-foreground" title={foundPath}>
          {target === "claude-code" ? "โปรแกรม: " : "ไฟล์ตั้งค่า: "}
          {foundPath}
        </p>
      )}

      <div className="grid gap-1.5">
        <Button
          className="w-full sm:w-fit"
          variant={state.registered ? "outline" : "primary"}
          disabled={!state.canAct || busy}
          aria-describedby={state.blocker ? hintId : undefined}
          onClick={() => onAction(target, action)}
        >
          {state.registered ? <UnplugIcon /> : <PlugIcon />}
          {state.registered ? "ยกเลิกการเชื่อมต่อ" : "เชื่อมต่อ"}
        </Button>
        {state.blocker && (
          <p id={hintId} className="text-xs text-muted-foreground">
            {state.blocker}
          </p>
        )}
      </div>

      <div className="grid gap-2 border-t pt-4">
        <h5 className="text-sm font-medium">ตั้งค่าเอง</h5>
        {target === "claude-code" ? (
          <>
            <p className="text-xs text-muted-foreground">ถ้าไม่อยากกดปุ่ม คัดลอกคำสั่งนี้ไปรันในเทอร์มินัลได้เลย</p>
            <CodeBlock label="คำสั่งสำหรับเทอร์มินัล" text={info.claudeCodeCommand} what="คำสั่ง Claude Code" />
          </>
        ) : (
          <>
            <p className="text-xs text-muted-foreground">
              เพิ่มส่วนนี้ในไฟล์ <code className="font-mono text-foreground">claude_desktop_config.json</code> (ถ้ามี mcpServers อยู่แล้ว ให้เพิ่มเฉพาะ diskwhy) แล้วรีสตาร์ท Claude Desktop
            </p>
            <CodeBlock label="claude_desktop_config.json" text={info.desktopSnippet} what="ค่าตั้งค่า JSON" />
          </>
        )}
      </div>
    </Card>
  );
}

function Section({ title, children, description }: { title: string; description?: string; children: ReactNode }) {
  return (
    <section className="grid gap-3">
      <div>
        <h3 className="text-base font-semibold">{title}</h3>
        {description && <p className="text-sm text-muted-foreground">{description}</p>}
      </div>
      {children}
    </section>
  );
}

export function McpPanel({ backend }: { backend: Backend }) {
  const [info, setInfo] = useState<McpInfo | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [pending, setPending] = useState<{ target: McpTarget; action: McpAction } | null>(null);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [lastNote, setLastNote] = useState<string | null>(null);

  const load = useCallback(async () => {
    setChecking(true);
    setLoadError(null);
    try {
      setInfo(await backend.mcpInfo());
    } catch (e) {
      setLoadError(errorText(e));
    } finally {
      setChecking(false);
    }
  }, [backend]);

  useEffect(() => {
    void load();
  }, [load]);

  const ask = (target: McpTarget, action: McpAction) => {
    setActionError(null);
    setPending({ target, action });
  };

  // Registration only ever happens here, after the user pressed the confirm button of the dialog.
  const confirm = async () => {
    if (!pending) return;
    const { target, action } = pending;
    setBusy(true);
    setActionError(null);
    try {
      const next = action === "register" ? await backend.mcpRegister(target) : await backend.mcpUnregister(target);
      setInfo(next);
      setLastNote(next.note);
      setPending(null);
      toast.success(action === "register" ? `เชื่อมต่อ ${TARGET_LABEL[target]} แล้ว` : `ยกเลิกการเชื่อมต่อ ${TARGET_LABEL[target]} แล้ว`);
    } catch (e) {
      setActionError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const plan = pending && info ? confirmPlan(info, pending.target, pending.action) : null;
  // Keep the last dialog text on screen while the close animation runs, instead of flashing an empty dialog.
  const shown = useRef<{ plan: ReturnType<typeof confirmPlan>; action: McpAction } | null>(null);
  if (plan && pending) shown.current = { plan, action: pending.action };
  const view = shown.current;

  return (
    <div className="grid gap-6 pb-6" aria-busy={checking}>
      <Section title="ใช้ Claude ที่คุณมีอยู่แล้ว" description="DiskWhy ทำงานร่วมกับ Claude Code และ Claude Desktop ที่คุณใช้อยู่ ไม่ต้องใส่คีย์ API และไม่มีค่าใช้จ่ายเพิ่มจากแอปนี้">
        <p className="text-sm">เมื่อเชื่อมต่อแล้ว คุณสั่งเป็นภาษาไทยในแอป Claude ได้เลย แล้ว Claude จะใช้เครื่องมือของ DiskWhy ทำงานแทนคุณ:</p>
        <ul className="grid gap-2 sm:grid-cols-2">
          {CAPABILITIES.map((c) => (
            <li key={c.title} className="flex gap-2 rounded-lg border bg-card px-3 py-2.5 text-sm">
              <CheckIcon className="mt-0.5 size-4 shrink-0 text-success" aria-hidden />
              <span>
                <span className="font-medium">{c.title}</span>
                <span className="block text-xs text-muted-foreground">{c.detail}</span>
              </span>
            </li>
          ))}
        </ul>
        <Alert variant="success" icon={false}>
          <ShieldCheckIcon aria-hidden />
          <AlertTitle>Claude ถามคุณก่อนทุกครั้งที่จะลบหรือเปลี่ยนแปลง</AlertTitle>
          <AlertDescription>
            <p>ไฟล์ที่ลบจะถูกย้ายไปถังขยะ (Recycle Bin) ไม่ลบถาวร กู้คืนได้จนกว่าคุณจะล้างถังขยะ งานที่ต้องใช้สิทธิ์แอดมิน Claude จะให้คำสั่งมาให้คุณรันเอง</p>
            <p>DiskWhy เองไม่ส่งข้อมูลออกจากเครื่อง ผลสแกนที่ Claude เรียกดูจะไปอยู่ในบทสนทนากับ Claude ของคุณเท่านั้น</p>
          </AlertDescription>
        </Alert>
      </Section>

      {info && !info.cliFound && (
        <Alert variant="warning">
          <AlertTitle>ยังไม่พบ diskwhy-cli.exe</AlertTitle>
          <AlertDescription>
            <p>นี่คือเวอร์ชันสำหรับนักพัฒนาที่ยังไม่ได้สร้างตัวเชื่อมต่อ จึงกด “เชื่อมต่อ” ไม่ได้ รันคำสั่งนี้ในโฟลเดอร์โปรเจกต์ แล้วเปิดแอปใหม่ (ตัวติดตั้งจริงมีไฟล์นี้มาให้แล้ว)</p>
            <div className="mt-1 flex w-full items-center justify-between gap-2 rounded-md border bg-background/70 pl-3 pr-1">
              <code className="font-mono text-xs text-foreground">{CLI_BUILD_COMMAND}</code>
              <CopyButton text={CLI_BUILD_COMMAND} what="คำสั่งสร้าง CLI" />
            </div>
          </AlertDescription>
        </Alert>
      )}

      <Section title="เลือกแอป Claude ที่จะเชื่อมต่อ">
        {loadError ? (
          <Alert variant="destructive">
            <AlertTitle>ตรวจสถานะไม่สำเร็จ</AlertTitle>
            <AlertDescription>
              <p className="break-words">{loadError}</p>
              <Button size="sm" variant="outline" onClick={() => void load()} loading={checking}>
                <RefreshCwIcon /> ลองอีกครั้ง
              </Button>
            </AlertDescription>
          </Alert>
        ) : !info ? (
          <div className="grid gap-3" role="status" aria-label="กำลังตรวจสอบ Claude ในเครื่อง">
            <Skeleton className="h-64 rounded-xl" />
            <Skeleton className="h-64 rounded-xl" />
          </div>
        ) : (
          <>
            {lastNote && (
              <Alert variant="info" role="status">
                <AlertDescription>
                  <p className="text-foreground">{lastNote}</p>
                </AlertDescription>
              </Alert>
            )}
            <ConnectCard target="claude-code" info={info} busy={busy} onAction={ask} />
            <ConnectCard target="claude-desktop" info={info} busy={busy} onAction={ask} />
            <div>
              <Button size="sm" variant="ghost" onClick={() => void load()} loading={checking}>
                <RefreshCwIcon /> ตรวจสอบสถานะอีกครั้ง
              </Button>
            </div>
          </>
        )}
      </Section>

      <Section title="ลองสั่ง Claude" description="เปิด Claude แล้ววางประโยคเหล่านี้ได้เลย (ถ้า Claude ถามสิทธิ์ใช้ diskwhy ให้อนุญาต)">
        <ul className="grid gap-2">
          {EXAMPLE_PROMPTS.map((p) => (
            <li key={p.text} className="flex items-center gap-2 rounded-lg border bg-card py-1.5 pl-3 pr-1">
              <div className="min-w-0 flex-1">
                <div className="text-xs text-muted-foreground">{p.label}</div>
                <q className="text-sm break-words">{p.text}</q>
              </div>
              <CopyButton text={p.text} what={`ประโยค “${p.label}”`} />
            </li>
          ))}
        </ul>
        <p className="text-xs text-muted-foreground">Claude จะถามยืนยันก่อนทำสิ่งที่ลบหรือเปลี่ยนแปลงไฟล์ทุกครั้ง และของที่ลบจะไปอยู่ในถังขยะ (Recycle Bin)</p>
      </Section>

      <Section title="ใช้จากเทอร์มินัลโดยตรง" description="ตัวเดียวกันนี้ใช้เป็นคำสั่งได้ด้วย เช่น สรุปไดรฟ์ C: เฉพาะ 10 อันดับแรก">
        <CodeBlock label="PowerShell / Command Prompt" text={CLI_USAGE} what="ตัวอย่างคำสั่ง CLI" />
        {info?.cliPath && <p className="break-all font-mono text-xs text-muted-foreground">ตำแหน่งไฟล์: {info.cliPath}</p>}
      </Section>

      <Dialog open={Boolean(plan)} onOpenChange={(o) => !o && !busy && setPending(null)}>
        <DialogContent closeLabel="ปิด" className="max-w-xl">
          <DialogHeader>
            <DialogTitle>{view?.plan.title}</DialogTitle>
            <DialogDescription>{view?.plan.description}</DialogDescription>
          </DialogHeader>
          <div className="grid max-h-[40svh] gap-3 overflow-y-auto">
            {view?.plan.details.map((d) => (
              <div key={d.label}>
                <div className="mb-1 text-xs font-medium text-muted-foreground">{d.label}</div>
                <pre className="rounded-md bg-muted px-3 py-2 font-mono text-xs break-all whitespace-pre-wrap">{d.value}</pre>
              </div>
            ))}
          </div>
          <p className="text-sm text-muted-foreground">{view?.plan.after}</p>
          {actionError && (
            <Alert variant="destructive">
              <AlertTitle>ทำไม่สำเร็จ</AlertTitle>
              <AlertDescription>
                <p className="break-words">{actionError}</p>
              </AlertDescription>
            </Alert>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setPending(null)} disabled={busy}>
              ยกเลิก
            </Button>
            <Button variant={view?.action === "unregister" ? "destructive" : "primary"} onClick={confirm} loading={busy}>
              {view?.action === "unregister" ? <UnplugIcon /> : <PlugIcon />} {view?.plan.confirmLabel}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
