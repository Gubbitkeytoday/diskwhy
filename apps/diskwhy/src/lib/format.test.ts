import { describe, expect, it } from "vitest";
import { formatAge, formatBytes, formatDuration, percent } from "./format";

const DAY = 86_400;
/** Fixed "now" (2026-10-12 00:00 UTC) so age tests do not depend on the clock. */
const NOW_MS = Date.UTC(2026, 9, 12);
const NOW_SECS = NOW_MS / 1000;

describe("formatBytes", () => {
  it("uses binary units like Windows Explorer", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1023)).toBe("1,023 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(476 * 1024 ** 3)).toBe("476 GB");
    expect(percent(1, 0)).toBe(0);
  });
});

describe("formatAge", () => {
  it("describes the age relative to nowMs in Thai", () => {
    expect(formatAge(NOW_SECS - 3_600, NOW_MS)).toBe("วันนี้");
    expect(formatAge(NOW_SECS - 12 * DAY, NOW_MS)).toBe("12 วันก่อน");
    expect(formatAge(NOW_SECS - 90 * DAY, NOW_MS)).toBe("3 เดือนก่อน");
    expect(formatAge(NOW_SECS - 730 * DAY, NOW_MS)).toBe("2 ปีก่อน");
    expect(formatAge(NOW_SECS - 760 * DAY, NOW_MS)).toBe("2 ปี 1 เดือนก่อน");
  });

  it("returns ไม่ทราบ when the timestamp is missing", () => {
    expect(formatAge(null, NOW_MS)).toBe("ไม่ทราบ");
    expect(formatAge(undefined, NOW_MS)).toBe("ไม่ทราบ");
    expect(formatAge(0, NOW_MS)).toBe("ไม่ทราบ");
  });
});

describe("formatDuration", () => {
  it("shows seconds below one minute", () => {
    expect(formatDuration(0)).toBe("0 วินาที");
    expect(formatDuration(1_500)).toBe("1.5 วินาที");
    expect(formatDuration(59_000)).toBe("59 วินาที");
  });

  it("shows minutes and seconds from one minute up", () => {
    expect(formatDuration(90_000)).toBe("1 นาที 30 วินาที");
    expect(formatDuration(73_600)).toBe("1 นาที 14 วินาที");
  });
});
