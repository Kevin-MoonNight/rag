// 規範 T2：一般頁面的版面。頁面名稱只在頁首，內容區第一列是說明和動作按鈕
import { cn } from "@/lib/utils";

/** 頁面的捲動區和內距；width 是內容寬（一般頁 max-w-5xl，設定頁 max-w-2xl） */
export function Page({ width = "max-w-5xl", children }: { width?: string; children: React.ReactNode }) {
  return (
    <div className="h-full overflow-y-auto">
      <div className={cn("mx-auto flex flex-col gap-4 px-4 py-4 @3xl/main:gap-6 @3xl/main:px-6 @3xl/main:py-6", width)}>
        {children}
      </div>
    </div>
  );
}

/** 第一列：說明文字＋右邊的動作按鈕 */
export function PageIntro({ description, actions }: { description: React.ReactNode; actions?: React.ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3">
      <p className="min-w-64 flex-1 text-sm text-muted-foreground">{description}</p>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  );
}
