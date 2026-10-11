// Pure helpers for the "เชื่อมต่อ Claude (MCP)" panel: which actions are possible, what the confirm dialog says, example prompts.
import type { McpInfo, McpTarget } from "./types";

export type McpAction = "register" | "unregister";

/** Usage shown for people who want to run the same engine by hand from a terminal. */
export const CLI_USAGE = "diskwhy-cli report C:\\ --top 10";
export const CLI_BUILD_COMMAND = "pnpm cli:build";
export const CLAUDE_CODE_REMOVE_COMMAND = "claude mcp remove --scope user diskwhy";

export const TARGET_LABEL: Record<McpTarget, string> = {
  "claude-code": "Claude Code",
  "claude-desktop": "Claude Desktop",
};

/** What Claude can do through DiskWhy — plain Thai, no jargon. */
export const CAPABILITIES: readonly { title: string; detail: string }[] = [
  { title: "สแกนไดรฟ์และโฟลเดอร์", detail: "ดูว่าอะไรกินพื้นที่อยู่ เรียงจากใหญ่ไปเล็ก" },
  { title: "วินิจฉัยว่าทำไมดิสก์เต็ม", detail: "แยกตามประเภทไฟล์ แคช และตัวการที่พบบ่อย" },
  { title: "หาข้อมูลที่ไม่ได้ใช้", detail: "ไฟล์และโฟลเดอร์ที่ไม่ได้แตะมานาน" },
  { title: "ย้ายไปถังขยะ (เมื่อคุณยืนยัน)", detail: "ไม่ลบถาวร — กู้คืนได้จาก Recycle Bin" },
  { title: "สั่งล้างแคชที่รู้จัก", detail: "เช่น แคช npm, pnpm, ไฟล์ชั่วคราว" },
  { title: "ให้คำสั่งสำหรับผู้ดูแลระบบ", detail: "งานที่ต้องใช้สิทธิ์แอดมิน Claude จะเขียนคำสั่งให้คุณรันเอง" },
];

/** Requests to try in Claude. Copied verbatim, so keep each one self-contained. */
export const EXAMPLE_PROMPTS: readonly { label: string; text: string }[] = [
  { label: "ตรวจไดรฟ์ที่เต็ม", text: "ใช้ diskwhy ตรวจไดรฟ์ C: ว่าทำไมเต็ม แล้วเสนอแผนเคลียร์ที่ปลอดภัย" },
  { label: "หาไฟล์ที่ไม่ได้ใช้", text: "หาไฟล์ที่ไม่ได้ใช้เกิน 1 ปีใน C:\\Users" },
  { label: "ล้างแคช", text: "ล้างแคช npm และ pnpm ให้หน่อย" },
  { label: "ไฟล์ใหญ่สุด", text: "ใช้ diskwhy แสดงไฟล์ที่ใหญ่ที่สุด 10 อันดับในไดรฟ์ C: พร้อมบอกว่าอันไหนลบได้" },
];

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

export interface TargetState {
  /** Claude Code's claude.exe / Claude Desktop's config folder was found. */
  installed: boolean;
  registered: boolean;
  /** The primary button can be pressed. */
  canAct: boolean;
  /** Why it cannot (shown under the button); null when it can. */
  blocker: string | null;
}

export function targetState(info: McpInfo, target: McpTarget): TargetState {
  const installed = target === "claude-code" ? info.claudeCodePath !== null : info.claudeDesktopConfigPath !== null;
  const registered = target === "claude-code" ? info.claudeCodeRegistered : info.claudeDesktopRegistered;
  if (!installed) {
    return {
      installed,
      registered,
      canAct: false,
      blocker: `ไม่พบ ${TARGET_LABEL[target]} ในเครื่องนี้ — ติดตั้งก่อน หรือใช้วิธีตั้งค่าเองด้านล่างเมื่อพร้อม`,
    };
  }
  // Removing an existing registration never needs the CLI; adding one does.
  if (!registered && !info.cliFound) {
    return { installed, registered, canAct: false, blocker: `ไม่พบ diskwhy-cli.exe — รัน ${CLI_BUILD_COMMAND} ก่อน` };
  }
  return { installed, registered, canAct: true, blocker: null };
}

export interface ConfirmPlan {
  title: string;
  description: string;
  /** What exactly changes: label + value shown in monospace. */
  details: { label: string; value: string }[];
  /** Short follow-up the user should know about. */
  after: string;
  confirmLabel: string;
}

/** Text of the confirm dialog: names the exact file or command that will change. */
export function confirmPlan(info: McpInfo, target: McpTarget, action: McpAction): ConfirmPlan {
  const name = TARGET_LABEL[target];
  if (target === "claude-code") {
    return action === "register"
      ? {
          title: `เชื่อมต่อ DiskWhy กับ ${name}?`,
          description: "DiskWhy จะรันคำสั่งของ Claude Code ที่เพิ่ม DiskWhy เป็น MCP server ระดับผู้ใช้ (ใช้ได้ทุกโปรเจกต์) ไม่มีการแก้ไขไฟล์อื่น",
          details: [
            { label: "คำสั่งที่จะรัน", value: info.claudeCodeCommand },
            { label: "บันทึกลงใน", value: "ไฟล์ตั้งค่าผู้ใช้ของ Claude Code (.claude.json ในโฟลเดอร์ผู้ใช้)" },
          ],
          after: "เปิดเซสชัน Claude Code ใหม่ แล้วพิมพ์ /mcp เพื่อตรวจสอบว่า diskwhy เชื่อมต่อแล้ว",
          confirmLabel: "เชื่อมต่อ",
        }
      : {
          title: `ยกเลิกการเชื่อมต่อกับ ${name}?`,
          description: "DiskWhy จะรันคำสั่งที่ลบ DiskWhy ออกจากรายการ MCP server ของ Claude Code เท่านั้น ไม่กระทบ server อื่น",
          details: [{ label: "คำสั่งที่จะรัน", value: CLAUDE_CODE_REMOVE_COMMAND }],
          after: "Claude Code จะไม่เห็น DiskWhy ในเซสชันถัดไป",
          confirmLabel: "ยกเลิกการเชื่อมต่อ",
        };
  }
  const file = info.claudeDesktopConfigPath ?? "claude_desktop_config.json";
  return action === "register"
    ? {
        title: `เชื่อมต่อ DiskWhy กับ ${name}?`,
        description:
          "DiskWhy จะเพิ่มรายการ mcpServers.diskwhy ลงในไฟล์ตั้งค่าของ Claude Desktop โดยไม่แตะค่าอื่นหรือ server อื่น และสำรองไฟล์เดิมไว้ข้าง ๆ (claude_desktop_config.json.bak-เวลา) ก่อนเขียนเสมอ",
        details: [
          { label: "ไฟล์ที่จะแก้ไข", value: file },
          { label: "ค่าที่จะเพิ่ม", value: info.desktopSnippet },
        ],
        after: "ต้องปิด Claude Desktop ให้สนิท (คลิกขวาไอคอนที่ถาดระบบแล้วเลือกออก) แล้วเปิดใหม่ จึงจะเห็น DiskWhy",
        confirmLabel: "เชื่อมต่อ",
      }
    : {
        title: `ยกเลิกการเชื่อมต่อกับ ${name}?`,
        description: "DiskWhy จะลบเฉพาะรายการ mcpServers.diskwhy ออกจากไฟล์ตั้งค่าของ Claude Desktop และสำรองไฟล์เดิมไว้ก่อนเสมอ ค่าอื่นไม่ถูกแตะ",
        details: [{ label: "ไฟล์ที่จะแก้ไข", value: file }],
        after: "ต้องปิดและเปิด Claude Desktop ใหม่เพื่อให้มีผล",
        confirmLabel: "ยกเลิกการเชื่อมต่อ",
      };
}
