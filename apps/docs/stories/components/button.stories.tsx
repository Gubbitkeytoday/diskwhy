import { Button, Kbd, Tooltip, TooltipContent, TooltipTrigger } from "@nova/ui";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ArrowRightIcon, PlusIcon, Trash2Icon } from "lucide-react";
import { expect, fn, userEvent, within } from "storybook/test";

const meta = {
  title: "Components/Button",
  component: Button,
  args: { children: "บันทึกการเปลี่ยนแปลง", onClick: fn() },
  argTypes: {
    variant: { control: "select", options: ["primary", "secondary", "outline", "ghost", "destructive", "link"] },
    size: { control: "select", options: ["sm", "md", "lg", "icon", "icon-sm"] },
  },
  parameters: {
    docs: {
      description: {
        component:
          "Triggers an action. Use **one primary** button per view. `loading` keeps width stable and announces `aria-busy`. Use `asChild` to render links (`<a>`, Next `<Link>`) with button styling.",
      },
    },
  },
} satisfies Meta<typeof Button>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Playground: Story = {
  play: async ({ canvasElement, args }) => {
    const button = within(canvasElement).getByRole("button");
    await userEvent.click(button);
    await expect(args.onClick).toHaveBeenCalledOnce();
  },
};

export const Variants: Story = {
  render: (args) => (
    <div className="flex flex-wrap items-center gap-3">
      <Button {...args} variant="primary">Primary</Button>
      <Button {...args} variant="secondary">Secondary</Button>
      <Button {...args} variant="outline">Outline</Button>
      <Button {...args} variant="ghost">Ghost</Button>
      <Button {...args} variant="destructive">Destructive</Button>
      <Button {...args} variant="link">Link</Button>
    </div>
  ),
};

export const Sizes: Story = {
  render: (args) => (
    <div className="flex flex-wrap items-center gap-3">
      <Button {...args} size="sm">Small</Button>
      <Button {...args} size="md">Medium</Button>
      <Button {...args} size="lg">Large</Button>
      <Button {...args} size="icon" aria-label="Add">
        <PlusIcon />
      </Button>
      <Button {...args} size="icon-sm" variant="outline" aria-label="Add">
        <PlusIcon />
      </Button>
    </div>
  ),
};

export const WithIcons: Story = {
  render: (args) => (
    <div className="flex flex-wrap gap-3">
      <Button {...args}>
        <PlusIcon /> สร้างโปรเจกต์
      </Button>
      <Button {...args} variant="outline">
        ถัดไป <ArrowRightIcon />
      </Button>
      <Button {...args} variant="destructive">
        <Trash2Icon /> ลบ
      </Button>
    </div>
  ),
};

export const Loading: Story = { args: { loading: true, children: "กำลังบันทึก" } };
export const Disabled: Story = { args: { disabled: true } };

export const AsLink: Story = {
  render: (args) => (
    <Button {...args} asChild variant="outline">
      <a href="#docs">Read the docs</a>
    </Button>
  ),
};

export const IconWithTooltip: Story = {
  name: "Icon button + tooltip",
  render: () => (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button size="icon" variant="ghost" aria-label="New file">
          <PlusIcon />
        </Button>
      </TooltipTrigger>
      <TooltipContent>
        New file <Kbd className="ml-1 border-background/20 bg-background/10 text-background">⌘N</Kbd>
      </TooltipContent>
    </Tooltip>
  ),
};
