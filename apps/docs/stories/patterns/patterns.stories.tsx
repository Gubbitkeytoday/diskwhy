import {
  Alert,
  AlertDescription,
  AlertTitle,
  Avatar,
  AvatarFallback,
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  Checkbox,
  Field,
  Input,
  Label,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Separator,
  Switch,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  toast,
} from "@nova/ui";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ArrowUpRightIcon, BellIcon, SparklesIcon } from "lucide-react";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

const meta = {
  title: "Patterns",
  parameters: { layout: "fullscreen" },
  tags: ["!autodocs"],
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

/* ── Sign in ─────────────────────────────────────────────────────────────── */

function SignInForm() {
  const [error, setError] = useState<string>();
  const [loading, setLoading] = useState(false);
  return (
    <div className="grid min-h-svh place-items-center bg-muted/50 p-6">
      <Card className="w-full max-w-sm">
        <CardHeader className="text-center">
          <div className="mx-auto mb-2 grid size-10 place-items-center rounded-xl bg-primary text-primary-foreground">
            <SparklesIcon className="size-5" />
          </div>
          <CardTitle className="text-xl">เข้าสู่ระบบ Nova</CardTitle>
          <CardDescription>ยินดีต้อนรับกลับ! กรอกข้อมูลเพื่อดำเนินการต่อ</CardDescription>
        </CardHeader>
        <CardContent>
          <form
            noValidate
            className="grid gap-4"
            onSubmit={(e) => {
              e.preventDefault();
              const email = new FormData(e.currentTarget).get("email")?.toString() ?? "";
              if (!/^\S+@\S+\.\S+$/.test(email)) {
                setError("กรุณากรอกอีเมลให้ถูกต้อง");
                return;
              }
              setError(undefined);
              setLoading(true);
              setTimeout(() => {
                setLoading(false);
                toast.success("เข้าสู่ระบบสำเร็จ");
              }, 1200);
            }}
          >
            <Field label="อีเมล" error={error} required>
              <Input name="email" type="email" autoComplete="email" placeholder="you@example.com" />
            </Field>
            <Field label="รหัสผ่าน" required>
              <Input name="password" type="password" autoComplete="current-password" />
            </Field>
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Checkbox id="remember" />
                <Label htmlFor="remember" className="font-normal">
                  จดจำฉัน
                </Label>
              </div>
              <Button variant="link" size="sm" asChild>
                <a href="#forgot">ลืมรหัสผ่าน?</a>
              </Button>
            </div>
            <Button type="submit" loading={loading} className="w-full">
              เข้าสู่ระบบ
            </Button>
          </form>
        </CardContent>
        <CardFooter className="justify-center text-sm text-muted-foreground">
          ยังไม่มีบัญชี?
          <Button variant="link" size="sm" asChild>
            <a href="#signup">สมัครสมาชิก</a>
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
}

export const SignIn: Story = {
  render: () => <SignInForm />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.type(canvas.getByLabelText(/อีเมล/), "somchai@");
    await userEvent.click(canvas.getByRole("button", { name: "เข้าสู่ระบบ" }));
    await expect(canvas.getByRole("alert")).toHaveTextContent("กรุณากรอกอีเมลให้ถูกต้อง");
  },
};

/* ── Dashboard ───────────────────────────────────────────────────────────── */

const stats = [
  { label: "รายได้", value: "฿482,190", delta: "+12.4%", up: true },
  { label: "ผู้ใช้งานใหม่", value: "2,341", delta: "+5.1%", up: true },
  { label: "อัตราการยกเลิก", value: "1.8%", delta: "-0.3%", up: true },
  { label: "ตั๋วซัพพอร์ตค้าง", value: "37", delta: "+9", up: false },
];

export const Dashboard: Story = {
  render: () => (
    <div className="min-h-svh bg-muted/40">
      <header className="flex h-14 items-center gap-3 border-b bg-background px-6">
        <div className="grid size-7 place-items-center rounded-lg bg-primary text-primary-foreground">
          <SparklesIcon className="size-4" />
        </div>
        <span className="font-semibold">Nova</span>
        <Separator orientation="vertical" className="mx-2 h-5" />
        <span className="text-sm text-muted-foreground">ภาพรวม</span>
        <div className="ml-auto flex items-center gap-2">
          <Button size="icon-sm" variant="ghost" aria-label="การแจ้งเตือน">
            <BellIcon />
          </Button>
          <Avatar className="size-8">
            <AvatarFallback className="text-xs">สช</AvatarFallback>
          </Avatar>
        </div>
      </header>
      <main className="mx-auto grid max-w-6xl gap-6 p-6">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight">สวัสดี สมชาย 👋</h1>
            <p className="text-sm text-muted-foreground">นี่คือสรุปผลการดำเนินงาน 30 วันล่าสุด</p>
          </div>
          <div className="flex gap-2">
            <Select defaultValue="30d">
              <SelectTrigger className="w-40" aria-label="ช่วงเวลา">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="7d">7 วันล่าสุด</SelectItem>
                <SelectItem value="30d">30 วันล่าสุด</SelectItem>
                <SelectItem value="90d">90 วันล่าสุด</SelectItem>
              </SelectContent>
            </Select>
            <Button onClick={() => toast.success("ส่งออกรายงานแล้ว")}>ส่งออก</Button>
          </div>
        </div>
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {stats.map((s) => (
            <Card key={s.label} className="gap-2 py-5">
              <CardHeader className="px-5">
                <CardDescription>{s.label}</CardDescription>
              </CardHeader>
              <CardContent className="flex items-end justify-between px-5">
                <span className="text-2xl font-semibold tabular-nums">{s.value}</span>
                <Badge variant={s.up ? "success" : "warning"}>{s.delta}</Badge>
              </CardContent>
            </Card>
          ))}
        </div>
        <div className="grid gap-4 lg:grid-cols-[2fr_1fr]">
          <Card>
            <CardHeader>
              <CardTitle>ยอดขายรายสัปดาห์</CardTitle>
              <CardDescription>เทียบกับช่วงเดียวกันของปีที่แล้ว</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="flex h-48 items-end gap-2" role="img" aria-label="กราฟแท่งยอดขาย 12 สัปดาห์ แนวโน้มเพิ่มขึ้น">
                {[38, 52, 45, 60, 58, 72, 66, 80, 74, 88, 84, 96].map((h, i) => (
                  <div
                    key={i}
                    className="flex-1 rounded-t-md bg-primary/85 transition-colors hover:bg-primary"
                    style={{ height: `${h}%` }}
                  />
                ))}
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>กิจกรรมล่าสุด</CardTitle>
            </CardHeader>
            <CardContent className="grid gap-4">
              {[
                ["ก", "กานต์ อัปเกรดเป็น Pro", "2 นาที"],
                ["ม", "มานี เชิญสมาชิก 3 คน", "1 ชม."],
                ["ป", "ปิติ ส่งออกรายงาน", "เมื่อวาน"],
              ].map(([i, t, d]) => (
                <div key={t} className="flex items-center gap-3 text-sm">
                  <Avatar className="size-8">
                    <AvatarFallback className="text-xs">{i}</AvatarFallback>
                  </Avatar>
                  <span className="flex-1">{t}</span>
                  <span className="text-xs text-muted-foreground">{d}</span>
                </div>
              ))}
            </CardContent>
            <CardFooter>
              <Button variant="ghost" size="sm" className="w-full">
                ดูทั้งหมด <ArrowUpRightIcon />
              </Button>
            </CardFooter>
          </Card>
        </div>
      </main>
    </div>
  ),
};

/* ── Settings ────────────────────────────────────────────────────────────── */

export const Settings: Story = {
  render: () => (
    <div className="mx-auto max-w-3xl p-6">
      <h1 className="text-2xl font-semibold tracking-tight">ตั้งค่า</h1>
      <p className="mb-6 text-sm text-muted-foreground">จัดการบัญชีและการตั้งค่าของคุณ</p>
      <Tabs defaultValue="account">
        <TabsList>
          <TabsTrigger value="account">บัญชี</TabsTrigger>
          <TabsTrigger value="notifications">การแจ้งเตือน</TabsTrigger>
          <TabsTrigger value="danger">โซนอันตราย</TabsTrigger>
        </TabsList>
        <TabsContent value="account">
          <Card>
            <CardHeader>
              <CardTitle>ข้อมูลส่วนตัว</CardTitle>
              <CardDescription>ข้อมูลนี้จะแสดงต่อสมาชิกในทีม</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-4 sm:grid-cols-2">
              <Field label="ชื่อ">
                <Input defaultValue="สมชาย" />
              </Field>
              <Field label="นามสกุล">
                <Input defaultValue="ใจดี" />
              </Field>
              <Field label="อีเมล" description="ใช้สำหรับเข้าสู่ระบบ" className="sm:col-span-2">
                <Input type="email" defaultValue="somchai@nova.app" />
              </Field>
            </CardContent>
            <CardFooter className="justify-end gap-2 border-t pt-6">
              <Button variant="outline">ยกเลิก</Button>
              <Button onClick={() => toast.success("บันทึกการเปลี่ยนแปลงแล้ว")}>บันทึก</Button>
            </CardFooter>
          </Card>
        </TabsContent>
        <TabsContent value="notifications">
          <Card>
            <CardContent className="grid gap-1">
              {[
                ["product", "อัปเดตผลิตภัณฑ์", "ฟีเจอร์ใหม่และการปรับปรุง", true],
                ["security", "แจ้งเตือนความปลอดภัย", "การเข้าสู่ระบบจากอุปกรณ์ใหม่", true],
                ["marketing", "ข่าวสารและโปรโมชัน", "ไม่เกินเดือนละ 2 ครั้ง", false],
              ].map(([id, title, desc, on]) => (
                <div key={id as string} className="flex items-center justify-between gap-4 rounded-lg p-3 hover:bg-muted/60">
                  <div>
                    <Label htmlFor={id as string}>{title}</Label>
                    <p className="mt-1 text-xs text-muted-foreground">{desc}</p>
                  </div>
                  <Switch id={id as string} defaultChecked={on as boolean} />
                </div>
              ))}
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="danger">
          <Alert variant="destructive">
            <AlertTitle>ลบบัญชี</AlertTitle>
            <AlertDescription>
              <p>ข้อมูลทั้งหมดจะถูกลบถาวรภายใน 30 วัน</p>
              <Button variant="destructive" size="sm" className="mt-2">
                ลบบัญชีของฉัน
              </Button>
            </AlertDescription>
          </Alert>
        </TabsContent>
      </Tabs>
    </div>
  ),
};
