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
               get_report / get_node ─────┘  insights.rs (hogs, categories, top files, node view)
```

- **ต้นไม้ในหน่วยความจำ**: เก็บทุกโฟลเดอร์ + ไฟล์ ≥ 1 MB แยกรายตัว; ไฟล์เล็กรวมเป็นยอดต่อโฟลเดอร์ → ใช้ RAM น้อยแม้มีหลายล้านไฟล์
- **ระบุตำแหน่งด้วย index path** (`ids: [0, 3, 1]`) แทน string path — เร็วและไม่มีปัญหาตัวอักษรพิเศษ
- **หลังลบ** index ของพี่น้องอาจเลื่อน → UI โหลดโฟลเดอร์แม่ใหม่เสมอ (`results.tsx › confirmTrash`)

## 2. สัญญาข้อมูล (ต้องแก้คู่กันเสมอ)

| Rust (serde) | TypeScript |
|---|---|
| `lib.rs › DriveInfo, ScanStats, ProgressEvent, DoneEvent, Report` | `src/lib/types.ts` |
| `insights.rs › Hog, Location, TopFile, NodeView, ChildView, CategoryStat` | `src/lib/types.ts` |

ชื่อ argument ของ command ฝั่ง JS เป็น camelCase (`dirIds`, `fileName`) — Tauri แปลงเป็น snake_case ให้

## 3. เพิ่มกฎ "ตัวการ" ใหม่

1. เพิ่ม `Rule` ใน `src-tauri/src/insights.rs › RULES` (id, label ไทย, advice, safety, test)
2. เพิ่มเทสต์ใน `insights.rs › tests`
3. (ถ้าต้องการให้เห็นในโหมดสาธิต) เพิ่มกฎเดียวกันใน `src/lib/demo.ts`

ระดับความปลอดภัย: `safe` = สร้างใหม่ได้, `review` = ข้อมูลส่วนตัว ดูก่อน, `system` = ใช้เครื่องมือ Windows ห้ามลบเอง

## 4. ความปลอดภัย

- ลบได้เฉพาะ `trash::delete` (ย้ายไป Recycle Bin) ไม่มี hard delete ในโค้ด
- `is_protected()` ใน `lib.rs` บล็อก: ราก drive, C:\Windows, Program Files, ProgramData\Microsoft, โฟลเดอร์ระดับบนของ C:, pagefile/hiberfil/swapfile, $Recycle.Bin, WinSxS — มีเทสต์ครอบ
- Capabilities น้อยที่สุด (`capabilities/default.json`): event, reveal/open path, dialog เลือกโฟลเดอร์
- CSP เข้มงวด ไม่มีการเรียกเครือข่ายภายนอก

## 5. ข้อจำกัดที่รู้ (และแนวทางต่อยอด)

| ข้อจำกัด | แนวทาง |
|---|---|
| ขนาดเป็น logical size; ไฟล์ NTFS-compressed/sparse อาจแสดงมากกว่าที่ใช้จริง | ใช้ `GetCompressedFileSizeW` สำหรับไฟล์ที่มี attribute compressed/sparse |
| Hard link (WinSxS, pnpm store) อาจนับซ้ำ | ตรวจ file index ผ่าน `GetFileInformationByHandle` (ต้นทุนสูง ทำเป็นโหมดเสริม) |
| โฟลเดอร์ที่ไม่มีสิทธิ์จะข้าม | เพิ่มปุ่ม "รันแบบผู้ดูแลระบบ" (ShellExecute runas) |
| สแกนแบบ directory walk (~36k ไฟล์/วินาที) | โหมดอ่าน NTFS MFT โดยตรง (แบบ WizTree) เร็วขึ้น 10–50× แต่ต้องสิทธิ์แอดมิน |
| ยังไม่มีการค้นหาไฟล์ซ้ำ | hash ไฟล์ขนาดเท่ากัน (BLAKE3) แบบขนาน |
| ตัวติดตั้งยังไม่เซ็น (SmartScreen จะเตือน) | เซ็นด้วย code-signing certificate ใน `tauri.conf.json › bundle.windows` |

## 6. หมายเหตุสภาพแวดล้อม build (เครื่องนี้)

โปรเจกต์ถูกสร้างใน "โฟลเดอร์ชั่วคราวของ session" ซึ่ง Windows จำลอง path ไว้ ทำให้ `link.exe` เขียนไฟล์ไม่ได้
จึง build ด้วย `CARGO_TARGET_DIR=C:\Users\User\.cache\diskwhy-target` — **ถ้าย้ายโปรเจกต์ไปโฟลเดอร์ปกติ (เช่น D:\dev) ไม่ต้องตั้งค่านี้**
ลบโฟลเดอร์ cache นั้นได้ทุกเมื่อ (≈ไม่กี่ GB)

## 7. Design System ที่ใช้ (Nova)

แอปใช้ `@nova/ui` + `@nova/tokens` จาก monorepo นี้ — ดูเอกสารคอมโพเนนต์ด้วย `pnpm storybook`
สีทุกคู่ผ่าน WCAG 2.2 AA (ตรวจอัตโนมัติตอน build tokens)
