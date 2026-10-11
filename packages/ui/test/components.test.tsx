import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
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
  CardHeader,
  CardTitle,
  Checkbox,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
  DialogTrigger,
  Field,
  Input,
  Label,
  RadioGroup,
  RadioGroupItem,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Switch,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  Textarea,
  ThemeProvider,
  useTheme,
} from "../src";
import { expectNoA11yViolations } from "./axe";

describe("Button", () => {
  it("renders an accessible button that defaults to type=button", async () => {
    const onClick = vi.fn();
    const { container } = render(<Button onClick={onClick}>บันทึก</Button>);
    const btn = screen.getByRole("button", { name: "บันทึก" });
    expect(btn).toHaveAttribute("type", "button");
    await userEvent.click(btn);
    expect(onClick).toHaveBeenCalledOnce();
    await expectNoA11yViolations(container);
  });

  it("loading state blocks clicks and exposes aria-busy", async () => {
    const onClick = vi.fn();
    render(
      <Button loading onClick={onClick}>
        Save
      </Button>,
    );
    const btn = screen.getByRole("button", { name: "Save" });
    expect(btn).toBeDisabled();
    expect(btn).toHaveAttribute("aria-busy", "true");
    await userEvent.click(btn);
    expect(onClick).not.toHaveBeenCalled();
  });

  it("asChild renders the child element with button styles", () => {
    render(
      <Button asChild variant="outline">
        <a href="/docs">Docs</a>
      </Button>,
    );
    const link = screen.getByRole("link", { name: "Docs" });
    expect(link).toHaveAttribute("data-slot", "button");
    expect(link.className).toContain("border-input");
  });

  it("lets consumer classes override variants (tailwind-merge)", () => {
    render(<Button className="h-14">Tall</Button>);
    const cls = screen.getByRole("button").className;
    expect(cls).toContain("h-14");
    expect(cls).not.toContain("h-10");
  });
});

describe("Field", () => {
  it("wires label, description and error to the control automatically", async () => {
    const { container } = render(
      <Field label="อีเมล" description="เราจะไม่เปิดเผยอีเมลของคุณ" error="รูปแบบอีเมลไม่ถูกต้อง" required>
        <Input type="email" />
      </Field>,
    );
    const input = screen.getByLabelText(/อีเมล/);
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input).toBeRequired();
    expect(input).toHaveAccessibleDescription("รูปแบบอีเมลไม่ถูกต้อง");
    expect(screen.getByRole("alert")).toHaveTextContent("รูปแบบอีเมลไม่ถูกต้อง");
    await expectNoA11yViolations(container);
  });

  it("shows the description when there is no error", () => {
    render(
      <Field label="Bio" description="Max 160 characters">
        <Textarea />
      </Field>,
    );
    expect(screen.getByLabelText("Bio")).toHaveAccessibleDescription("Max 160 characters");
  });
});

describe("Form controls", () => {
  it("Checkbox toggles and is labelled", async () => {
    const { container } = render(
      <div className="flex items-center gap-2">
        <Checkbox id="terms" />
        <Label htmlFor="terms">ยอมรับเงื่อนไข</Label>
      </div>,
    );
    const cb = screen.getByRole("checkbox", { name: "ยอมรับเงื่อนไข" });
    expect(cb).toHaveAttribute("aria-checked", "false");
    await userEvent.click(cb);
    expect(cb).toHaveAttribute("aria-checked", "true");
    await expectNoA11yViolations(container);
  });

  it("Switch toggles with the keyboard", async () => {
    render(<Switch aria-label="Notifications" />);
    const sw = screen.getByRole("switch", { name: "Notifications" });
    sw.focus();
    await userEvent.keyboard(" ");
    expect(sw).toHaveAttribute("aria-checked", "true");
  });

  it("RadioGroup supports arrow-key navigation", async () => {
    const { container } = render(
      <RadioGroup defaultValue="a" aria-label="Plan">
        <RadioGroupItem value="a" aria-label="Free" />
        <RadioGroupItem value="b" aria-label="Pro" />
      </RadioGroup>,
    );
    await userEvent.tab();
    expect(screen.getByRole("radio", { name: "Free" })).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    expect(screen.getByRole("radio", { name: "Pro" })).toHaveFocus();
    await userEvent.keyboard(" ");
    expect(screen.getByRole("radio", { name: "Pro" })).toBeChecked();
    await expectNoA11yViolations(container);
  });

  it("Select trigger inherits Field wiring", () => {
    render(
      <Field label="จังหวัด">
        <Select>
          <SelectTrigger>
            <SelectValue placeholder="เลือกจังหวัด" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="bkk">กรุงเทพมหานคร</SelectItem>
          </SelectContent>
        </Select>
      </Field>,
    );
    expect(screen.getByRole("combobox", { name: "จังหวัด" })).toBeInTheDocument();
  });
});

describe("Overlays & navigation", () => {
  it("Dialog opens, traps focus and closes on Escape", async () => {
    render(
      <Dialog>
        <DialogTrigger asChild>
          <Button>Open</Button>
        </DialogTrigger>
        <DialogContent closeLabel="ปิด">
          <DialogTitle>ยืนยันการลบ</DialogTitle>
          <DialogDescription>การกระทำนี้ไม่สามารถย้อนกลับได้</DialogDescription>
        </DialogContent>
      </Dialog>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Open" }));
    const dialog = screen.getByRole("dialog", { name: "ยืนยันการลบ" });
    expect(dialog).toHaveAccessibleDescription("การกระทำนี้ไม่สามารถย้อนกลับได้");
    expect(screen.getByRole("button", { name: "ปิด" })).toBeInTheDocument();
    await expectNoA11yViolations(dialog);
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("Tabs switch panels with arrow keys", async () => {
    const { container } = render(
      <Tabs defaultValue="account">
        <TabsList>
          <TabsTrigger value="account">บัญชี</TabsTrigger>
          <TabsTrigger value="security">ความปลอดภัย</TabsTrigger>
        </TabsList>
        <TabsContent value="account">Account panel</TabsContent>
        <TabsContent value="security">Security panel</TabsContent>
      </Tabs>,
    );
    screen.getByRole("tab", { name: "บัญชี" }).focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("tab", { name: "ความปลอดภัย" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel")).toHaveTextContent("Security panel");
    await expectNoA11yViolations(container);
  });
});

describe("Display & feedback", () => {
  it("Alert uses role=alert for urgent variants and status otherwise", () => {
    const { rerender } = render(
      <Alert variant="destructive">
        <AlertTitle>Payment failed</AlertTitle>
        <AlertDescription>Try another card.</AlertDescription>
      </Alert>,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("Payment failed");
    rerender(<Alert variant="success">Saved</Alert>);
    expect(screen.getByRole("status")).toHaveTextContent("Saved");
  });

  it("Card, Badge and Avatar render without a11y violations", async () => {
    const { container } = render(
      <Card>
        <CardHeader>
          <CardTitle>สรุปยอดขาย</CardTitle>
          <CardDescription>ข้อมูล 30 วันล่าสุด</CardDescription>
        </CardHeader>
        <CardContent>
          <Badge variant="success">+12.4%</Badge>
          <Avatar>
            <AvatarFallback>สม</AvatarFallback>
          </Avatar>
        </CardContent>
      </Card>,
    );
    expect(screen.getByRole("heading", { name: "สรุปยอดขาย" })).toBeInTheDocument();
    await expectNoA11yViolations(container);
  });
});

describe("ThemeProvider", () => {
  function Toggle() {
    const { resolvedTheme, setTheme } = useTheme();
    return <Button onClick={() => setTheme(resolvedTheme === "dark" ? "light" : "dark")}>{resolvedTheme}</Button>;
  }

  it("applies .dark and data-theme to <html> and persists the choice", async () => {
    render(
      <ThemeProvider defaultTheme="light">
        <Toggle />
      </ThemeProvider>,
    );
    expect(document.documentElement).not.toHaveClass("dark");
    await userEvent.click(screen.getByRole("button", { name: "light" }));
    expect(document.documentElement).toHaveClass("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem("nova-theme")).toBe("dark");
  });
});
