import {
  Checkbox,
  Field,
  Input,
  Label,
  RadioGroup,
  RadioGroupItem,
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
  Switch,
  Textarea,
} from "@nova/ui";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";

const meta = {
  title: "Components/Forms",
  component: Field,
  parameters: {
    docs: {
      description: {
        component:
          "`<Field>` wires label ↔ control ↔ description ↔ error automatically (`id`, `aria-describedby`, `aria-invalid`, `required`). Every control (Input, Textarea, SelectTrigger, Checkbox, Switch) reads it — consumers never hand-write ARIA.",
      },
    },
  },
  args: { label: "อีเมล", description: "เราจะไม่เปิดเผยอีเมลของคุณกับบุคคลอื่น", children: null },
  decorators: [(Story) => <div className="w-80">{Story()}</div>],
} satisfies Meta<typeof Field>;
export default meta;
type Story = StoryObj<typeof meta>;

export const TextInput: Story = {
  render: (args) => (
    <Field {...args}>
      <Input type="email" placeholder="you@example.com" />
    </Field>
  ),
};

export const WithError: Story = {
  args: { error: "รูปแบบอีเมลไม่ถูกต้อง", required: true },
  render: (args) => (
    <Field {...args}>
      <Input type="email" defaultValue="somchai@" />
    </Field>
  ),
  play: async ({ canvasElement }) => {
    const input = within(canvasElement).getByLabelText(/อีเมล/);
    await expect(input).toHaveAttribute("aria-invalid", "true");
    await expect(input).toHaveAccessibleDescription("รูปแบบอีเมลไม่ถูกต้อง");
  },
};

export const Disabled: Story = {
  args: { disabled: true, description: undefined },
  render: (args) => (
    <Field {...args}>
      <Input defaultValue="locked@example.com" />
    </Field>
  ),
};

export const TextareaField: Story = {
  name: "Textarea (auto-grow)",
  args: { label: "เกี่ยวกับคุณ", description: "สูงสุด 160 ตัวอักษร" },
  render: (args) => (
    <Field {...args}>
      <Textarea placeholder="เล่าเรื่องสั้น ๆ เกี่ยวกับตัวคุณ…" maxLength={160} />
    </Field>
  ),
};

export const SelectField: Story = {
  name: "Select",
  args: { label: "จังหวัด", description: undefined },
  render: (args) => (
    <Field {...args}>
      <Select>
        <SelectTrigger>
          <SelectValue placeholder="เลือกจังหวัด" />
        </SelectTrigger>
        <SelectContent>
          <SelectGroup>
            <SelectLabel>ภาคกลาง</SelectLabel>
            <SelectItem value="bkk">กรุงเทพมหานคร</SelectItem>
            <SelectItem value="nonthaburi">นนทบุรี</SelectItem>
          </SelectGroup>
          <SelectSeparator />
          <SelectGroup>
            <SelectLabel>ภาคเหนือ</SelectLabel>
            <SelectItem value="cnx">เชียงใหม่</SelectItem>
            <SelectItem value="cei">เชียงราย</SelectItem>
          </SelectGroup>
        </SelectContent>
      </Select>
    </Field>
  ),
};

export const CheckboxAndSwitch: Story = {
  name: "Checkbox & Switch",
  render: () => (
    <div className="grid gap-5">
      <div className="flex items-center gap-2.5">
        <Checkbox id="terms" defaultChecked />
        <Label htmlFor="terms">ยอมรับข้อกำหนดและเงื่อนไข</Label>
      </div>
      <div className="flex items-center gap-2.5">
        <Checkbox id="partial" checked="indeterminate" />
        <Label htmlFor="partial">เลือกบางรายการ (indeterminate)</Label>
      </div>
      <div className="flex items-center justify-between gap-4 rounded-lg border p-3">
        <div>
          <Label htmlFor="notif">การแจ้งเตือนทางอีเมล</Label>
          <p className="mt-1 text-xs text-muted-foreground">รับสรุปรายสัปดาห์</p>
        </div>
        <Switch id="notif" />
      </div>
    </div>
  ),
  play: async ({ canvasElement }) => {
    const sw = within(canvasElement).getByRole("switch");
    await userEvent.click(sw);
    await expect(sw).toHaveAttribute("aria-checked", "true");
  },
};

export const Radio: Story = {
  render: () => (
    <RadioGroup defaultValue="pro" aria-label="แพ็กเกจ">
      {(
        [
          ["free", "Free", "สำหรับทดลองใช้"],
          ["pro", "Pro", "สำหรับทีมขนาดเล็ก"],
          ["enterprise", "Enterprise", "SSO, SLA และ audit log"],
        ] as const
      ).map(([value, title, desc]) => (
        <Label
          key={value}
          htmlFor={value}
          className="flex cursor-pointer items-start gap-3 rounded-lg border p-3 has-[[data-state=checked]]:border-primary has-[[data-state=checked]]:bg-accent"
        >
          <RadioGroupItem value={value} id={value} className="mt-0.5" />
          <span className="grid gap-1">
            <span>{title}</span>
            <span className="text-xs font-normal text-muted-foreground">{desc}</span>
          </span>
        </Label>
      ))}
    </RadioGroup>
  ),
};
