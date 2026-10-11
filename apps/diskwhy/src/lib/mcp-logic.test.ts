import { describe, expect, it } from "vitest";
import { CAPABILITIES, CLAUDE_CODE_REMOVE_COMMAND, confirmPlan, EXAMPLE_PROMPTS, errorText, targetState } from "./mcp-logic";
import type { McpInfo } from "./types";

const CLI = "C:\\Users\\a\\AppData\\Local\\DiskWhy\\binaries\\diskwhy-cli.exe";
const base: McpInfo = {
  cliPath: CLI,
  cliFound: true,
  claudeCodePath: "C:\\Users\\a\\.local\\bin\\claude.exe",
  claudeCodeRegistered: false,
  claudeDesktopConfigPath: "C:\\Users\\a\\AppData\\Roaming\\Claude\\claude_desktop_config.json",
  claudeDesktopRegistered: false,
  claudeCodeCommand: `claude mcp add --scope user diskwhy -- "${CLI}" mcp`,
  desktopSnippet: "{}",
  note: null,
};

describe("targetState", () => {
  it("allows connecting when the app and the CLI are present", () => {
    expect(targetState(base, "claude-code")).toEqual({ installed: true, registered: false, canAct: true, blocker: null });
    expect(targetState(base, "claude-desktop").canAct).toBe(true);
  });

  it("blocks when the Claude app is missing", () => {
    const s = targetState({ ...base, claudeCodePath: null }, "claude-code");
    expect(s).toMatchObject({ installed: false, canAct: false });
    expect(s.blocker).toContain("Claude Code");
    expect(targetState({ ...base, claudeDesktopConfigPath: null }, "claude-desktop").installed).toBe(false);
  });

  it("blocks connecting without the CLI but still allows disconnecting", () => {
    const noCli = { ...base, cliFound: false, cliPath: null };
    const s = targetState(noCli, "claude-code");
    expect(s.canAct).toBe(false);
    expect(s.blocker).toContain("pnpm cli:build");
    expect(targetState({ ...noCli, claudeCodeRegistered: true }, "claude-code").canAct).toBe(true);
  });
});

describe("confirmPlan", () => {
  it("names the exact command for Claude Code", () => {
    expect(confirmPlan(base, "claude-code", "register").details[0]?.value).toBe(base.claudeCodeCommand);
    expect(confirmPlan(base, "claude-code", "unregister").details[0]?.value).toBe(CLAUDE_CODE_REMOVE_COMMAND);
  });

  it("names the exact file for Claude Desktop and warns about the restart", () => {
    const reg = confirmPlan(base, "claude-desktop", "register");
    expect(reg.details[0]?.value).toBe(base.claudeDesktopConfigPath);
    expect(reg.description).toContain("สำรองไฟล์เดิม");
    expect(reg.after).toContain("เปิดใหม่");
    expect(confirmPlan(base, "claude-desktop", "unregister").details[0]?.value).toBe(base.claudeDesktopConfigPath);
  });
});

describe("content", () => {
  it("offers the required example requests and capabilities", () => {
    const texts = EXAMPLE_PROMPTS.map((p) => p.text);
    expect(texts).toContain("ใช้ diskwhy ตรวจไดรฟ์ C: ว่าทำไมเต็ม แล้วเสนอแผนเคลียร์ที่ปลอดภัย");
    expect(texts).toContain("หาไฟล์ที่ไม่ได้ใช้เกิน 1 ปีใน C:\\Users");
    expect(texts).toContain("ล้างแคช npm และ pnpm ให้หน่อย");
    expect(CAPABILITIES).toHaveLength(6);
  });

  it("formats errors from Tauri (strings) and Errors", () => {
    expect(errorText("ข้อความ")).toBe("ข้อความ");
    expect(errorText(new Error("boom"))).toBe("boom");
  });
});
