# Handoff — DiskWhy

## 1. สถาปัตยกรรม

```
React UI (WebView2)  ──invoke()──▶  Rust commands (lib.rs)
      ▲                                  │
      └──── events: scan-progress ◀──────┤  thread: scan::scan_dir (rayon fork-join)
                   scan-done              │  ticker thread: ~8 events/s
                                          ▼
                              AppState { tree in memory, extension stats }
                                          │
               get_report / get_node ─────┤  insights.rs (hogs, categories, top files, node view)
                                          │
               ai_analyze ────────────────┘  ai.rs ──HTTPS (reqwest)──▶ api.anthropic.com
```

- **`insights.rs`** — สร้างรายงานตัวการ (hogs) จากต้นไม้ที่สแกนแล้ว รวมกฎ AppData ของโปรแกรมที่ถอนการติดตั้งแล้ว (ใช้รายการจาก `installed.rs`)
- **`installed.rs`** — อ่านรายการโปรแกรมที่ติดตั้งจาก Registry แบบอ่านอย่างเดียว ใช้เทียบกับโฟลเดอร์ AppData
- **`ai.rs`** — ผู้ช่วย AI สองขั้น: Haiku 5.5 จัดประเภทรายการ → Sonnet 5.5 เขียนแผน เรียก `api.anthropic.com` ผ่าน reqwest และทำงานเฉพาะเมื่อผู้ใช้กดปุ่มเท่านั้น คีย์เก็บใน Windows Credential Manager ผ่าน keyring
- **ต้นไม้ในหน่วยความจำ**: เก็บทุกโฟลเดอร์ + ไฟล์ ≥ 1 MB แยกรายตัว; ไฟล์เล็กรวมเป็นยอดต่อโฟลเดอร์ → ใช้ RAM น้อยแม้มีหลายล้านไฟล์
- **ระบุตำแหน่งด้วย index path** (`ids: [0, 3, 1]`) แทน string path — เร็วและไม่มีปัญหาตัวอักษรพิเศษ
- **หลังลบ** index ของพี่น้องอาจเลื่อน → UI โหลดโฟลเดอร์แม่ใหม่เสมอ (`results.tsx › confirmTrash`)
- **การเริ่มต้นฝั่ง UI**: `getBackend()` reject ด้วย `Error("NOT_IN_APP")` เมื่อเปิดนอกแอปเดสก์ท็อป → `App.tsx` แสดง `NotInAppScreen`; error อื่นของการเริ่มต้นแสดง `StartupErrorScreen` พร้อมปุ่มลองอีกครั้ง

## 2. สัญญาข้อมูล (ต้องแก้คู่กันเสมอ)

| Rust (serde) | TypeScript |
|---|---|
| `lib.rs › DriveInfo, ScanStats, ProgressEvent, DoneEvent, Report` | `src/lib/types.ts` |
| `insights.rs › Hog, Location, TopFile, NodeView, ChildView, CategoryStat` | `src/lib/types.ts` |
| `ai.rs › AiKeyStatus, AiProgress, AiItem, AiPlan, AiStep, AiCommand, AiUsage, AiResult` (camelCase) | `src/lib/types.ts` (ส่วน AI advisor) |

ชื่อ argument ของ command ฝั่ง JS เป็น camelCase (`dirIds`, `fileName`) — Tauri แปลงเป็น snake_case ให้

## 3. เพิ่มกฎ "ตัวการ" ใหม่

1. เพิ่ม `Rule` ใน `src-tauri/src/insights.rs › RULES` (id, label ไทย, advice, safety, test)
2. เพิ่มเทสต์ใน `insights.rs › tests`

ไม่มีโหมดจำลองให้ต้องซิงก์ตาม UI รับรายการตัวการจาก backend โดยตรง จึงไม่ต้องเพิ่มข้อมูลตัวอย่างฝั่ง TypeScript

ระดับความปลอดภัย: `safe` = สร้างใหม่ได้, `review` = ข้อมูลส่วนตัว ดูก่อน, `system` = ใช้เครื่องมือ Windows ห้ามลบเอง

## 4. ความปลอดภัย

- ลบได้เฉพาะ `trash::delete` (ย้ายไป Recycle Bin) ไม่มี hard delete ในโค้ด
- `is_protected()` ใน `lib.rs` บล็อก: ราก drive, C:\Windows, Program Files, ProgramData\Microsoft, โฟลเดอร์ระดับบนของ C:, pagefile/hiberfil/swapfile, $Recycle.Bin, WinSxS — มีเทสต์ครอบ
- `open_system_tool` รับเฉพาะชื่อเครื่องมือใน whitelist 4 รายการ (`storage-sense`, `disk-cleanup`, `apps-features`, `recycle-bin`) ไม่รับ path หรือคำสั่งจาก WebView
- Capabilities น้อยที่สุด (`capabilities/default.json`): event, reveal/open path, dialog เลือกโฟลเดอร์
- CSP: `connect-src 'self' ipc:` — WebView ไม่เรียกเครือข่ายภายนอก การเรียก Claude ทำจาก Rust เท่านั้น

**คีย์ API ของ Claude**

- บันทึกผ่าน `ai_set_key` → Rust ตรวจรูปแบบคีย์แล้วเก็บใน Windows Credential Manager (ผ่าน keyring) ไม่เก็บในไฟล์ตั้งค่าหรือใน WebView
- หลังบันทึก WebView เห็นเพียงสถานะ (`configured`, `source` = `keychain` / `env` / `none`) ไม่เห็นค่าคีย์
- ถ้าตั้งตัวแปร `ANTHROPIC_API_KEY` ไว้ แหล่งจะเป็น `env`
- `ai_clear_key` ลบคีย์ออกจาก Credential Manager

**ข้อมูลที่ส่งให้ Claude**

- ส่งเฉพาะพาธ ขนาด วันที่ ประเภทไฟล์ และธงของกฎที่ตรวจพบ — **ไม่ส่งเนื้อหาไฟล์**
- `anonymize: true` ปิดบังชื่อผู้ใช้ในพาธก่อนส่ง

**เครือข่าย**

- มีการเชื่อมต่อออกนอกเครื่องเฉพาะ `api.anthropic.com` และเฉพาะตอนผู้ใช้กดวิเคราะห์ ไม่มีการส่งอัตโนมัติ
- คำสั่งในแผนไม่ถูกรันโดยอัตโนมัติ ผู้ใช้ตรวจและคัดลอกไปรันเอง

## 5. ข้อจำกัดที่รู้ (และแนวทางต่อยอด)

| ข้อจำกัด | แนวทาง |
|---|---|
| ขนาดเป็น logical size; ไฟล์ NTFS-compressed/sparse อาจแสดงมากกว่าที่ใช้จริง | ใช้ `GetCompressedFileSizeW` สำหรับไฟล์ที่มี attribute compressed/sparse |
| Hard link (WinSxS, pnpm store) อาจนับซ้ำ | ตรวจ file index ผ่าน `GetFileInformationByHandle` (ต้นทุนสูง ทำเป็นโหมดเสริม) |
| โฟลเดอร์ที่ไม่มีสิทธิ์จะข้าม | เพิ่มปุ่ม "รันแบบผู้ดูแลระบบ" (ShellExecute runas) |
| สแกนแบบ directory walk (~36k ไฟล์/วินาที) | โหมดอ่าน NTFS MFT โดยตรง (แบบ WizTree) เร็วขึ้น 10–50× แต่ต้องสิทธิ์แอดมิน |
| ยังไม่มีการค้นหาไฟล์ซ้ำ | hash ไฟล์ขนาดเท่ากัน (BLAKE3) แบบขนาน |
| ผลจาก AI เป็นคำแนะนำ อาจผิดพลาด | มีฟิลด์ความมั่นใจ (`confidence`) และเหตุผล (`reason`) ต่อรายการ; ผู้ใช้ต้องตรวจก่อนลบเสมอ |
| ตัวติดตั้งยังไม่เซ็น (SmartScreen จะเตือน) | เซ็นด้วย code-signing certificate ใน `tauri.conf.json › bundle.windows` |

## 6. หมายเหตุสภาพแวดล้อม build (เครื่องนี้)

โปรเจกต์ถูกสร้างใน "โฟลเดอร์ชั่วคราวของ session" ซึ่ง Windows จำลอง path ไว้ ทำให้ `link.exe` เขียนไฟล์ไม่ได้
จึง build ด้วย `CARGO_TARGET_DIR=C:\Users\User\.cache\diskwhy-target` — **ถ้าย้ายโปรเจกต์ไปโฟลเดอร์ปกติ (เช่น D:\dev) ไม่ต้องตั้งค่านี้**
ลบโฟลเดอร์ cache นั้นได้ทุกเมื่อ (≈ไม่กี่ GB)

## 7. Design System ที่ใช้ (Nova)

แอปใช้ `@nova/ui` + `@nova/tokens` จาก monorepo นี้ — ดูเอกสารคอมโพเนนต์ด้วย `pnpm storybook`
สีทุกคู่ผ่าน WCAG 2.2 AA (ตรวจอัตโนมัติตอน build tokens)

## 8. การทดสอบ AI

- เทสต์อัตโนมัติ (`pnpm test`, `cargo test`) ทำงานแบบออฟไลน์ ไม่เรียก `api.anthropic.com` และไม่ใช้คีย์จริง
- การทดสอบแบบรันจริง (live) ต้องใช้คีย์ Anthropic ของผู้ทดสอบ และมีค่าใช้จ่ายจริงในบัญชีนั้น ควรรันเฉพาะเมื่อจำเป็น
- ห้ามใส่คีย์ในโค้ด เทสต์ หรือไฟล์ที่ commit ใช้ Credential Manager หรือตัวแปรสภาพแวดล้อมในเครื่องเท่านั้น
