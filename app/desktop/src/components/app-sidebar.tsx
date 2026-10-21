// 規範 T1：shadcn block dashboard-01／sidebar-07 的側欄（inset、可收成圖示）
import {
  CircleDashedIcon,
  CircleCheckIcon,
  DatabaseIcon,
  FileSearchIcon,
  FolderOpenIcon,
  LockIcon,
  PauseIcon,
  SearchIcon,
  SettingsIcon,
} from "lucide-react";
import type { Status } from "@/api";
import { Progress } from "@/components/ui/progress";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
import { Spinner } from "@/components/ui/spinner";
import { num } from "@/lib/format";
import { cn } from "@/lib/utils";

export type View = "search" | "sources" | "index" | "settings";

export const VIEW_TITLES: Record<View, string> = {
  search: "搜尋",
  sources: "搜尋範圍",
  index: "索引",
  settings: "設定",
};

const ICONS: Record<View, React.ReactNode> = {
  search: <SearchIcon />,
  sources: <FolderOpenIcon />,
  index: <DatabaseIcon />,
  settings: <SettingsIcon />,
};

/** 6px 狀態圓點（規範 1.1：狀態色只用在圖示、短字和圓點） */
function Dot({ tone }: { tone: "warning" | "destructive" }) {
  return <span aria-hidden className={cn("size-1.5 rounded-full", tone === "warning" ? "bg-warning" : "bg-destructive")} />;
}

/** 索引狀態的一句話和圖示；側欄展開時的小卡和收合時的圖示按鈕共用 */
function summary(status: Status | null) {
  const p = status?.progress;
  const paused = !!(p?.user_paused || p?.paused);
  if (!status || !p) return { icon: <Spinner />, text: "讀取狀態……" };
  if (p.running)
    return {
      icon: paused ? <PauseIcon /> : <Spinner />,
      text: p.finishing ? "正在整理索引" : paused ? "建立索引已暫停" : "正在建立索引",
    };
  // 失敗的檔案由「索引」的數字標示，這裡只說能不能搜尋
  if (status.indexed === 0) return { icon: <CircleDashedIcon />, text: "還沒有可以搜尋的檔案" };
  return { icon: <CircleCheckIcon className="text-success" />, text: `${num(status.indexed)} 個檔案可以搜尋` };
}

/** 側欄下方的索引狀態小卡：建立索引時顯示進度，點一下到索引頁 */
function IndexSummary({ status, onClick }: { status: Status | null; onClick: () => void }) {
  const p = status?.progress;
  const { icon, text } = summary(status);
  const percent = p && p.total > 0 ? Math.round((p.done / p.total) * 100) : 0;
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex w-full flex-col gap-2 rounded-lg border bg-background p-3 text-left text-sm transition-colors outline-none hover:bg-sidebar-accent focus-visible:ring-2 focus-visible:ring-sidebar-ring group-data-[collapsible=icon]:hidden"
    >
      <span className="flex items-center gap-2 font-medium [&_svg]:size-4 [&_svg]:shrink-0">
        {icon}
        <span className="truncate">{text}</span>
      </span>
      {p?.running && (
        <>
          <Progress value={percent} aria-label="建立索引的進度" />
          <span className="text-xs text-muted-foreground tabular-nums">
            {num(p.done)} / {num(p.total)}（{percent}%）
          </span>
        </>
      )}
      <span className="flex items-center gap-2 text-muted-foreground">
        <LockIcon className="size-4 shrink-0" />
        資料只留在這台電腦
      </span>
    </button>
  );
}

function NavItem({
  view,
  active,
  badge,
  onNavigate,
}: {
  view: View;
  active: boolean;
  badge?: React.ReactNode;
  onNavigate: (view: View) => void;
}) {
  return (
    <SidebarMenuItem>
      <SidebarMenuButton isActive={active} onClick={() => onNavigate(view)} tooltip={VIEW_TITLES[view]}>
        {ICONS[view]}
        <span>{VIEW_TITLES[view]}</span>
      </SidebarMenuButton>
      {badge != null && <SidebarMenuBadge className="gap-1.5">{badge}</SidebarMenuBadge>}
    </SidebarMenuItem>
  );
}

export function AppSidebar({
  view,
  onNavigate,
  status,
  sourceCount,
  offlineCount,
}: {
  view: View;
  onNavigate: (view: View) => void;
  status: Status | null;
  sourceCount: number | null;
  offlineCount: number;
}) {
  const running = !!status?.progress.running;
  const collapsed = summary(status);
  return (
    <Sidebar variant="inset" collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" className="pointer-events-none" tabIndex={-1}>
              <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sidebar-primary-foreground">
                <FileSearchIcon className="size-4" />
              </div>
              <div className="grid flex-1 text-left leading-tight">
                <span className="truncate font-semibold">文件搜尋</span>
                <span className="truncate text-xs text-muted-foreground">離線搜尋與問答</span>
              </div>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              <NavItem view="search" active={view === "search"} onNavigate={onNavigate} />
              <NavItem
                view="sources"
                active={view === "sources"}
                onNavigate={onNavigate}
                badge={
                  offlineCount > 0 ? (
                    <>
                      <Dot tone="warning" />
                      {offlineCount}
                      <span className="sr-only">個來源找不到</span>
                    </>
                  ) : (
                    sourceCount || null
                  )
                }
              />
              <NavItem
                view="index"
                active={view === "index"}
                onNavigate={onNavigate}
                badge={
                  running ? (
                    <Spinner />
                  ) : status && status.failed > 0 ? (
                    <>
                      <Dot tone="destructive" />
                      {num(status.failed)}
                      <span className="sr-only">個檔案失敗</span>
                    </>
                  ) : null
                }
              />
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
        {/* 設定固定在最下面（dashboard-01 的 NavSecondary） */}
        <SidebarGroup className="mt-auto">
          <SidebarGroupContent>
            <SidebarMenu>
              <NavItem view="settings" active={view === "settings"} onNavigate={onNavigate} />
              {/* 收成圖示時，索引狀態小卡換成一個狀態圖示 */}
              <SidebarMenuItem className="hidden group-data-[collapsible=icon]:block">
                <SidebarMenuButton onClick={() => onNavigate("index")} tooltip={collapsed.text}>
                  {collapsed.icon}
                  <span>{collapsed.text}</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter className="group-data-[collapsible=icon]:hidden">
        <IndexSummary status={status} onClick={() => onNavigate("index")} />
      </SidebarFooter>
    </Sidebar>
  );
}
