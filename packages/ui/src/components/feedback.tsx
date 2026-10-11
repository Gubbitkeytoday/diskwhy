import { cva, type VariantProps } from "class-variance-authority";
import { CircleAlertIcon, CircleCheckIcon, InfoIcon, TriangleAlertIcon } from "lucide-react";
import type * as React from "react";
import { Toaster as Sonner, type ToasterProps, toast } from "sonner";
import { cn } from "../lib/cn";

/* ── Alert ───────────────────────────────────────────────────────────────── */

export const alertVariants = cva(
  "relative grid w-full grid-cols-[0_1fr] items-start gap-y-0.5 rounded-lg border px-4 py-3 text-sm has-[>svg]:grid-cols-[calc(var(--spacing)*4)_1fr] has-[>svg]:gap-x-3 [&>svg]:size-4 [&>svg]:translate-y-1 [&>svg]:text-current",
  {
    variants: {
      variant: {
        default: "bg-card text-card-foreground",
        info: "border-info/30 bg-info/8 text-foreground [&>svg]:text-info",
        success: "border-success/30 bg-success/8 text-foreground [&>svg]:text-success",
        warning: "border-warning/50 bg-warning/12 text-foreground [&>svg]:text-warning-foreground",
        destructive: "border-destructive/30 bg-destructive/8 text-foreground [&>svg]:text-destructive",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

const alertIcons = {
  default: InfoIcon,
  info: InfoIcon,
  success: CircleCheckIcon,
  warning: TriangleAlertIcon,
  destructive: CircleAlertIcon,
} as const;

export interface AlertProps extends React.ComponentProps<"div">, VariantProps<typeof alertVariants> {
  /** Show the variant's default icon. Pass your own `<svg>` as the first child to override. */
  icon?: boolean;
}

export function Alert({ className, variant, icon = true, children, ...props }: AlertProps) {
  const Icon = alertIcons[variant ?? "default"];
  const urgent = variant === "destructive" || variant === "warning";
  return (
    <div
      data-slot="alert"
      role={urgent ? "alert" : "status"}
      className={cn(alertVariants({ variant }), className)}
      {...props}
    >
      {icon && <Icon aria-hidden />}
      {children}
    </div>
  );
}

export function AlertTitle({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="alert-title" className={cn("col-start-2 font-medium leading-6", className)} {...props} />;
}

export function AlertDescription({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="alert-description"
      className={cn("col-start-2 grid justify-items-start gap-1 text-muted-foreground [&_p]:leading-relaxed", className)}
      {...props}
    />
  );
}

/* ── Toast (Sonner) ──────────────────────────────────────────────────────── */

/** Mount once near the app root. Then call `toast("Saved")`, `toast.success(...)`, `toast.promise(...)` anywhere. */
export function Toaster({ theme = "system", ...props }: ToasterProps) {
  return (
    <Sonner
      theme={theme}
      className="toaster group"
      style={
        {
          "--normal-bg": "var(--nv-color-popover)",
          "--normal-text": "var(--nv-color-popover-foreground)",
          "--normal-border": "var(--nv-color-border)",
          "--border-radius": "var(--nv-radius-lg)",
          fontFamily: "var(--nv-font-family-sans)",
        } as React.CSSProperties
      }
      {...props}
    />
  );
}

export { toast };

/* ── Skeleton ────────────────────────────────────────────────────────────── */

export function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div data-slot="skeleton" aria-hidden className={cn("animate-pulse rounded-md bg-muted", className)} {...props} />
  );
}
