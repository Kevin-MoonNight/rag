import { useCallback, useEffect, useState } from "react";
import { ChevronDownIcon, MoonIcon, RotateCcwIcon, ServerCrashIcon, SunIcon, TriangleAlertIcon } from "lucide-react";
import { AppSidebar, View, VIEW_TITLES } from "@/components/app-sidebar";
import { Theme, THEMES, useTheme } from "@/components/theme-provider";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Separator } from "@/components/ui/separator";
import { SidebarInset, SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useSources, useStatus } from "@/hooks/use-app-state";
import { cn } from "@/lib/utils";
import { IndexView } from "@/views/index-view";
import { SearchView } from "@/views/search-view";
import { SettingsView } from "@/views/settings-view";
import { SourcesView } from "@/views/sources-view";

/** 側欄展開或收起記在這台電腦上；讀不到（例如隱私設定）就用展開 */
function savedSidebar(): boolean {
  try {
    return localStorage.getItem("sidebar") !== "closed";
  } catch {
    return true;
  }
}


function ThemeMenu() {
  const { theme, setTheme, resolvedTheme } = useTheme();
  return (
    <DropdownMenu>
      <Tooltip>
        <TooltipTrigger asChild>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon" aria-label="主題">
              {resolvedTheme === "dark" ? <MoonIcon /> : <SunIcon />}
            </Button>
          </DropdownMenuTrigger>
        </TooltipTrigger>
        <TooltipContent>主題</TooltipContent>
      </Tooltip>
      <DropdownMenuContent align="end" className="w-40">
        <DropdownMenuRadioGroup value={theme} onValueChange={(v) => setTheme(v as Theme)}>
          {THEMES.map((t) => (
            <DropdownMenuRadioItem key={t.value} value={t.value}>
              {t.icon}
              {t.label}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/** 規範 T6：整個後端讀不到時的全頁錯誤；詳細錯誤收在 Collapsible 裡 */
function BackendError({ error, onRetry }: { error: string; onRetry: () => void }) {
  return (
    <Empty className="h-full">
      <EmptyHeader>
        <EmptyMedia variant="icon" className="text-destructive">
          <ServerCrashIcon />
        </EmptyMedia>
        <EmptyTitle>讀不到索引資料</EmptyTitle>
        <EmptyDescription>程式的背景服務沒有回應。請按「重試」；一直不行的話，關掉程式再打開一次。</EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button onClick={onRetry}>
          <RotateCcwIcon data-icon="inline-start" />
          重試
        </Button>
        <Collapsible className="flex w-full flex-col items-center gap-2">
          <CollapsibleTrigger asChild>
            <Button variant="ghost" size="sm" className="group/details">
              詳細錯誤
              <ChevronDownIcon data-icon="inline-end" className="transition-transform group-data-[state=open]/details:rotate-180" />
            </Button>
          </CollapsibleTrigger>
          <CollapsibleContent className="w-full">
            <pre className="rounded-lg bg-muted p-3 text-left text-xs whitespace-pre-wrap text-secondary-foreground select-text">
              {error}
            </pre>
          </CollapsibleContent>
        </Collapsible>
      </EmptyContent>
    </Empty>
  );
}

export default function App() {
  const [view, setView] = useState<View>("search");
  const [sidebarOpen, setSidebarOpen] = useState(savedSidebar);
  const [focusSignal, setFocusSignal] = useState(0);
  const { status, error: statusError, refresh } = useStatus();
  const running = !!status?.progress.running;
  const { sources, error: sourcesError, reload: reloadSources } = useSources(running);

  const changeSidebar = (open: boolean) => {
    setSidebarOpen(open);
    try {
      localStorage.setItem("sidebar", open ? "open" : "closed");
    } catch {
      // 存不起來就只在這次有效
    }
  };

  const goSearch = useCallback(() => {
    setView("search");
    setFocusSignal((n) => n + 1);
  }, []);

  // Ctrl+K 或 /：回到搜尋框
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLElement && e.target.closest("input, textarea, [contenteditable=true]");
      if ((e.key === "k" && (e.ctrlKey || e.metaKey)) || (e.key === "/" && !typing && !e.ctrlKey)) {
        e.preventDefault();
        goSearch();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [goSearch]);

  const onSourcesChanged = useCallback(() => {
    reloadSources();
    refresh();
  }, [reloadSources, refresh]);

  const panes: Record<View, React.ReactNode> = {
    search: <SearchView status={status} focusSignal={focusSignal} onAddSources={() => setView("sources")} />,
    sources: <SourcesView sources={sources} error={sourcesError} running={running} onChanged={onSourcesChanged} />,
    index: <IndexView status={status} sourceCount={sources?.length ?? null} refresh={refresh} onAddSources={() => setView("sources")} />,
    settings: <SettingsView status={status} />,
  };

  return (
    <SidebarProvider open={sidebarOpen} onOpenChange={changeSidebar} className="h-svh min-h-0">
      <AppSidebar
        view={view}
        onNavigate={(v) => (v === "search" ? goSearch() : setView(v))}
        status={status}
        sourceCount={sources?.length ?? null}
        offlineCount={sources?.filter((s) => s.offline_since).length ?? 0}
      />
      {/* 規範 T1 捲動骨架：頁首固定，下面每一頁自己捲動 */}
      <SidebarInset className="min-h-0 min-w-0 overflow-hidden">
        <header className="flex h-12 shrink-0 items-center gap-2 border-b">
          <div className="flex w-full items-center gap-1 px-4 lg:gap-2 lg:px-6">
            <SidebarTrigger className="-ml-1" />
            <Separator orientation="vertical" className="mx-2 data-[orientation=vertical]:h-4" />
            <h1 className="text-base font-medium">{VIEW_TITLES[view]}</h1>
            <div className="ml-auto flex items-center gap-2">
              <ThemeMenu />
            </div>
          </div>
        </header>
        {statusError && status && (
          <Alert variant="destructive" className="rounded-none border-x-0 border-t-0">
            <TriangleAlertIcon />
            <AlertDescription>暫時讀不到索引狀態，程式會自動重試。</AlertDescription>
          </Alert>
        )}
        <div className="@container/main relative min-h-0 flex-1">
          {statusError && !status ? (
            <BackendError error={statusError} onRetry={refresh} />
          ) : (
            // 切換頁面時其他頁面只是藏起來，搜尋結果和回答都還在
            (Object.keys(panes) as View[]).map((key) => (
              <div key={key} className={cn("absolute inset-0", key !== view && "hidden")}>
                {panes[key]}
              </div>
            ))
          )}
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}
