import "@fontsource-variable/inter";
import "@fontsource/ibm-plex-sans-thai/400.css";
import "@fontsource/ibm-plex-sans-thai/500.css";
import "@fontsource/ibm-plex-sans-thai/600.css";
import "./styles.css";

import { ThemeProvider, Toaster, TooltipProvider, useTheme } from "@nova/ui";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";

function ThemedToaster() {
  const { resolvedTheme } = useTheme();
  return <Toaster theme={resolvedTheme} position="bottom-right" richColors closeButton />;
}

const root = document.getElementById("root");
if (!root) throw new Error("#root missing");

createRoot(root).render(
  <StrictMode>
    <ThemeProvider>
      <TooltipProvider>
        <App />
        <ThemedToaster />
      </TooltipProvider>
    </ThemeProvider>
  </StrictMode>,
);
