import React from "react";
import ReactDOM from "react-dom/client";
import { applySavedTheme, ThemeProvider } from "@/components/theme-provider";
import App from "./App";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { applySavedTextSize } from "@/lib/text-size";
import "./index.css";

function start() {
  applySavedTheme();
  applySavedTextSize();
  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <ThemeProvider>
        <TooltipProvider delayDuration={300}>
          <App />
          {/* 規範 T6：放在下方中間，不擋右側的回答欄 */}
          <Toaster position="bottom-center" closeButton />
        </TooltipProvider>
      </ThemeProvider>
    </React.StrictMode>,
  );
}

// 開發時在一般瀏覽器裡預覽：換成假的後端（正式版不會包含）
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) import("./dev/mock").then(start);
else start();
