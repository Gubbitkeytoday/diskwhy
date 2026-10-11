import type * as React from "react";
import { cn } from "../lib/cn";

export interface SpinnerProps extends React.ComponentProps<"svg"> {
  /** Accessible label. Pass `null` when the spinner is decorative (e.g. inside a busy button). */
  label?: string | null;
}

export function Spinner({ className, label = "Loading", ...props }: SpinnerProps) {
  return (
    <svg
      data-slot="spinner"
      viewBox="0 0 24 24"
      fill="none"
      role={label ? "status" : undefined}
      aria-label={label ?? undefined}
      aria-hidden={label ? undefined : true}
      className={cn("size-4 animate-spin", className)}
      {...props}
    >
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeOpacity="0.25" strokeWidth="3" />
      <path d="M21 12a9 9 0 0 0-9-9" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
    </svg>
  );
}
