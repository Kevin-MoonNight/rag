// 規範 §3 索引：T2 頁面＋T5 統計卡＋建立中、上次更新、不支援的檔案三張卡
import { useState } from "react";
import { toast } from "sonner";
import {
  ArrowRightIcon,
  ChevronDownIcon,
  CircleStopIcon,
  CopyIcon,
  DatabaseIcon,
  FolderSearchIcon,
  ImageOffIcon,
  PauseIcon,
  PlayIcon,
  RefreshCwIcon,
  TriangleAlertIcon,
} from "lucide-react";
import { pauseIndex, Status, stopIndex, updateNow } from "@/api";
import { Page, PageIntro } from "@/components/page-header";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { copyText } from "@/lib/actions";
import { errorText, fileName, folderOf, formatDuration, num, shortenPath } from "@/lib/format";

/** T5 統計卡 */
function Stat({ label, value, note, alert }: { label: string; value: number; note?: string; alert?: string }) {
  return (
    <Card>
      <CardHeader>
        <CardDescription>{label}</CardDescription>
        <CardTitle className="text-2xl font-semibold tabular-nums">{num(value)}</CardTitle>
        {alert && (
          <CardAction>
            <Badge variant="outline">
              <span aria-hidden className="size-1.5 rounded-full bg-destructive" />
              {alert}
            </Badge>
          </CardAction>
        )}
      </CardHeader>
      {note && <CardContent className="text-muted-foreground">{note}</CardContent>}
    </Card>
  );
}

function StatSkeleton() {
  return (
    <Card>
      <CardHeader>
        <Skeleton className="h-5 w-24" />
        <Skeleton className="h-8 w-20" />
      </CardHeader>
    </Card>
  );
}

/** 建立中的卡：進度、目前的檔案、暫停／停止 */
function RunningCard({ status, act }: { status: Status; act: (what: string, action: () => Promise<void>) => void }) {
  const p = status.progress;
  const percent = p.total > 0 ? Math.min(100, Math.round((p.done / p.total) * 100)) : 0;
  const pausedNow = p.user_paused || !!p.paused;
  const title = p.finishing ? "正在整理索引" : pausedNow ? "建立索引已暫停" : "正在建立索引";
  const state = p.user_paused
    ? p.paused
      ? "按「繼續」從目前的進度接著做。"
      : "目前的檔案做完就暫停……"
    : p.paused
      ? `${p.paused}，情況改善後會自動繼續。`
      : p.mode
        ? `${p.mode}。`
        : "";

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {pausedNow ? <PauseIcon className="size-4" /> : <Spinner />}
          {title}
        </CardTitle>
        {state && <CardDescription>{state}</CardDescription>}
        <CardAction className="flex items-center gap-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="outline" onClick={() => act(p.user_paused ? "繼續" : "暫停", () => pauseIndex(!p.user_paused))}>
                {p.user_paused ? <PlayIcon data-icon="inline-start" /> : <PauseIcon data-icon="inline-start" />}
                {p.user_paused ? "繼續" : "暫停"}
              </Button>
            </TooltipTrigger>
            <TooltipContent className="max-w-64">
              {p.user_paused ? "從目前的進度接著做" : "暫時停下來並釋放記憶體，之後按「繼續」接著做"}
            </TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="outline" onClick={() => act("停止", stopIndex)}>
                <CircleStopIcon data-icon="inline-start" />
                停止
              </Button>
            </TooltipTrigger>
            <TooltipContent className="max-w-64">結束這次更新。已完成的檔案會保留，下次更新從沒做完的接著做</TooltipContent>
          </Tooltip>
        </CardAction>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <p className="tabular-nums">
          {p.finishing ? (
            "建完之後的整理，馬上就好。"
          ) : (
            <>
              已處理 {num(p.done)} / {num(p.total)}（{percent}%）
              {p.estimate_seconds > 0 && `・估計全部約需 ${formatDuration(p.estimate_seconds)}`}
            </>
          )}
        </p>
        <Progress value={p.finishing ? 100 : percent} aria-label="建立索引的進度" />
        {p.current && !p.finishing && (
          <p className="flex min-w-0 items-center gap-2 text-muted-foreground">
            <span className="shrink-0">目前的檔案</span>
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="min-w-0 truncate text-foreground select-text">{fileName(p.current)}</span>
              </TooltipTrigger>
              <TooltipContent className="max-w-md break-all">{p.current}</TooltipContent>
            </Tooltip>
          </p>
        )}
      </CardContent>
      <CardFooter className="flex-col items-start gap-1 text-muted-foreground">
        {p.estimate_files > 0 && (
          <span>
            這次要處理 {num(p.estimate_files)} 個檔案、約 {num(p.estimate_chunks)} 個文字段落。白天在背景慢慢做，夜間全速。
          </span>
        )}
        <span>建立索引時仍然可以搜尋，已完成的檔案就搜尋得到。</span>
      </CardFooter>
    </Card>
  );
}

/** 上次更新的結果：6 個數字（規範 E5） */
function LastRunCard({ status }: { status: Status }) {
  const p = status.progress;
  const items: [string, number][] = [
    ["新建", p.indexed],
    ["沒有變動", p.unchanged],
    ["內容重複", p.duplicates],
    ["搬移或改名", p.moved],
    ["已刪除", p.removed],
    ["失敗", p.failed],
  ];
  const notes = [
    p.reembedded > 0 && `其中 ${num(p.reembedded)} 個因為換了模型而重算。`,
    p.offline_sources > 0 && `有 ${p.offline_sources} 個來源找不到，到「搜尋範圍」查看。`,
  ].filter(Boolean) as string[];
  return (
    <Card>
      <CardHeader>
        <CardTitle>上次更新</CardTitle>
        <CardDescription className="tabular-nums">
          {p.finished_at}
          {p.reason && `・${p.reason}`}
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <dl className="grid grid-cols-3 gap-4 @3xl/main:grid-cols-6">
          {items.map(([label, value]) => (
            <div key={label} className="flex flex-col gap-1">
              <dt className="text-muted-foreground">{label}</dt>
              <dd className="flex items-center gap-1.5 text-base font-medium tabular-nums">
                {label === "失敗" && value > 0 && <span aria-hidden className="size-1.5 rounded-full bg-destructive" />}
                {num(value)}
              </dd>
            </div>
          ))}
        </dl>
        {notes.map((n) => (
          <p key={n} className="text-muted-foreground">
            {n}
          </p>
        ))}
      </CardContent>
    </Card>
  );
}

/** 不支援的檔案：收起來的長清單（規範 T4） */
function UnsupportedCard({ files }: { files: string[] }) {
  const [open, setOpen] = useState(false);
  return (
    <Collapsible open={open} onOpenChange={setOpen} asChild>
      <Card>
        <CardHeader>
          <CardTitle>{num(files.length)} 個檔案的格式不支援，已略過</CardTitle>
          <CardDescription>只支援 PDF 和圖片檔（PNG、JPG）。{files.length >= 200 && "清單最多列出 200 個。"}</CardDescription>
          <CardAction>
            <CollapsibleTrigger asChild>
              <Button variant="ghost">
                {open ? "收起" : "顯示清單"}
                <ChevronDownIcon data-icon="inline-end" className={open ? "rotate-180 transition-transform" : "transition-transform"} />
              </Button>
            </CollapsibleTrigger>
          </CardAction>
        </CardHeader>
        <CollapsibleContent asChild>
          <CardContent className="flex flex-col gap-3">
            <ScrollArea className="rounded-lg border [&>[data-slot=scroll-area-viewport]]:max-h-96">
              <Table className="table-fixed">
                <TableHeader>
                  <TableRow>
                    <TableHead className="w-2/5">檔名</TableHead>
                    <TableHead>資料夾</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {files.map((f) => (
                    <TableRow key={f}>
                      <TableCell className="truncate select-text">{fileName(f)}</TableCell>
                      <TableCell className="truncate text-muted-foreground select-text">{shortenPath(folderOf(f))}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </ScrollArea>
            <Button variant="outline" className="self-start" onClick={() => copyText(files.join("\n"), "清單")}>
              <CopyIcon data-icon="inline-start" />
              複製清單
            </Button>
          </CardContent>
        </CollapsibleContent>
      </Card>
    </Collapsible>
  );
}

export function IndexView({
  status,
  sourceCount,
  refresh,
  onAddSources,
}: {
  status: Status | null;
  /** 搜尋範圍裡有幾個來源；還沒讀到是 null */
  sourceCount: number | null;
  refresh: () => void;
  onAddSources: () => void;
}) {
  const [starting, setStarting] = useState(false);
  const progress = status?.progress;
  const running = !!progress?.running;
  const empty = !!status && status.files === 0 && !running && !progress?.finished_at;
  // 索引裡有檔案、搜尋範圍卻是空的（例如用命令列建的索引）：更新不會做任何事，要先加入來源
  const noSources = !!status && status.files > 0 && sourceCount === 0 && !running;

  const act = (what: string, action: () => Promise<void>) => {
    action()
      .catch((e) => toast.error(`${what}失敗`, { description: errorText(e) }))
      .finally(refresh);
  };

  const run = async () => {
    setStarting(true);
    try {
      await updateNow();
    } catch (e) {
      toast.error("無法開始更新", { description: errorText(e) });
    } finally {
      setStarting(false);
      refresh();
    }
  };

  return (
    <Page>
      <PageIntro
        description="索引是搜尋用的資料，建在這台電腦上。檔案新增、修改、搬移後，更新索引才搜尋得到最新的內容。"
        actions={
          running || empty || noSources ? null : (
            <Button onClick={run} disabled={starting || !status}>
              {starting ? <Spinner data-icon="inline-start" /> : <RefreshCwIcon data-icon="inline-start" />}
              立即更新
            </Button>
          )
        }
      />

      {progress?.error && (
        <Alert variant="destructive">
          <TriangleAlertIcon />
          <AlertTitle>更新時發生錯誤</AlertTitle>
          <AlertDescription>{progress.error}</AlertDescription>
        </Alert>
      )}

      {noSources && (
        <Alert>
          <FolderSearchIcon className="text-warning!" />
          <AlertTitle>搜尋範圍是空的，索引沒辦法更新</AlertTitle>
          <AlertDescription>
            <p>
              索引裡有 {num(status.files)} 個檔案，但沒有記錄它們來自哪些資料夾（例如用命令列建的索引）。現在仍然可以搜尋；要更新或重建，請把它們所在的資料夾加入搜尋範圍，加入後會自動開始更新。
            </p>
            <Button variant="outline" size="sm" className="mt-2" onClick={onAddSources}>
              前往搜尋範圍
              <ArrowRightIcon data-icon="inline-end" />
            </Button>
          </AlertDescription>
        </Alert>
      )}

      {status && !status.can_index_images && (
        <Alert>
          <ImageOffIcon className="text-warning!" />
          <AlertTitle>找不到影像模型</AlertTitle>
          <AlertDescription>掃描的頁面和圖片檔只會用辨識出的文字建立索引，不會以頁面影像比對。請重新安裝程式補上模型檔。</AlertDescription>
        </Alert>
      )}

      {empty ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <DatabaseIcon />
            </EmptyMedia>
            <EmptyTitle>還沒有索引</EmptyTitle>
            <EmptyDescription>先加入要搜尋的資料夾或檔案，程式會自動在背景建立索引。</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button onClick={onAddSources}>
              加入資料夾
              <ArrowRightIcon data-icon="inline-end" />
            </Button>
          </EmptyContent>
        </Empty>
      ) : (
        <>
          <div className="grid gap-4 @xl/main:grid-cols-2 @5xl/main:grid-cols-4">
            {!status ? (
              [0, 1, 2, 3].map((i) => <StatSkeleton key={i} />)
            ) : (
              <>
                <Stat label="可以搜尋的檔案" value={status.indexed} />
                <Stat label="文字段落" value={status.text_chunks} />
                <Stat label="頁面影像" value={status.images} />
                <Stat
                  label="失敗的檔案"
                  value={status.failed}
                  alert={status.failed > 0 ? "沒有建入" : undefined}
                  note={status.failed > 0 ? "可能是檔案損壞、加了密碼，或格式有問題。" : undefined}
                />
              </>
            )}
          </div>
          {status && running && <RunningCard status={status} act={act} />}
          {status && !running && progress?.finished_at && <LastRunCard status={status} />}
          {progress && progress.unsupported_files.length > 0 && <UnsupportedCard files={progress.unsupported_files} />}
        </>
      )}
    </Page>
  );
}
