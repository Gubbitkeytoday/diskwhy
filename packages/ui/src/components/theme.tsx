import * as React from "react";

export type ThemePreference = "light" | "dark" | "system";

interface ThemeContextValue {
  theme: ThemePreference;
  resolvedTheme: "light" | "dark";
  setTheme: (theme: ThemePreference) => void;
}

const ThemeContext = React.createContext<ThemeContextValue | null>(null);
const DEFAULT_KEY = "nova-theme";

const systemTheme = (): "light" | "dark" =>
  typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";

function readStored(key: string): ThemePreference | null {
  try {
    const v = localStorage.getItem(key);
    return v === "light" || v === "dark" || v === "system" ? v : null;
  } catch {
    return null;
  }
}

export interface ThemeProviderProps {
  children: React.ReactNode;
  defaultTheme?: ThemePreference;
  /** localStorage key used to persist the user's choice. */
  storageKey?: string;
}

/** Applies `.dark` + `data-theme` to <html>, follows the OS setting in "system" mode and persists the choice. */
export function ThemeProvider({ children, defaultTheme = "system", storageKey = DEFAULT_KEY }: ThemeProviderProps) {
  const [theme, setThemeState] = React.useState<ThemePreference>(() =>
    typeof window === "undefined" ? defaultTheme : (readStored(storageKey) ?? defaultTheme),
  );
  const [system, setSystem] = React.useState<"light" | "dark">(systemTheme);
  const resolvedTheme = theme === "system" ? system : theme;

  React.useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setSystem(mq.matches ? "dark" : "light");
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  React.useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", resolvedTheme === "dark");
    root.dataset.theme = resolvedTheme;
  }, [resolvedTheme]);

  const setTheme = React.useCallback(
    (next: ThemePreference) => {
      setThemeState(next);
      try {
        localStorage.setItem(storageKey, next);
      } catch {
        /* storage unavailable (private mode) — keep in-memory choice */
      }
    },
    [storageKey],
  );

  const value = React.useMemo(() => ({ theme, resolvedTheme, setTheme }), [theme, resolvedTheme, setTheme]);
  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): ThemeContextValue {
  const ctx = React.useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme must be used inside <ThemeProvider>");
  return ctx;
}

/**
 * Inline this in <head> (before CSS paints) to avoid a flash of the wrong theme on first load:
 *   <script dangerouslySetInnerHTML={{ __html: themeInitScript() }} />
 */
export function themeInitScript(storageKey = DEFAULT_KEY): string {
  // Static script; the key is JSON-encoded and `<` escaped so it can never close the <script> tag.
  const key = JSON.stringify(storageKey).replace(/</g, "\\u003c");
  return `(function(){try{var t=localStorage.getItem(${key})||"system";var d=t==="dark"||(t==="system"&&matchMedia("(prefers-color-scheme: dark)").matches);var r=document.documentElement;r.classList.toggle("dark",d);r.dataset.theme=d?"dark":"light";}catch(e){}})();`;
}
