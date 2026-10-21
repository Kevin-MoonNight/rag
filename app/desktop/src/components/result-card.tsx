// 規範 T4：每筆搜尋結果一張 Card size="sm"
import { useLayoutEffect, useRef, useState } from "react";
import {
  CopyIcon,
  ExternalLinkIcon,
  FileImageIcon,
  FileTextIcon,
  FolderOpenIcon,
  MoreHorizontalIcon,
  ScanLineIcon,
  TextQuoteIcon,
} from "lucide-react";
import type { ResultItem } from "@/api";
import { PageImage } from "@/components/page-image";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { copyText, openPath, revealPath } from "@/lib/actions";
import { fileName, folderOf, isImageFile, pages, shortenPath, startsWithHeading } from "@/lib/format";
import { cn } from "@/lib/utils";

/** 比對方式是分類，不是狀態：一律用中性的 outline Badge（規範 1.1） */
const SOURCE: Record<ResultItem["source"], { label: string; hint: string }> = {
  text: { label: "內容相近", hint: "意思和查詢相近的段落，用字不一定一樣" },
  image: { label: "頁面影像相近", hint: "以整頁影像比對找到的頁面" },
  keyword: { label: "字詞相符", hint: "段落裡有查詢的字詞" },
};

/** 段落文字，關鍵字的位置用 <mark> 標出來 */
function marked(text: string, spans: [number, number][]) {
  const parts: React.ReactNode[] = [];
  let at = 0;
  spans.forEach(([start, end], i) => {
    if (start < at) return;
    if (start > at) parts.push(text.slice(at, start));
    parts.push(<mark key={i}>{text.slice(start, end)}</mark>);
    at = end;
  });
  parts.push(text.slice(at));
  return parts;
}

/** 段落最多 4 行（規範 1.9），超過的才有「顯示整段」 */
function Snippet({ item }: { item: ResultItem }) {
  const ref = useRef<HTMLParagraphElement>(null);
  const [long, setLong] = useState(false);
  const [open, setOpen] = useState(false);
  useLayoutEffect(() => {
    const el = ref.current;
    if (el && !open) setLong(el.scrollHeight > el.clientHeight + 2);
  }, [item.text, open]);
  return (
    <div className="flex flex-col items-start gap-1">
      <p ref={ref} className={cn("text-sm leading-6 whitespace-pre-line select-text", !open && "line-clamp-4")}>
        {marked(item.text!, item.highlights)}
      </p>
      {(long || open) && (
        <button
          type="button"
          onClick={() => setOpen(!open)}
          aria-expanded={open}
          className="rounded-sm text-sm text-brand underline-offset-4 outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
        >
          {open ? "收起" : "顯示整段"}
        </button>
      )}
    </div>
  );
}

/** 可以點的檔名（text-brand），滑過顯示全名 */
function FileLink({ path, className }: { path: string; className?: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={() => openPath(path)}
          className={cn(
            "min-w-0 truncate rounded-sm text-left text-brand underline-offset-4 outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/50",
            className,
          )}
        >
          {fileName(path)}
        </button>
      </TooltipTrigger>
      <TooltipContent className="max-w-md break-all">開啟 {path}</TooltipContent>
    </Tooltip>
  );
}

export function ResultCard({ item }: { item: ResultItem }) {
  const source = SOURCE[item.source];
  const name = fileName(item.file_path);
  const folder = folderOf(item.file_path);
  const Icon = item.is_image ? (isImageFile(item.file_path) ? FileImageIcon : ScanLineIcon) : FileTextIcon;
  const pageText = `第 ${pages(item.page, item.page_end)} 頁`;

  return (
    <li>
      <Card size="sm">
        <CardHeader>
          <CardTitle className="flex min-w-0 items-center gap-2">
            <Icon className="size-4 shrink-0 text-muted-foreground" aria-hidden />
            <FileLink path={item.file_path} />
          </CardTitle>
          <CardDescription className="min-w-0">
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="block truncate select-text">{shortenPath(folder)}</span>
              </TooltipTrigger>
              <TooltipContent className="max-w-md break-all">{folder}</TooltipContent>
            </Tooltip>
          </CardDescription>
          <CardAction>
            <DropdownMenu>
              <Tooltip>
                <TooltipTrigger asChild>
                  <DropdownMenuTrigger asChild>
                    <Button variant="ghost" size="icon" aria-label="更多動作">
                      <MoreHorizontalIcon />
                    </Button>
                  </DropdownMenuTrigger>
                </TooltipTrigger>
                <TooltipContent>更多動作</TooltipContent>
              </Tooltip>
              <DropdownMenuContent align="end" className="w-48">
                <DropdownMenuItem onSelect={() => openPath(item.file_path)}>
                  <ExternalLinkIcon />
                  開啟檔案
                </DropdownMenuItem>
                <DropdownMenuItem onSelect={() => revealPath(item.file_path)}>
                  <FolderOpenIcon />
                  在檔案總管中顯示
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem onSelect={() => copyText(item.file_path, "檔案路徑")}>
                  <CopyIcon />
                  複製檔案路徑
                </DropdownMenuItem>
                <DropdownMenuItem disabled={!item.text} onSelect={() => item.text && copyText(item.text, "段落文字")}>
                  <TextQuoteIcon />
                  複製段落文字
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </CardAction>
        </CardHeader>

        <CardContent className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-1.5">
            <Badge variant="outline">{pageText}</Badge>
            <Tooltip>
              <TooltipTrigger asChild>
                <Badge variant="secondary">{source.label}</Badge>
              </TooltipTrigger>
              <TooltipContent>{source.hint}</TooltipContent>
            </Tooltip>
          </div>
          {item.heading && !startsWithHeading(item) && (
            <div className="text-sm font-medium select-text">{item.heading}</div>
          )}
          {item.text && <Snippet item={item} />}
          {item.is_image && item.image_path && (
            <>
              <p className="text-sm text-muted-foreground">這一頁是以頁面影像比對到的，請直接看頁面。</p>
              <PageImage
                path={item.image_path}
                title={name}
                description={pageText}
                onOpenFile={() => openPath(item.file_path)}
              />
            </>
          )}
          {item.same_content.length > 0 && (
            <p className="text-sm text-muted-foreground">
              內容相同的檔案：
              {item.same_content.map((p, i) => (
                <span key={p}>
                  {i > 0 && "、"}
                  <FileLink path={p} className="inline" />
                </span>
              ))}
            </p>
          )}
        </CardContent>
      </Card>
    </li>
  );
}
