import {
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuTrigger,
  Field,
  Input,
  Popover,
  PopoverContent,
  PopoverTrigger,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
  toast,
} from "@nova/ui";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { LogOutIcon, SettingsIcon, UserIcon } from "lucide-react";
import { useState } from "react";
import { expect, screen, userEvent, within } from "storybook/test";

const meta = {
  title: "Components/Overlays",
  component: DialogContent,
  parameters: {
    docs: {
      description: {
        component:
          "Radix-powered: focus trap, Escape to close, scroll lock, return focus to trigger, collision-aware positioning. Enter/exit animations respect `prefers-reduced-motion`.",
      },
    },
  },
} satisfies Meta<typeof DialogContent>;
export default meta;
type Story = StoryObj<typeof meta>;

export const DialogConfirm: Story = {
  name: "Dialog — destructive confirm",
  render: () => {
    const [open, setOpen] = useState(false);
    return (
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogTrigger asChild>
          <Button variant="destructive">ลบโปรเจกต์</Button>
        </DialogTrigger>
        <DialogContent closeLabel="ปิด">
          <DialogHeader>
            <DialogTitle>ลบโปรเจกต์ “Nova Website”?</DialogTitle>
            <DialogDescription>ข้อมูลทั้งหมด รวมถึงไฟล์และประวัติการแก้ไข จะถูกลบถาวรและไม่สามารถกู้คืนได้</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">ยกเลิก</Button>
            </DialogClose>
            <Button
              variant="destructive"
              onClick={() => {
                setOpen(false);
                toast.success("ลบโปรเจกต์แล้ว", { description: "Nova Website", action: { label: "เลิกทำ", onClick: () => {} } });
              }}
            >
              ลบถาวร
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  },
  play: async ({ canvasElement }) => {
    await userEvent.click(within(canvasElement).getByRole("button", { name: "ลบโปรเจกต์" }));
    const dialog = await screen.findByRole("dialog");
    await expect(dialog).toBeVisible();
    await userEvent.keyboard("{Escape}");
  },
};

export const DialogForm: Story = {
  name: "Dialog — form",
  render: () => (
    <Dialog>
      <DialogTrigger asChild>
        <Button variant="outline">แก้ไขโปรไฟล์</Button>
      </DialogTrigger>
      <DialogContent closeLabel="ปิด">
        <DialogHeader>
          <DialogTitle>แก้ไขโปรไฟล์</DialogTitle>
          <DialogDescription>การเปลี่ยนแปลงจะมีผลทันทีหลังบันทึก</DialogDescription>
        </DialogHeader>
        <form
          className="grid gap-4"
          onSubmit={(e) => {
            e.preventDefault();
            toast.success("บันทึกแล้ว");
          }}
        >
          <Field label="ชื่อที่แสดง">
            <Input defaultValue="สมชาย ใจดี" />
          </Field>
          <Field label="ชื่อผู้ใช้" description="nova.app/@username">
            <Input defaultValue="somchai" />
          </Field>
          <DialogFooter>
            <Button type="submit">บันทึก</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  ),
};

export const Menu: Story = {
  name: "Dropdown menu",
  render: () => {
    const [compact, setCompact] = useState(false);
    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline">บัญชีของฉัน</Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent className="w-56" align="start">
          <DropdownMenuLabel>somchai@nova.app</DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuGroup>
            <DropdownMenuItem>
              <UserIcon /> โปรไฟล์ <DropdownMenuShortcut>⇧⌘P</DropdownMenuShortcut>
            </DropdownMenuItem>
            <DropdownMenuItem>
              <SettingsIcon /> ตั้งค่า <DropdownMenuShortcut>⌘,</DropdownMenuShortcut>
            </DropdownMenuItem>
          </DropdownMenuGroup>
          <DropdownMenuSeparator />
          <DropdownMenuCheckboxItem checked={compact} onCheckedChange={setCompact}>
            โหมดกะทัดรัด
          </DropdownMenuCheckboxItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem variant="destructive">
            <LogOutIcon /> ออกจากระบบ
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    );
  },
};

export const PopoverExample: Story = {
  name: "Popover",
  render: () => (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline">ตั้งค่าขนาด</Button>
      </PopoverTrigger>
      <PopoverContent>
        <div className="grid gap-3">
          <p className="text-sm font-medium">ขนาด (px)</p>
          <div className="grid grid-cols-2 gap-2">
            <Field label="กว้าง">
              <Input defaultValue="1280" inputMode="numeric" />
            </Field>
            <Field label="สูง">
              <Input defaultValue="720" inputMode="numeric" />
            </Field>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  ),
};

export const TooltipExample: Story = {
  name: "Tooltip",
  render: () => (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="outline">วางเมาส์ที่นี่</Button>
      </TooltipTrigger>
      <TooltipContent>คำอธิบายสั้น ๆ ไม่เกินหนึ่งบรรทัด</TooltipContent>
    </Tooltip>
  ),
};

export const Toasts: Story = {
  render: () => (
    <div className="flex flex-wrap gap-2">
      <Button variant="outline" onClick={() => toast("บันทึกฉบับร่างแล้ว")}>Default</Button>
      <Button variant="outline" onClick={() => toast.success("ชำระเงินสำเร็จ", { description: "฿1,290.00" })}>Success</Button>
      <Button variant="outline" onClick={() => toast.error("อัปโหลดไม่สำเร็จ", { description: "ไฟล์ใหญ่เกิน 10MB" })}>Error</Button>
      <Button
        variant="outline"
        onClick={() =>
          toast.promise(new Promise((r) => setTimeout(r, 1500)), {
            loading: "กำลังเผยแพร่…",
            success: "เผยแพร่แล้ว",
            error: "ผิดพลาด",
          })
        }
      >
        Promise
      </Button>
    </div>
  ),
};
