import { Label as LabelPrimitive } from "radix-ui";
import * as React from "react";
import { cn } from "../lib/cn";

export function Label({ className, ...props }: React.ComponentProps<typeof LabelPrimitive.Root>) {
  return (
    <LabelPrimitive.Root
      data-slot="label"
      className={cn(
        "flex select-none items-center gap-2 text-sm font-medium leading-none",
        "group-data-[disabled=true]:pointer-events-none group-data-[disabled=true]:opacity-50 peer-disabled:cursor-not-allowed peer-disabled:opacity-50",
        className,
      )}
      {...props}
    />
  );
}

interface FieldContextValue {
  id: string;
  descriptionId?: string;
  errorId?: string;
  invalid: boolean;
  required: boolean;
  disabled: boolean;
}

const FieldContext = React.createContext<FieldContextValue | null>(null);

/**
 * Wires a control inside <Field> to its label, description and error message.
 * Controls (Input, Textarea, SelectTrigger…) call this so consumers never hand-write aria attributes.
 */
export function useFieldControl<P extends { id?: string; "aria-describedby"?: string; "aria-invalid"?: unknown; required?: boolean; disabled?: boolean }>(
  props: P,
): P {
  const field = React.useContext(FieldContext);
  if (!field) return props;
  const describedBy = [props["aria-describedby"], field.descriptionId, field.errorId].filter(Boolean).join(" ");
  return {
    ...props,
    id: props.id ?? field.id,
    "aria-describedby": describedBy || undefined,
    "aria-invalid": props["aria-invalid"] ?? (field.invalid || undefined),
    required: props.required ?? (field.required || undefined),
    disabled: props.disabled ?? (field.disabled || undefined),
  };
}

export interface FieldProps extends Omit<React.ComponentProps<"div">, "children"> {
  label?: React.ReactNode;
  description?: React.ReactNode;
  /** Error message. When set, the control is marked `aria-invalid` and the message is announced. */
  error?: React.ReactNode;
  required?: boolean;
  disabled?: boolean;
  /** Override the generated control id. */
  htmlFor?: string;
  children: React.ReactNode;
}

export function Field({
  label,
  description,
  error,
  required = false,
  disabled = false,
  htmlFor,
  className,
  children,
  ...props
}: FieldProps) {
  const autoId = React.useId();
  const id = htmlFor ?? `nv-field-${autoId}`;
  const descriptionId = description ? `${id}-description` : undefined;
  const errorId = error ? `${id}-error` : undefined;
  const value = React.useMemo(
    () => ({ id, descriptionId, errorId, invalid: Boolean(error), required, disabled }),
    [id, descriptionId, errorId, error, required, disabled],
  );

  return (
    <FieldContext.Provider value={value}>
      <div data-slot="field" data-disabled={disabled} className={cn("group grid gap-2", className)} {...props}>
        {label && (
          <Label htmlFor={id}>
            {label}
            {required && (
              <span aria-hidden className="text-destructive">
                *
              </span>
            )}
          </Label>
        )}
        {children}
        {description && !error && (
          <p id={descriptionId} className="text-xs text-muted-foreground">
            {description}
          </p>
        )}
        {error && (
          <p id={errorId} role="alert" className="text-xs font-medium text-destructive">
            {error}
          </p>
        )}
      </div>
    </FieldContext.Provider>
  );
}
