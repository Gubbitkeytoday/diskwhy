# DiskWhy — ดิสก์เต็มเพราะอะไร

แอปเดสก์ท็อป Windows ที่สแกนไดรฟ์แล้วตอบว่า **อะไรกินพื้นที่ อะไรลบได้อย่างปลอดภัย และควรจัดการอย่างไร**
สร้างด้วย Tauri 2 (Rust) + React 19 + Nova Design System

| | |
|---|---|
| ตัวติดตั้ง | `release/DiskWhy_0.1.0_x64-setup.exe` (≈2 MB, ติดตั้งแบบผู้ใช้ปัจจุบัน ไม่ต้องใช้สิทธิ์แอดมิน) |
| ความเร็วจริง | 2.67 ล้านไฟล์ / 167 GB ใน 73.6 วินาที (≈36,000 ไฟล์/วินาที, cold cache) |
| หน่วยความจำ | ≈31 MB ตอนเปิด |

![หน้าสรุปสาเหตุ](docs/screenshots/diskwhy-why.png)

DiskWhy อ่านข้อมูลจากเครื่องจริงเท่านั้น ไม่มีโหมดสาธิตหรือข้อมูลตัวอย่าง หากเปิดหน้าเว็บนอกแอปเดสก์ท็อป จะขึ้นหน้าให้เปิดใน DiskWhy แทน

## ฟีเจอร์

- **เลือกไดรฟ์ / โฟลเดอร์** — แสดงพื้นที่ใช้/ว่างของทุกไดรฟ์ เตือนเมื่อใกล้เต็ม
- **สแกนเร็วแบบหลายคอร์** (Rust + rayon) แสดงความคืบหน้าสด หยุดได้ทุกเมื่อ (Esc)
- **"ทำไมเต็ม"** — สรุปภาษาคน + ตัวการที่พบ เรียงตามขนาด พร้อมระดับความปลอดภัยและวิธีจัดการ:
  ถังขยะ, Temp, Windows Update, Windows.old, WinSxS, node_modules, แคชนักพัฒนา (npm/pnpm/pip/gradle/nuget/cargo),
  แคชเบราว์เซอร์, Downloads, ดิสก์เสมือน WSL/Docker (.vhdx), เกม, crash dump, hiberfil/pagefile/swapfile
  และตัวการที่เพิ่มล่าสุด:
  - ไฟล์ใหญ่ที่ไม่ได้แตะเกิน 1 ปี
  - โปรเจกต์เก่าที่ยังมี node_modules/build (ลบเฉพาะส่วนที่สร้างใหม่ได้)
  - ไฟล์ติดตั้งเก่าใน Downloads
  - ข้อมูล AppData ของโปรแกรมที่ถอนการติดตั้งแล้ว (เทียบกับรายการโปรแกรมใน Registry)
- **ผู้ช่วย AI (Claude)** — ให้ Claude จัดประเภทรายการที่พบและเขียนแผนลบทีละขั้น ทำงานเฉพาะเมื่อคุณกดปุ่มเอง (ดูหัวข้อด้านล่าง)
- **แยกตามประเภทไฟล์** — วิดีโอ รูป เกม ไฟล์ติดตั้ง ดิสก์อิมเมจ ฯลฯ
- **แผนผัง Treemap** คลิกเจาะลึกทีละโฟลเดอร์ + รายการเรียงตามขนาด + breadcrumb (Backspace = ขึ้นหนึ่งระดับ)
- **ไฟล์ใหญ่สุด 100 อันดับ** — ดูในแผนผัง / เปิดใน Explorer / ย้ายไปถังขยะ
- **ปลอดภัยโดยออกแบบ** — อ่านอย่างเดียว ไม่ส่งข้อมูลออกนอกเครื่อง เว้นแต่คุณกดใช้ผู้ช่วย AI ลบได้เฉพาะ "ย้ายไปถังขยะ" หลังยืนยัน
  และบล็อกตำแหน่งระบบ (C:\Windows, Program Files, ราก drive, pagefile ฯลฯ) ที่ฝั่ง Rust
- **นับถูกต้อง** — ไม่ตาม symlink/junction (ไม่นับซ้ำ ไม่วนลูป), ไฟล์ OneDrive แบบ online-only แยกออก (ไม่กินพื้นที่จริง)
- ธีมสว่าง/มืด, UI ภาษาไทยเต็มรูปแบบ, ใช้คีย์บอร์ดได้ทั้งหมด

## ผู้ช่วย AI (Claude)

ปุ่มวิเคราะห์ด้วย AI ส่งรายการที่ DiskWhy พบไปให้ Claude ประมวลผลเป็นสองขั้น:

1. **Claude Haiku 5.5 จัดประเภท** แต่ละรายการเป็น *ลบได้* / *ตรวจสอบ* / *เก็บ* / *ใช้เครื่องมือ Windows* พร้อมระดับความมั่นใจและเหตุผลสั้น ๆ
2. **Claude Sonnet 5.5 เขียนแผน** ทีละขั้นตอน พร้อมคำสั่ง Windows ที่ใช้ได้จริง (PowerShell, cmd หรือหน้า Settings) และระบุว่าคำสั่งไหนต้องเปิดสิทธิ์ผู้ดูแลระบบ

**ส่งอะไรไปบ้าง:** พาธของไฟล์และโฟลเดอร์ ขนาด วันที่แก้ไขล่าสุด ประเภทไฟล์ และธงของกฎที่ตรวจพบ
**ไม่ส่งเนื้อหาของไฟล์** มีตัวเลือกปิดบังชื่อผู้ใช้ในพาธก่อนส่ง

**คีย์ API:** ใช้คีย์ของ Anthropic ของคุณเอง เก็บใน Windows Credential Manager หรือใช้ค่าจากตัวแปรสภาพแวดล้อม `ANTHROPIC_API_KEY` ก็ได้
คีย์ไม่ถูกเก็บในไฟล์ของแอปและไม่ถูกเก็บในส่วนหน้าของแอป (WebView)

**ค่าใช้จ่าย:** คิดกับบัญชี API ของคุณเอง ประมาณ $0.15–0.25 (≈5–9 บาท) ต่อการวิเคราะห์หนึ่งครั้งที่ ~300 รายการ (Haiku ≈ $0.02 + Sonnet ≈ $0.16; ประเมินจากราคาต่อ token ยังไม่ได้วัดกับการเรียกจริง) แอปแสดงยอดจริงหลังวิเคราะห์ทุกครั้ง
ผลลัพธ์มีข้อมูลการใช้งาน (จำนวนโทเคนและค่าใช้จ่ายโดยประมาณ)

**การควบคุม:** ส่งเฉพาะเมื่อคุณกดปุ่มเอง ไม่มีการส่งอัตโนมัติ คำสั่งในแผนไม่ถูกรันให้โดยอัตโนมัติ คุณตรวจแล้วจึงคัดลอกไปรันเอง

**เครือข่าย:** แอปเชื่อมต่อออกนอกเครื่องเฉพาะ `api.anthropic.com` และเฉพาะตอนที่คุณกดวิเคราะห์

## โครงสร้าง (monorepo, pnpm + Turborepo)

```
apps/diskwhy/            แอป DiskWhy
  src-tauri/src/scan.rs        ตัวสแกนแบบขนาน + ลบโหนดออกจากต้นไม้
  src-tauri/src/insights.rs    กฎตรวจจับตัวการ, หมวดไฟล์, ไฟล์ใหญ่สุด, มุมมองโหนด
  src-tauri/src/installed.rs   รายการโปรแกรมที่ติดตั้ง (อ่าน Registry อย่างเดียว) ใช้ตรวจ AppData ที่ค้างอยู่
  src-tauri/src/ai.rs          ผู้ช่วย AI: จัดประเภทและเขียนแผนผ่าน api.anthropic.com (เมื่อผู้ใช้กดเท่านั้น)
  src-tauri/src/lib.rs         Tauri commands + event stream + การป้องกันการลบ
  src/lib/backend.ts           อินเทอร์เฟซเดียวไปยัง Rust backend (นอกแอปจะ reject ด้วย NOT_IN_APP)
  src/components/              Treemap, SizeList, Insights, Screens, Results
apps/docs/               Storybook 10 — เอกสาร Design System
packages/tokens/         Design tokens (W3C DTCG → CSS / Tailwind v4 / JS / Figma) + WCAG contrast gate
packages/ui/             คอมโพเนนต์ React (Radix + Tailwind v4) ที่แอปใช้
registry/                shadcn registry ของคอมโพเนนต์ Nova
```

## พัฒนาต่อ

ต้องมี Node ≥ 22, pnpm 11, Rust (stable/nightly), Visual Studio Build Tools 2022 (C++), WebView2

```bash
pnpm install
pnpm --filter @nova/diskwhy app:dev     # เปิดแอปเดสก์ท็อปแบบ hot-reload
pnpm --filter @nova/diskwhy dev         # เปิด UI ในเบราว์เซอร์ (จะขึ้นหน้า "เปิดใน DiskWhy" เพราะไม่มี backend)
pnpm --filter @nova/diskwhy app:build   # สร้างตัวติดตั้ง .exe (NSIS)
```

ทดสอบ:

```bash
pnpm test                                              # tokens + UI + DiskWhy (TS)
cd apps/diskwhy/src-tauri && cargo test               # เทสต์ฝั่ง Rust
DISKWHY_BENCH='C:\' cargo test --release bench_real -- --ignored --nocapture   # วัดความเร็วจริง (อ่านอย่างเดียว)
```

การทดสอบปกติไม่เรียก API ของ Claude จริง ดูหัวข้อ "การทดสอบ AI" ใน [docs/HANDOFF.md](docs/HANDOFF.md)

ดูรายละเอียดสำหรับทีมที่ [docs/HANDOFF.md](docs/HANDOFF.md)
