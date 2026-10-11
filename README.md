# DiskWhy — ดิสก์เต็มเพราะอะไร

แอปเดสก์ท็อป Windows ที่สแกนไดรฟ์แล้วตอบว่า **อะไรกินพื้นที่ อะไรลบได้อย่างปลอดภัย และควรจัดการอย่างไร**
สร้างด้วย Tauri 2 (Rust) + React 19 + Nova Design System

| | |
|---|---|
| ตัวติดตั้ง | `release/DiskWhy_0.1.0_x64-setup.exe` (≈2 MB, ติดตั้งแบบผู้ใช้ปัจจุบัน ไม่ต้องใช้สิทธิ์แอดมิน) |
| ความเร็วจริง | 2.67 ล้านไฟล์ / 167 GB ใน 73.6 วินาที (≈36,000 ไฟล์/วินาที, cold cache) |
| หน่วยความจำ | ≈31 MB ตอนเปิด |

![หน้าสรุปสาเหตุ](docs/screenshots/diskwhy-why.png)

## ฟีเจอร์

- **เลือกไดรฟ์ / โฟลเดอร์** — แสดงพื้นที่ใช้/ว่างของทุกไดรฟ์ เตือนเมื่อใกล้เต็ม
- **สแกนเร็วแบบหลายคอร์** (Rust + rayon) แสดงความคืบหน้าสด หยุดได้ทุกเมื่อ (Esc)
- **"ทำไมเต็ม"** — สรุปภาษาคน + ตัวการที่พบ เรียงตามขนาด พร้อมระดับความปลอดภัยและวิธีจัดการ:
  ถังขยะ, Temp, Windows Update, Windows.old, WinSxS, node_modules, แคชนักพัฒนา (npm/pnpm/pip/gradle/nuget/cargo),
  แคชเบราว์เซอร์, Downloads, ดิสก์เสมือน WSL/Docker (.vhdx), เกม, crash dump, hiberfil/pagefile/swapfile
- **แยกตามประเภทไฟล์** — วิดีโอ รูป เกม ไฟล์ติดตั้ง ดิสก์อิมเมจ ฯลฯ
- **แผนผัง Treemap** คลิกเจาะลึกทีละโฟลเดอร์ + รายการเรียงตามขนาด + breadcrumb (Backspace = ขึ้นหนึ่งระดับ)
- **ไฟล์ใหญ่สุด 100 อันดับ** — ดูในแผนผัง / เปิดใน Explorer / ย้ายไปถังขยะ
- **ปลอดภัยโดยออกแบบ** — อ่านอย่างเดียว ไม่ส่งข้อมูลออกนอกเครื่อง ลบได้เฉพาะ "ย้ายไปถังขยะ" หลังยืนยัน
  และบล็อกตำแหน่งระบบ (C:\Windows, Program Files, ราก drive, pagefile ฯลฯ) ที่ฝั่ง Rust
- **นับถูกต้อง** — ไม่ตาม symlink/junction (ไม่นับซ้ำ ไม่วนลูป), ไฟล์ OneDrive แบบ online-only แยกออก (ไม่กินพื้นที่จริง)
- ธีมสว่าง/มืด, UI ภาษาไทยเต็มรูปแบบ, ใช้คีย์บอร์ดได้ทั้งหมด

## โครงสร้าง (monorepo, pnpm + Turborepo)

```
apps/diskwhy/            แอป DiskWhy
  src-tauri/src/scan.rs      ตัวสแกนแบบขนาน + ลบโหนดออกจากต้นไม้
  src-tauri/src/insights.rs  กฎตรวจจับตัวการ, หมวดไฟล์, ไฟล์ใหญ่สุด, มุมมองโหนด
  src-tauri/src/lib.rs       Tauri commands + event stream + การป้องกันการลบ
  src/lib/backend.ts         อินเทอร์เฟซเดียว: Native (Tauri) / Demo (เบราว์เซอร์)
  src/components/            Treemap, SizeList, Insights, Screens, Results
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
pnpm --filter @nova/diskwhy dev         # เปิด UI ในเบราว์เซอร์ (โหมดสาธิต ข้อมูลจำลอง)
pnpm --filter @nova/diskwhy app:build   # สร้างตัวติดตั้ง .exe (NSIS)
```

ทดสอบ:

```bash
pnpm test                                              # tokens + UI + DiskWhy (TS)
cd apps/diskwhy/src-tauri && cargo test               # Rust 10 tests
DISKWHY_BENCH='C:\' cargo test --release bench_real -- --ignored --nocapture   # วัดความเร็วจริง (อ่านอย่างเดียว)
```

ดูรายละเอียดสำหรับทีมที่ [docs/HANDOFF.md](docs/HANDOFF.md)
