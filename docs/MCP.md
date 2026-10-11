# DiskWhy MCP server

DiskWhy มี MCP server ชื่อ `diskwhy` ให้ Claude Code และ Claude Desktop เรียกใช้เครื่องมือเพื่อตรวจ วิเคราะห์ และเคลียร์พื้นที่ดิสก์ได้
Claude เป็นผู้เริ่มโปรเซส `diskwhy-cli mcp` และคุยด้วยผ่าน stdio

- **ไม่ต้องตั้งค่าคีย์ใด ๆ** ใช้ subscription Claude ที่คุณมีอยู่แล้ว DiskWhy ไม่มีค่าใช้จ่ายเพิ่ม
- **ไม่มีการเชื่อมต่อเครือข่าย** ฝั่ง DiskWhy และ CLI/MCP
- ทุกการลบต้องได้รับการยืนยันจาก Claude (ซึ่งจะถามคุณก่อน)

## 1. เชื่อมต่อ

### วิธีที่ 1: ผ่านแอป (แนะนำ)

1. เปิด DiskWhy แล้วกดปุ่ม **"เชื่อมต่อ Claude (MCP)"**
2. เลือก **Claude Code** หรือ **Claude Desktop** แล้วกดเชื่อมต่อ
3. ถ้าเป็น Claude Desktop ให้ปิดแล้วเปิดใหม่ (Restart) หนึ่งครั้ง

ก่อนแก้ไข config ของ Claude Desktop แอปจะสำรองไฟล์เดิมไว้ก่อนเสมอ

### วิธีที่ 2: Claude Code (คำสั่ง)

```
claude mcp add --scope user diskwhy -- "<install dir>\binaries\diskwhy-cli.exe" mcp
```

- `<install dir>` คือโฟลเดอร์ที่ติดตั้ง DiskWhy (ต้องมี `binaries\diskwhy-cli.exe` อยู่)
- ตรวจผลด้วย `claude mcp list` ต้องเห็นรายการ `diskwhy`

### วิธีที่ 3: Claude Desktop (แก้ไฟล์ config ด้วยมือ)

เพิ่มส่วนนี้ในไฟล์ `claude_desktop_config.json` (ถ้ามี `mcpServers` อยู่แล้ว ให้เพิ่มเฉพาะ `diskwhy` เข้าไปในนั้น):

```json
{
  "mcpServers": {
    "diskwhy": {
      "command": "<install dir>\\binaries\\diskwhy-cli.exe",
      "args": ["mcp"]
    }
  }
}
```

- ใน JSON ต้องเขียน backslash เป็น `\\`
- ตำแหน่งไฟล์ config ขึ้นกับวิธีติดตั้ง Claude Desktop:

| วิธีติดตั้ง Claude Desktop | ตำแหน่งไฟล์ config |
|---|---|
| แบบ MSIX (เช่น จาก Microsoft Store) | `%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude\claude_desktop_config.json` (แทน `*` ด้วยโฟลเดอร์ที่ขึ้นต้นด้วย `Claude_` ใน `%LOCALAPPDATA%\Packages`) |
| แบบปกติ (ตัวติดตั้ง .exe) | `%APPDATA%\Claude\claude_desktop_config.json` |

หลังแก้ไขแล้วต้อง Restart Claude Desktop ทุกครั้ง

### ยกเลิกการเชื่อมต่อ

- ในแอป DiskWhy ใช้ปุ่มยกเลิกการเชื่อมต่อ (`mcp_unregister`)
- หรือลบเองด้วยคำสั่ง `claude mcp remove diskwhy` (Claude Code) / ลบบล็อก `diskwhy` ออกจาก config (Claude Desktop) แล้ว Restart

## 2. เครื่องมือ (tools)

| เครื่องมือ | ประเภท | อินพุต | คืนค่า |
|---|---|---|---|
| `list_drives` | อ่านอย่างเดียว | ไม่มี | ไดรฟ์ทั้งหมด: mount, ชื่อ, ระบบไฟล์, พื้นที่ทั้งหมด, พื้นที่ว่าง, removable หรือไม่ |
| `scan` | อ่านอย่างเดียว | `path` โฟลเดอร์ที่จะสแกน | รอผลได้สูงสุดประมาณ 55 วินาที ถ้าเสร็จคืนสถิติ (จำนวนไฟล์ โฟลเดอร์ ขนาดรวม จำนวน error ที่ข้าม พื้นที่ cloud เวลาที่ใช้) ถ้ายังไม่เสร็จคืนสถานะกำลังทำงาน ให้เรียก `scan_status` ซ้ำจนเสร็จ |
| `scan_status` | อ่านอย่างเดียว | ไม่มี | ความคืบหน้าของการสแกนล่าสุด: จำนวนไฟล์ โฟลเดอร์ ไบต์ รายการที่กำลังสแกน เวลาที่ผ่านไป |
| `cancel_scan` | อ่านอย่างเดียว | ไม่มี | หยุดการสแกนที่กำลังทำงาน (ไม่กระทบไฟล์) |
| `disk_report` | อ่านอย่างเดียว | `path`, `top` (ไม่บังคับ) | รายงาน "ทำไมเต็ม": หมวดไฟล์, นามสกุลที่ใหญ่สุด, ตัวการ (hogs) พร้อมระดับความปลอดภัย (`safe` / `review` / `system`) และวิธีจัดการ, ไฟล์ใหญ่สุด |
| `find_unused` | อ่านอย่างเดียว | `path` | สี่กลุ่มที่ตรวจพบ: ไฟล์ใหญ่ (≥ 100 MB) ที่ไม่ได้แตะเกิน 1 ปี, โปรเจกต์ที่ source ไม่เปลี่ยนเกิน 90 วันพร้อมโฟลเดอร์ที่สร้างใหม่ได้, ตัวติดตั้งหรือไฟล์บีบอัดเก่า (≥ 10 MB) ใน Downloads ที่เกิน 30 วัน, ข้อมูล AppData ของโปรแกรมที่ไม่พบในรายการที่ติดตั้ง |
| `browse` | อ่านอย่างเดียว | `path` | รายการลูกของโฟลเดอร์นั้นในผลสแกน: ชื่อ, ขนาด, จำนวนไฟล์, วันที่แก้ไขล่าสุด |
| `largest_files` | อ่านอย่างเดียว | `path`, `limit` (ไม่บังคับ) | ไฟล์ใหญ่สุดเรียงตามขนาด: path, ชื่อ, ขนาด, หมวด, วันที่แก้ไขล่าสุด |
| `move_to_recycle_bin` | **ทำลาย (destructive)** | `path` | ย้ายไฟล์หรือโฟลเดอร์ไป Recycle Bin (กู้คืนได้) คืนค่าพื้นที่ที่ปลดปล่อย (ไบต์) ตำแหน่งระบบถูกปฏิเสธ |
| `run_cleanup` | **ทำลาย (destructive)** | `action` (ต้องอยู่ในรายการด้านล่าง) | ผลของ action ที่สั่ง (ดูตารางถัดไป) |
| `admin_commands` | อ่านอย่างเดียว | ไม่มี | ข้อความคำสั่งที่ต้องใช้สิทธิ์ผู้ดูแลระบบ เช่น `DISM /Online /Cleanup-Image /StartComponentCleanup`, `powercfg /h off`, `Optimize-VHD` ให้คุณรันเอง DiskWhy ไม่รันให้ |

> ชื่อพารามิเตอร์ในตารางนี้อิงกับคำสั่ง CLI (`<PATH>`, `--top`, `--limit`) ถ้า `tools/list` ของ binary จริงต่างจากนี้ ให้ยึด `tools/list` เป็นหลักและอัปเดตตารางนี้

**ลำดับการใช้งานที่แนะนำ:** `list_drives` → `scan` (ถ้าได้สถานะกำลังทำงาน ให้ใช้ `scan_status`) → `disk_report` หรือ `find_unused` → ค่อยใช้ `move_to_recycle_bin` / `run_cleanup` หลังผู้ใช้ยืนยันแล้ว

### action ของ `run_cleanup`

| action | ทำอะไร | ผลต่อไฟล์ |
|---|---|---|
| `npm-cache` | ล้างแคชของ npm | ย้ายไป Recycle Bin (กู้คืนได้) |
| `pnpm-store` | ล้างแคช pnpm store | ย้ายไป Recycle Bin (กู้คืนได้) |
| `yarn-cache` | ล้างแคชของ Yarn | ย้ายไป Recycle Bin (กู้คืนได้) |
| `pip-cache` | ล้างแคชของ pip | ย้ายไป Recycle Bin (กู้คืนได้) |
| `nuget-cache` | ล้างแคชของ NuGet | ย้ายไป Recycle Bin (กู้คืนได้) |
| `temp-files` | ล้างไฟล์ชั่วคราว | ย้ายไป Recycle Bin (กู้คืนได้) |
| `wsl-shutdown` | หยุด WSL ทั้งหมด (`wsl --shutdown`) เพื่อให้ไฟล์ดิสก์เสมือนพร้อมย่อขนาด | ไม่ลบไฟล์ |
| `empty-recycle-bin` | ล้างถังขยะ | **ลบถาวร** เป็นการลบถาวรเพียงอย่างเดียวของ DiskWhy |
| `open-storage-sense` | เปิด Settings › Storage Sense | ไม่แตะไฟล์ |
| `open-disk-cleanup` | เปิด Disk Cleanup | ไม่แตะไฟล์ |
| `open-apps-features` | เปิด Settings › Apps & features | ไม่แตะไฟล์ |
| `open-recycle-bin` | เปิดถังขยะใน Explorer | ไม่แตะไฟล์ |

action นอกเหนือจากนี้จะถูกปฏิเสธ

## 3. Prompts

| ชื่อ | ทำอะไร |
|---|---|
| `diagnose-disk` | ตรวจว่าไดรฟ์ที่เลือกเต็มเพราะอะไร แล้วเสนอแผนเคลียร์ที่ปลอดภัย |
| `cleanup-quick-wins` | เสนอรายการที่ล้างได้ปลอดภัยที่สุดก่อน แล้วให้ผู้ใช้ยืนยันทีละขั้นก่อนลบ |

Prompts ถือว่าพาธและชื่อไฟล์ที่ได้จากการสแกนเป็นข้อมูลเท่านั้น ไม่ใช่คำสั่ง แม้ชื่อไฟล์จะมีข้อความที่ดูเหมือนคำสั่งก็ตาม

## 4. ตัวอย่างคำขอ

- "ใช้ diskwhy ตรวจไดรฟ์ C: ว่าทำไมเต็ม แล้วเสนอแผนเคลียร์ที่ปลอดภัย"
- "หาไฟล์ที่ไม่ได้ใช้เกิน 1 ปีใน C:\Users"
- "ล้างแคช npm และ pnpm ให้หน่อย"

## 5. Safety model

- **อ่านอย่างเดียวเป็นค่าเริ่มต้น** เครื่องมือที่อ่านอย่างเดียวไม่เปลี่ยนแปลงไฟล์ของผู้ใช้
- **เครื่องมือที่ทำลาย** (`move_to_recycle_bin`, `run_cleanup`) ทำเครื่องหมาย destructive ทำให้ Claude ต้องถามคุณก่อนรัน
- **การลบปกติคือการย้ายไป Recycle Bin** (กู้คืนได้) ทางเดียวที่ลบถาวรคือ `run_cleanup` action `empty-recycle-bin`
- **ตำแหน่งที่ถูกปฏิเสธ** (ตรวจในคอร์ ทั้งแอป CLI และ MCP ใช้ร่วมกัน):
  - ราก drive (เช่น `C:\`, `D:\`)
  - `C:\Windows`, `C:\Program Files`, `C:\Program Files (x86)`, `C:\ProgramData\Microsoft`
  - โฟลเดอร์ระดับบนสุดของไดรฟ์ระบบ (เช่น `C:\Users`)
  - `pagefile.sys`, `hiberfil.sys`, `swapfile.sys`, `$Recycle.Bin`, `System Volume Information`, `WinSxS`
- **คำสั่งผู้ดูแลระบบไม่ถูกรัน** `admin_commands` คืนข้อความให้คุณรันเองเท่านั้น
- **action ที่อนุญาตมีรายการเดียว** `run_cleanup` รับเฉพาะ action ในตารางด้านบน ไม่รับคำสั่งหรือ path จาก Claude
- **อ่านเฉพาะ metadata** การสแกนอ่านชื่อ ขนาด และวันที่แก้ไข ไม่อ่านเนื้อไฟล์ รายการโปรแกรมที่ติดตั้งอ่านจาก Registry แบบอ่านอย่างเดียว
- **ไม่มีการเชื่อมต่อเครือข่าย** DiskWhy, CLI และ MCP ไม่เรียกเซิร์ฟเวอร์ภายนอก
- **ข้อมูลที่ Claude ได้รับ** ผลลัพธ์ของเครื่องมือ (พาธ ชื่อไฟล์ ขนาด วันที่) จะถูกส่งให้ Claude ประมวลผลตามการทำงานปกติของ Claude Code / Claude Desktop

## 6. การแก้ปัญหา

| อาการ | วิธีแก้ |
|---|---|
| ใน Claude Code ไม่เห็น `diskwhy` | รัน `claude mcp list` ถ้าไม่มีรายการ ให้เพิ่มใหม่ตามหัวข้อ 1 ถ้ามีแต่ใช้ไม่ได้ ให้ลบแล้วเพิ่มใหม่ |
| Claude Desktop ไม่เห็น `diskwhy` | ตรวจว่าแก้ไฟล์ config ที่ตำแหน่งตรงกับวิธีติดตั้ง (ดูตารางในหัวข้อ 1) ตรวจว่า JSON ถูกต้องและ backslash เป็น `\\` แล้ว Restart ให้เต็ม (ปิดจาก system tray ด้วย) |
| path ของ `diskwhy-cli.exe` ใช้ไม่ได้หลังติดตั้งใหม่หรือย้ายโฟลเดอร์ | เชื่อมต่อใหม่จากแอป (กดยกเลิกแล้วเชื่อมต่อ) หรือแก้ path ใน config |
| การสแกนนาน | `scan` รอได้ประมาณ 55 วินาที ถ้ายังไม่เสร็จให้เรียก `scan_status` เพื่อดูความคืบหน้า ถ้าต้องการหยุดให้เรียก `cancel_scan` |
| ได้รับข้อความ "ตำแหน่งนี้เป็นไฟล์/โฟลเดอร์ของระบบ" | เป็นการป้องกันที่ตั้งใจ ไม่ใช่บั๊ก ให้ใช้เครื่องมือของ Windows (เช่น Storage Sense) แทน |
| Claude ไม่ยอมรันคำสั่งจาก `admin_commands` | เป็นการออกแบบ DiskWhy ให้คุณรันคำสั่งที่ต้องใช้สิทธิ์ผู้ดูแลระบบด้วยตัวเอง |
