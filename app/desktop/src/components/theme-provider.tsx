import { createContext, useCallback, useContext, useEffect, useState } from "react";
import { MonitorIcon, MoonIcon, SunIcon } from "lucide-react";

export type Theme = "light" | "dark" | "system";

const KEY = "theme";
const dark = () => window.matchMedia("(prefers-color-scheme: dark)");

/** 使用者選的主題記在這台電腦上；讀不到（例如隱私設定）就跟隨系統 */
function saved(): Theme {
  try {
    const t = localStorage.getItem(KEY);
    return t === "light" || t === "dark" ? t : "system";
  } catch {
    return "system";
  }
}

function resolve(theme: Theme): "light" | "dark" {
  return theme === "system" ? (dark().matches ? "dark" : "light") : theme;
}

function apply(theme: Theme) {
  const resolved = resolve(theme);
  const root = document.documentElement;
  root.classList.toggle("dark", resolved === "dark");
  root.style.colorScheme = resolved;
}

/** 在畫面出來之前先套用主題，避免一開始閃一下白色 */
export function applySavedTheme() {
  apply(saved());
}

const ThemeContext = createContext<{
  theme: Theme;
  resolvedTheme: "light" | "dark";
  setTheme: (theme: Theme) => void;
}>({ theme: "system", resolvedTheme: "light", setTheme: () => {} });

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(saved);
  const [resolvedTheme, setResolved] = useState(() => resolve(theme));

  useEffect(() => {
    apply(theme);
    setResolved(resolve(theme));
    if (theme !== "system") return;
    // 跟隨系統時，Windows 切換淺色或深色就跟著換
    const media = dark();
    const onChange = () => {
      apply("system");
      setResolved(resolve("system"));
    };
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [theme]);

  const setTheme = useCallback((next: Theme) => {
    setThemeState(next);
    try {
      localStorage.setItem(KEY, next);
    } catch {
      // 存不起來就只在這次有效
    }
  }, []);

  return <ThemeContext.Provider value={{ theme, resolvedTheme, setTheme }}>{children}</ThemeContext.Provider>;
}

export const useTheme = () => useContext(ThemeContext);

/** 主題選項：頁首的選單和設定頁共用 */
export const THEMES: { value: Theme; label: string; icon: React.ReactNode }[] = [
  { value: "light", label: "淺色", icon: <SunIcon /> },
  { value: "dark", label: "深色", icon: <MoonIcon /> },
  { value: "system", label: "跟隨系統", icon: <MonitorIcon /> },
];
