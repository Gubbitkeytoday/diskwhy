import { darkTokens, type TokenEntry, tokens } from "@nova/tokens";
import { Badge, toast } from "@nova/ui";
import type { Meta, StoryObj } from "@storybook/react-vite";

const meta = {
  title: "Foundations",
  parameters: { layout: "padded" },
  tags: ["!autodocs"],
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

const entries = (prefix: string) =>
  (Object.entries(tokens) as [string, TokenEntry][]).filter(([k]) => k.startsWith(prefix));

function copy(text: string) {
  navigator.clipboard?.writeText(text).then(
    () => toast.success(`Copied ${text}`),
    () => toast.error("Clipboard unavailable"),
  );
}

function Section({ title, hint, children }: { title: string; hint?: string; children: React.ReactNode }) {
  return (
    <section className="mb-12">
      <h2 className="text-xl font-semibold">{title}</h2>
      {hint && <p className="mt-1 mb-5 max-w-2xl text-sm text-muted-foreground">{hint}</p>}
      {children}
    </section>
  );
}

/* ── Colors ──────────────────────────────────────────────────────────────── */

const hues = ["neutral", "brand", "red", "amber", "green", "blue"] as const;

export const Colors: Story = {
  render: () => {
    const semantic = entries("color.");
    return (
      <div className="mx-auto max-w-6xl">
        <Section
          title="Semantic colors"
          hint="Use these in product code — they switch automatically between light and dark. Every foreground/background pair is gated at WCAG 2.2 AA in CI. Click a swatch to copy its Tailwind class."
        >
          <div className="grid grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] gap-3">
            {semantic.map(([path, t]) => {
              const name = path.replace("color.", "");
              return (
                <button
                  key={path}
                  type="button"
                  onClick={() => copy(`bg-${name}`)}
                  className="group cursor-pointer overflow-hidden rounded-lg border bg-card text-left transition-shadow hover:shadow-md focus-visible:ring-[3px] focus-visible:ring-ring/45 focus-visible:outline-none"
                >
                  <div className="h-14 border-b" style={{ background: `var(${t.name})` }} />
                  <div className="p-3">
                    <div className="font-mono text-xs font-medium">{name}</div>
                    <div className="mt-1 truncate text-xs text-muted-foreground">{t.description ?? t.name}</div>
                    <div className="mt-1 font-mono text-[0.65rem] text-muted-foreground">
                      dark → {String(darkTokens[path as keyof typeof darkTokens]?.value ?? "—")}
                    </div>
                  </div>
                </button>
              );
            })}
          </div>
        </Section>

        <Section title="Palette (primitives)" hint="Raw OKLCH scales. Perceptually uniform: the same step has the same lightness across hues. Do not use in components — map them through semantic tokens.">
          <div className="grid gap-7 pb-4">
            {hues.map((hue) => (
              <div key={hue} className="grid grid-cols-[5rem_repeat(11,1fr)] items-center gap-1.5">
                <span className="text-sm font-medium capitalize">{hue}</span>
                {entries(`palette.${hue}.`).map(([path, t]) => (
                  <button
                    key={path}
                    type="button"
                    title={`${path} · ${t.value}`}
                    onClick={() => copy(String(t.value))}
                    className="group relative h-12 cursor-pointer rounded-md border border-black/5 focus-visible:ring-[3px] focus-visible:ring-ring/45 focus-visible:outline-none"
                    style={{ background: `var(${t.name})` }}
                  >
                    <span className="absolute inset-x-0 -bottom-5 text-center font-mono text-[0.6rem] text-muted-foreground">
                      {path.split(".").at(-1)}
                    </span>
                    <span className="sr-only">{path}</span>
                  </button>
                ))}
              </div>
            ))}
          </div>
        </Section>
      </div>
    );
  },
};

/* ── Typography ──────────────────────────────────────────────────────────── */

const sizes = ["5xl", "4xl", "3xl", "2xl", "xl", "lg", "base", "sm", "xs"] as const;
// Literal class names so Tailwind's scanner can see them.
const sizeClass: Record<(typeof sizes)[number], string> = {
  "5xl": "text-5xl font-semibold tracking-tight",
  "4xl": "text-4xl font-semibold tracking-tight",
  "3xl": "text-3xl font-semibold tracking-tight",
  "2xl": "text-2xl font-semibold tracking-tight",
  xl: "text-xl",
  lg: "text-lg",
  base: "text-base",
  sm: "text-sm",
  xs: "text-xs",
};

export const Typography: Story = {
  render: () => (
    <div className="mx-auto max-w-5xl">
      <Section
        title="Type scale"
        hint="Inter for Latin, IBM Plex Sans Thai for Thai — mixed in one stack. Line heights are ~1.6× so Thai upper/lower vowels and tone marks never clip."
      >
        <div className="divide-y rounded-xl border">
          {sizes.map((s) => (
            <div key={s} className="grid grid-cols-[6rem_1fr] items-baseline gap-4 px-5 py-4">
              <div className="font-mono text-xs text-muted-foreground">
                text-{s}
                <div className="text-[0.65rem]">{String(tokens[`font.size.${s}` as keyof typeof tokens].value)}</div>
              </div>
              <p className={sizeClass[s]}>
                ออกแบบระบบให้สวยงาม ใช้ง่าย — Design that scales.
              </p>
            </div>
          ))}
        </div>
      </Section>
      <Section title="Weights">
        <div className="flex flex-wrap gap-8 text-2xl">
          <span className="font-normal">ปกติ Regular</span>
          <span className="font-medium">กลาง Medium</span>
          <span className="font-semibold">หนา Semibold</span>
          <span className="font-bold">หนามาก Bold</span>
        </div>
      </Section>
      <Section title="Thai stress test" hint="Stacked marks (ญี่ปุ่น, ปั้ม, ฎีกา, ฐู) at the smallest size inside tight UI.">
        <div className="flex flex-wrap items-center gap-3">
          <Badge>ฎีกา ปั้ม ญี่ปุ่น</Badge>
          <Badge variant="outline">ผู้ใช้งานใหม่ ๆ ฐู</Badge>
          <span className="text-xs">ข้อความเล็กที่สุด: กรุณาตรวจสอบความถูกต้องก่อนกดยืนยัน</span>
        </div>
      </Section>
      <Section title="Monospace">
        <code className="rounded-md bg-muted px-2 py-1 font-mono text-sm">pnpm add @nova/ui @nova/tokens</code>
      </Section>
    </div>
  ),
};

/* ── Radius & elevation ──────────────────────────────────────────────────── */

export const RadiusAndElevation: Story = {
  name: "Radius & Elevation",
  render: () => (
    <div className="mx-auto max-w-5xl">
      <Section title="Radius">
        <div className="flex flex-wrap gap-6">
          {entries("radius.").map(([path, t]) => {
            const k = path.split(".")[1];
            return (
              <div key={path} className="text-center">
                <div className="size-20 border-2 border-primary bg-accent" style={{ borderRadius: `var(${t.name})` }} />
                <div className="mt-2 font-mono text-xs">rounded-{k}</div>
                <div className="font-mono text-[0.65rem] text-muted-foreground">{String(t.value)}</div>
              </div>
            );
          })}
        </div>
      </Section>
      <Section title="Elevation" hint="Soft, tinted, two-layer shadows. In dark mode prefer borders + surface steps over shadows.">
        <div className="flex flex-wrap gap-8 p-4">
          {entries("shadow.").map(([path, t]) => (
            <div key={path} className="grid size-28 place-items-center rounded-xl bg-card text-sm" style={{ boxShadow: `var(${t.name})` }}>
              <span className="font-mono text-xs">shadow-{path.split(".")[1]}</span>
            </div>
          ))}
        </div>
      </Section>
    </div>
  ),
};

/* ── Motion ──────────────────────────────────────────────────────────────── */

export const Motion: Story = {
  render: () => (
    <div className="mx-auto max-w-4xl">
      <Section title="Duration & easing" hint="Hover the tracks. All motion collapses to ~0ms when the user enables 'reduce motion'.">
        <div className="grid gap-4">
          {entries("duration.")
            .filter(([p]) => !p.endsWith("instant"))
            .flatMap(([dp, d]) =>
              entries("easing.").slice(0, 2).map(([ep, e]) => (
                <div key={dp + ep} className="group grid grid-cols-[12rem_1fr] items-center gap-4">
                  <span className="font-mono text-xs">
                    {dp.split(".")[1]} · {ep.split(".")[1]}
                  </span>
                  <div className="h-10 rounded-lg bg-muted p-1 [container-type:inline-size]">
                    <div
                      className="size-8 rounded-md bg-primary group-hover:translate-x-[calc(100cqw-2rem)]"
                      style={{ transition: `translate var(${d.name}) var(${e.name})` }}
                    />
                  </div>
                </div>
              )),
            )}
        </div>
      </Section>
    </div>
  ),
};
