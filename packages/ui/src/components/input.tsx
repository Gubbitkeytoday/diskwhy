import type * as React from "react";
import { cn } from "../lib/cn";
import { focusRing, invalidRing } from "../lib/focus";
import { useFieldControl } from "./field";

const controlBase = [
  "w-full min-w-0 rounded-md border border-input bg-background text-sm shadow-xs",
  "transition-[color,border-color,box-shadow] duration-150 ease-standard",
  "placeholder:text-muted-foreground focus-visible:border-ring",
  "disabled:cursor-not-allowed disabled:opacity-50",
  focusRing,
  invalidRing,
];

export function Input({ className, type = "text", ...props }: React.ComponentProps<"input">) {
  const fieldProps = useFieldControl(props);
  return (
    <input
      data-slot="input"
      type={type}
      className={cn(
        controlBase,
        "flex h-10 px-3 py-2",
        "file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground",
        className,
      )}
      {...fieldProps}
    />
  );
}

export function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  const fieldProps = useFieldControl(props);
  return (
    <textarea
      data-slot="textarea"
      className={cn(controlBase, "field-sizing-content flex min-h-20 px-3 py-2", className)}
      {...fieldProps}
    />
  );
}
