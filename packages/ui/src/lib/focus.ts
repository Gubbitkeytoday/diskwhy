/** Shared interaction classes so every control has an identical, visible focus treatment. */
export const focusRing =
  "outline-none focus-visible:ring-[3px] focus-visible:ring-ring/45 focus-visible:ring-offset-0";

export const invalidRing =
  "aria-invalid:border-destructive aria-invalid:ring-destructive/25 aria-invalid:focus-visible:ring-destructive/35";

export const disabledState = "disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50";
