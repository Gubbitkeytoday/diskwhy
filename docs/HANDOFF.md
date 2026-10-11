# Handoff — DiskWhy

## 1. สถาปัตยกรรม

```
React UI (WebView2) ──invoke()──▶ src-tauri/src/lib.rs (Tauri commands)
      ▲                                  │
      └──── events: scan-progress ◀──────┤  thread: scan (rayon fork-join ใน core)
                   scan-done              │  ticker thread: ~8 events/s
                                          ▼
Claude Code / Claude Desktop ──stdio──▶ diskwhy-cli mcp ──▶ diskwhy-core (crates/core)
                                          ▲                  scan · insights · report
terminal ──▶ diskwhy-cli (CLI) ───────────┘                  safety · actions · installed
```

- **`crates/core`** (`diskwhy-core`) — ไลบรารีกลางที่ไม่มี UI:
  - `scan.rs` ตัวสแกนแบบขนาน + ต้นไม้ในหน่วยความจำ
  - `insights.rs` รายงานตัวการ (hogs) หมวดไฟล์ ไฟล์ใหญ่สุด มุมมองโหนด และตัวตรวจข้อมูลที่ไม่ได้ใช้ (`stale-large-files`, `stale-projects`, `old-installers`, `orphaned-appdata`)
  - `installed.rs` รายการโปรแกรมที่ติดตั้งจาก Registry แบบอ่านอย่างเดียว ใช้เทียบกับโฟลเดอร์ AppData
  - `report.rs` สแกน (`run_scan`) สร้างรายงาน (`build_report`) และข้อมูลไดรฟ์
  - `safety.rs` กฎความปลอดภัยที่ทุกพื้นผิว (แอป, CLI, MCP) ใช้ร่วมกัน
  - `actions.rs` คำสั่ง Windows ที่อนุญาตแบบ whitelist
- **`crates/cli`** (`diskwhy-cli`) — เป็นทั้ง CLI (`drives`, `report`, `unused`, `top`, `cleanup`) และ MCP server (`mcp`, stdio) ยังอยู่ระหว่างพัฒนาโดยทีมอื่น
- **`src-tauri`** — เปลือกเดสก์ท็อป รับคำสั่งจาก WebView แล้วเรียก `diskwhy-core` ใน thread แยก (`start_scan`) ส่ง progress กลับเป็น event
- **ตัวติดตั้ง** — ตัวติดตั้งแอปต้องมี `diskwhy-cli.exe` มาด้วย (ไว้ให้ปุ่ม "เชื่อมต่อ Claude (MCP)" ชี้ไป) ตอนนี้ `tauri.conf.json` ยังไม่มี `bundle.externalBin` จึงต้องเพิ่มก่อนปล่อย
- **ต้นไม้ในหน่วยความจำ**: เก็บทุกโฟลเดอร์ + ไฟล์ ≥ 1 MB แยกรายตัว; ไฟล์เล็กรวมเป็นยอดต่อโฟลเดอร์ → ใช้ RAM น้อยแม้มีหลายล้านไฟล์
- **ระบุตำแหน่งด้วย index path** (`ids: [0, 3, 1]`) แทน string path — เร็วและไม่มีปัญหาตัวอักษรพิเศษ
- **หลังลบ** index ของพี่น้องอาจเลื่อน → UI โหลดโฟลเดอร์แม่ใหม่เสมอ (`results.tsx › confirmTrash`)
- **การเริ่มต้นฝั่ง UI**: `getBackend()` reject ด้วย `Error("NOT_IN_APP")` เมื่อเปิดนอกแอปเดสก์ท็อป → `App.tsx` แสดง `NotInAppScreen`; error อื่นของการเริ่มต้นแสดง `StartupErrorScreen` พร้อมปุ่มลองอีกครั้ง

รายละเอียด MCP (tools, prompts, การเชื่อมต่อ, การแก้ปัญหา) อยู่ที่ [docs/MCP.md](MCP.md)

## 2. สัญญาข้อมูล (ต้องแก้คู่กันเสมอ)

| Rust (serde) | TypeScript |
|---|---|
| `crates/core/src/report.rs › DriveInfo, ScanStats, Report` | `src/lib/types.ts` |
| `src-tauri/src/lib.rs › ProgressEvent, DoneEvent` | `src/lib/types.ts` |
| `crates/core/src/insights.rs › Hog, Location, TopFile, NodeView, ChildView, CategoryStat, ExtStat, Safety` | `src/lib/types.ts` |
| `src-tauri/src/lib.rs › McpInfo` (คำสั่ง `mcp_info`, `mcp_register`, `mcp_unregister`) | `src/lib/types.ts` (ส่วน MCP) |

- ชื่อ argument ของ command ฝั่ง JS เป็น camelCase (`dirIds`, `fileName`) — Tauri แปลงเป็น snake_case ให้
- คำสั่งที่มีอยู่ใน `src-tauri/src/lib.rs`: `list_drives`, `start_scan`, `cancel_scan`, `get_report`, `get_node`, `move_to_trash`, `open_system_tool`
- `mcp_info`, `mcp_register`, `mcp_unregister` อยู่ในสัญญานี้แล้ว แต่ยังไม่มีใน `src-tauri/src/lib.rs` ณ วันที่เขียนเอกสาร ให้ยึดนิยามจริงใน Rust และ `types.ts` เมื่อ implement เสร็จ
- `move_to_trash` ของเดสก์ท็อปคือการย้ายไปถังขยะเดียวกับเครื่องมือ `move_to_recycle_bin` ของ MCP (ดู MCP.md)

## 3. เพิ่มกฎ "ตัวการ" ใหม่

1. เพิ่ม `Rule` ใน `crates/core/src/insights.rs › RULES` (id, label ไทย, advice, safety, test)
2. เพิ่มเทสต์ใน `crates/core/src/insights.rs › tests`
3. ทั้งแอป, CLI และ MCP ใช้ `insights::find_hogs` ร่วมกัน จึงไม่ต้องเพิ่มข้อมูลแยกในแต่ละพื้นผิว

ไม่มีโหมดจำลองให้ต้องซิงก์ตาม UI รับรายการตัวการจาก backend โดยตรง จึงไม่ต้องเพิ่มข้อมูลตัวอย่างฝั่ง TypeScript

ระดับความปลอดภัย: `safe` = สร้างใหม่ได้, `review` = ข้อมูลส่วนตัว ดูก่อน, `system` = ใช้เครื่องมือ Windows ห้ามลบเอง

## 4. ความปลอดภัย

**การลบและการย้ายไฟล์**

- การลบทุกเส้นทางผ่าน `safety::recycle` → `trash::delete` (ย้ายไป Recycle Bin, กู้คืนได้) โค้ดปัจจุบันไม่มี hard delete
- ตามการออกแบบ การลบถาวรทางเดียวคือ action `empty-recycle-bin` ของ `run_cleanup` (MCP/CLI) ซึ่งยังไม่ได้ implement
- `safety::is_protected()` (ย้ายมาจาก `lib.rs` เดิม) บล็อก: ราก drive, `C:\Windows`, `Program Files` (และ x86), `ProgramData\Microsoft`, โฟลเดอร์ระดับบนของ C:, `pagefile.sys` / `hiberfil.sys` / `swapfile.sys`, `$Recycle.Bin`, `System Volume Information`, `WinSxS` — มีเทสต์ `protects_system_locations` ครอบ
- `safety::tree_path_matches` ตรวจว่า index path ยังชี้ไปยังรายการเดิมก่อนลบ — ถ้าผลสแกนเปลี่ยนไปแล้ว (มีการลบก่อนหน้า) ปฏิเสธและให้สแกนใหม่
- `actions::open_system_tool` รับเฉพาะชื่อใน whitelist 4 รายการ (`storage-sense`, `disk-cleanup`, `apps-features`, `recycle-bin`) ไม่รับ path หรือคำสั่งจาก WebView หรือ MCP
- `admin_commands` ของ MCP คืนค่าเป็นข้อความเท่านั้น ไม่รันคำสั่งเหล่านั้น

**MCP**

- อ่านอย่างเดียวเป็นค่าเริ่มต้น; เครื่องมือที่ลบหรือเปลี่ยนแปลงไฟล์ทำเครื่องหมาย destructive เพื่อให้ Claude ถามผู้ใช้ก่อนรัน
- Prompts ถือว่าชื่อพาธและชื่อไฟล์เป็นข้อมูล ไม่ใช่คำสั่ง
- รายละเอียดทั้งหมดและรายการ action ที่อนุญาตอยู่ใน [MCP.md](MCP.md)

**การตั้งค่า Claude (Claude Code / Claude Desktop)**

- `mcp_register` / `mcp_unregister` เปลี่ยนการตั้งค่า MCP ของ Claude Code หรือ Claude Desktop เฉพาะเมื่อผู้ใช้กดปุ่มเท่านั้น
- ก่อนแก้ไข config ของ Claude Desktop ต้องสำรองไฟล์เดิมไว้ทุกครั้ง
- ตำแหน่ง config ของ Claude Desktop: MSIX ใช้ `%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude\claude_desktop_config.json`; ติดตั้งแบบปกติใช้ `%APPDATA%\Claude\claude_desktop_config.json`

**ข้อมูลและเครือข่าย**

- การสแกนอ่านเฉพาะ metadata (ชื่อ ขนาด วันที่แก้ไข, และ Registry แบบอ่านอย่างเดียวสำหรับรายการโปรแกรม) ไม่อ่านเนื้อไฟล์
- DiskWhy ไม่เชื่อมต่อเครือข่ายออกนอกเครื่อง ไม่มีการส่งข้อมูลเอง ข้อมูลที่เครื่องมือคืนค่า (พาธ ชื่อไฟล์ ขนาด) จะไปถึง Claude เฉพาะเมื่อผู้ใช้ใช้งาน Claude Code/Desktop ตามปกติ
- CSP ของ WebView: `connect-src 'self' ipc:` — WebView ไม่เรียกเครือข่ายภายนอก

**Capabilities**

- Capabilities น้อยที่สุด (`src-tauri/capabilities/default.json`): event, reveal/open path, dialog เลือกโฟลเดอร์

## 5. ข้อจำกัดที่รู้ (และแนวทางต่อยอด)

| ข้อจำกัด | แนวทาง |
|---|---|
| ขนาดเป็น logical size; ไฟล์ NTFS-compressed/sparse อาจแสดงมากกว่าที่ใช้จริง | ใช้ `GetCompressedFileSizeW` สำหรับไฟล์ที่มี attribute compressed/sparse |
| Hard link (WinSxS, pnpm store) อาจนับซ้ำ | ตรวจ file index ผ่าน `GetFileInformationByHandle` (ต้นทุนสูง ทำเป็นโหมดเสริม) |
| โฟลเดอร์ที่ไม่มีสิทธิ์จะข้าม | เพิ่มปุ่ม "รันแบบผู้ดูแลระบบ" (ShellExecute runas) |
| สแกนแบบ directory walk (~36k ไฟล์/วินาที) | โหมดอ่าน NTFS MFT โดยตรง (แบบ WizTree) เร็วขึ้น 10–50× แต่ต้องสิทธิ์แอดมิน |
| ยังไม่มีการค้นหาไฟล์ซ้ำ | hash ไฟล์ขนาดเท่ากัน (BLAKE3) แบบขนาน |
| คำแนะนำจาก Claude ที่ใช้ MCP อาจผิดพลาด | DiskWhy ส่งระดับความปลอดภัย (`safe` / `review` / `system`) และคำอธิบายต่อรายการ; Claude ต้องถามผู้ใช้ก่อนลบทุกครั้ง ผู้ใช้ต้องตรวจก่อนลบเสมอ |
| ตัวติดตั้งยังไม่เซ็น (SmartScreen จะเตือน) | เซ็นด้วย code-signing certificate ใน `tauri.conf.json › bundle.windows` |
| ตัวติดตั้งยังไม่รวม `diskwhy-cli.exe` | เพิ่ม `bundle.externalBin` และตรวจว่าปุ่มเชื่อมต่อชี้ไปที่ `binaries\diskwhy-cli.exe` ที่ถูกต้อง |

## 6. หมายเหตุสภาพแวดล้อม build (เฉพาะ sandbox นี้)

โปรเจกต์ในเครื่องนี้อยู่ใน "โฟลเดอร์ชั่วคราวของ session" ซึ่ง Windows จำลอง path ไว้ ทำให้ `link.exe` เขียนไฟล์ไม่ได้
จึงต้องตั้ง `CARGO_TARGET_DIR=C:\Users\User\.cache\diskwhy-target` เฉพาะเมื่อ build ในสภาพแวดล้อมนี้

- **ไม่ต้องตั้งค่านี้ในโฟลเดอร์ปกติ** (เช่น `D:\dev\nova`) — `cargo test --workspace` ทำงานได้ตามปกติ
- ลบโฟลเดอร์ cache นั้นได้ทุกเมื่อ (≈ไม่กี่ GB)

## 7. Design System ที่ใช้ (Nova)

แอปใช้ `@nova/ui` + `@nova/tokens` จาก monorepo นี้ — ดูเอกสารคอมโพเนนต์ด้วย `pnpm storybook`
สีทุกคู่ผ่าน WCAG 2.2 AA (ตรวจอัตโนมัติตอน build tokens)

## 8. การทดสอบ

| คำสั่ง | ครอบคลุม |
|---|---|
| `pnpm test` (จาก root) | tokens + UI + DiskWhy (TS, vitest) |
| `cd apps/diskwhy && cargo test --workspace` | `crates/core` + `crates/cli` + `src-tauri` |
| `cd apps/diskwhy && cargo test --release bench_real -- --ignored --nocapture` | วัดความเร็วสแกนจริง (อ่านอย่างเดียว; ตั้ง `DISKWHY_BENCH` เป็นไดรฟ์ที่ต้องการ) |

- เทสต์อัตโนมัติทั้งหมดทำงานแบบออฟไลน์ ไม่เรียกเครือข่ายภายนอก
- **MCP integration test** เปิดตัว binary `diskwhy-cli` เป็นโปรเซสลูกและคุยด้วยผ่าน stdio ไม่เรียก Claude
- เทสต์ที่อ่านไดรฟ์จริง (เช่น bench) ใช้ `#[ignore]` และอ่านอย่างเดียว
