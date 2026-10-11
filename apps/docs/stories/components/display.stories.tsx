import {
  Alert,
  AlertDescription,
  AlertTitle,
  Avatar,
  AvatarFallback,
  AvatarImage,
  Badge,
  Button,
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  Kbd,
  Separator,
  Skeleton,
  Spinner,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@nova/ui";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { MoreHorizontalIcon } from "lucide-react";

// Self-contained image (no network) so the docs work offline and inside strict CSPs.
const avatarSrc = `data:image/svg+xml,${encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 80"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#7c5cff"/><stop offset="1" stop-color="#22c3a6"/></linearGradient></defs><rect width="80" height="80" fill="url(#g)"/><circle cx="40" cy="31" r="14" fill="#fff" fill-opacity=".9"/><rect x="16" y="50" width="48" height="30" rx="15" fill="#fff" fill-opacity=".9"/></svg>',
)}`;

const meta = {
  title: "Components/Display",
  component: Card,
} satisfies Meta<typeof Card>;
export default meta;
type Story = StoryObj<typeof meta>;

export const CardExample: Story = {
  name: "Card",
  render: () => (
    <Card className="w-96">
      <CardHeader>
        <CardTitle>รายได้เดือนนี้</CardTitle>
        <CardDescription>1 – 31 ตุลาคม 2569</CardDescription>
        <CardAction>
          <Button size="icon-sm" variant="ghost" aria-label="ตัวเลือกเพิ่มเติม">
            <MoreHorizontalIcon />
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent>
        <p className="text-4xl font-semibold tracking-tight tabular-nums">฿482,190</p>
        <p className="mt-1 flex items-center gap-2 text-sm text-muted-foreground">
          <Badge variant="success">+12.4%</Badge> เทียบกับเดือนก่อน
        </p>
      </CardContent>
      <CardFooter className="justify-end">
        <Button variant="outline" size="sm">ดูรายงาน</Button>
      </CardFooter>
    </Card>
  ),
};

export const Badges: Story = {
  render: () => (
    <div className="flex flex-wrap gap-2">
      <Badge>ใหม่</Badge>
      <Badge variant="secondary">ร่าง</Badge>
      <Badge variant="outline">v0.1.0</Badge>
      <Badge variant="success">ใช้งานอยู่</Badge>
      <Badge variant="warning">รอตรวจสอบ</Badge>
      <Badge variant="info">เบต้า</Badge>
      <Badge variant="destructive">ล้มเหลว</Badge>
    </div>
  ),
};

export const Alerts: Story = {
  render: () => (
    <div className="grid w-[32rem] gap-3">
      <Alert variant="info">
        <AlertTitle>มีเวอร์ชันใหม่</AlertTitle>
        <AlertDescription>Nova 0.2 เพิ่มคอมโพเนนต์ DataTable และ Command palette</AlertDescription>
      </Alert>
      <Alert variant="success">
        <AlertTitle>เชื่อมต่อสำเร็จ</AlertTitle>
        <AlertDescription>ซิงก์ Figma variables ล่าสุดเมื่อ 2 นาทีที่แล้ว</AlertDescription>
      </Alert>
      <Alert variant="warning">
        <AlertTitle>โควตาใกล้เต็ม</AlertTitle>
        <AlertDescription>ใช้พื้นที่ไปแล้ว 92% ของแพ็กเกจปัจจุบัน</AlertDescription>
      </Alert>
      <Alert variant="destructive">
        <AlertTitle>ชำระเงินไม่สำเร็จ</AlertTitle>
        <AlertDescription>บัตรถูกปฏิเสธ กรุณาลองบัตรใบอื่น</AlertDescription>
      </Alert>
    </div>
  ),
};

export const TabsExample: Story = {
  name: "Tabs",
  render: () => (
    <Tabs defaultValue="overview" className="w-[28rem]">
      <TabsList>
        <TabsTrigger value="overview">ภาพรวม</TabsTrigger>
        <TabsTrigger value="analytics">วิเคราะห์</TabsTrigger>
        <TabsTrigger value="reports">รายงาน</TabsTrigger>
      </TabsList>
      <TabsContent value="overview" className="rounded-lg border p-4 text-sm">ภาพรวมของโปรเจกต์</TabsContent>
      <TabsContent value="analytics" className="rounded-lg border p-4 text-sm">กราฟและสถิติการใช้งาน</TabsContent>
      <TabsContent value="reports" className="rounded-lg border p-4 text-sm">ดาวน์โหลดรายงานประจำเดือน</TabsContent>
    </Tabs>
  ),
};

export const AvatarExample: Story = {
  name: "Avatar",
  render: () => (
    <div className="flex items-center gap-3">
      <Avatar>
        <AvatarImage src={avatarSrc} alt="" />
        <AvatarFallback>สช</AvatarFallback>
      </Avatar>
      <Avatar>
        <AvatarFallback>NV</AvatarFallback>
      </Avatar>
      <div className="flex -space-x-2 *:ring-2 *:ring-background">
        {["ก", "ข", "ค", "+4"].map((t) => (
          <Avatar key={t} className="size-8">
            <AvatarFallback className="text-xs">{t}</AvatarFallback>
          </Avatar>
        ))}
      </div>
    </div>
  ),
};

export const LoadingStates: Story = {
  name: "Skeleton & Spinner",
  render: () => (
    <div className="grid w-80 gap-6">
      <div className="flex items-center gap-3">
        <Skeleton className="size-10 rounded-full" />
        <div className="grid flex-1 gap-2">
          <Skeleton className="h-4 w-3/4" />
          <Skeleton className="h-3 w-1/2" />
        </div>
      </div>
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Spinner label="กำลังโหลด" /> กำลังโหลดข้อมูล…
      </div>
    </div>
  ),
};

export const SeparatorAndKbd: Story = {
  name: "Separator & Kbd",
  render: () => (
    <div className="w-80 text-sm">
      <p className="font-medium">Nova Design System</p>
      <p className="text-muted-foreground">ระบบออกแบบสำหรับทีมโปรดักต์</p>
      <Separator className="my-4" />
      <div className="flex h-5 items-center gap-4">
        <span>เอกสาร</span>
        <Separator orientation="vertical" />
        <span>คอมโพเนนต์</span>
        <Separator orientation="vertical" />
        <span className="flex items-center gap-1">
          ค้นหา <Kbd>⌘</Kbd>
          <Kbd>K</Kbd>
        </span>
      </div>
    </div>
  ),
};
