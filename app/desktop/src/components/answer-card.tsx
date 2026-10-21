// 規範 §3「AI 回答」：Card＋標頭動作，引用編號用 Popover 顯示出處
import { useCallback, useEffect, useRef, useState } from "react";
import { CircleStopIcon, CopyIcon, ExternalLinkIcon, InfoIcon, RotateCcwIcon, SparklesIcon, TriangleAlertIcon } from "lucide-react";
import { answer, AnswerOutcome, AnswerSource, stopAnswer } from "@/api";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { copyText, openPath } from "@/lib/actions";
import { errorText, fileName, pages, seconds } from "@/lib/format";
import { cn } from "@/lib/utils";

/** 回答裡的引用：[1]、[1, 3]、[2][3] */
const CITATION = /\[(\d+(?:\s*[,，、]\s*\d+)*)\]/g;
const BOLD = /\*\*([^*\n]+)\*\*/g;

const CTRL_ENTER = (
  <KbdGroup>
    <Kbd>Ctrl</Kbd>
    <Kbd>Enter</Kbd>
  </KbdGroup>
);

/** 出處的內容：檔名、頁碼、標題、段落，和開啟檔案的按鈕 */
function SourceDetail({ source }: { source: AnswerSource }) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex min-w-0 items-baseline gap-2">
        <span className="min-w-0 truncate font-medium select-text">{fileName(source.file_path)}</span>
        <span className="shrink-0 text-muted-foreground">第 {pages(source.page, source.page_end)} 頁</span>
      </div>
      {source.heading && <div className="font-medium select-text">{source.heading}</div>}
      <p className="line-clamp-6 leading-6 whitespace-pre-line text-muted-foreground select-text">{source.text}</p>
      <Button variant="outline" size="sm" className="self-start" onClick={() => openPath(source.file_path)}>
        <ExternalLinkIcon data-icon="inline-start" />
        開啟檔案
      </Button>
    </div>
  );
}

/** 引用編號小方標（規範 E11）；點一下或按 Enter 打開出處 */
function Citation({ number, source }: { number: number; source: AnswerSource | undefined }) {
  const chip = (
    <Badge
      asChild
      variant="outline"
      className="mx-0.5 h-5 min-w-5 px-1 align-baseline text-brand tabular-nums transition-colors hover:bg-accent"
    >
      <button type="button" disabled={!source} aria-label={`出處 ${number}`}>
        {number}
      </button>
    </Badge>
  );
  if (!source) return chip;
  return (
    <Popover>
      <PopoverTrigger asChild>{chip}</PopoverTrigger>
      <PopoverContent className="w-80" side="top">
        <SourceDetail source={source} />
      </PopoverContent>
    </Popover>
  );
}

/** 回答文字：引用編號換成可以點的小方標，**粗體** 換成粗體 */
function AnswerText({ text, sources }: { text: string; sources: AnswerSource[] }) {
  const byNumber = new Map(sources.map((s) => [s.number, s]));
  const nodes: React.ReactNode[] = [];
  let at = 0;
  let key = 0;
  const plain = (s: string) => {
    let i = 0;
    for (const m of s.matchAll(BOLD)) {
      if (m.index! > i) nodes.push(s.slice(i, m.index));
      nodes.push(<strong key={key++} className="font-semibold">{m[1]}</strong>);
      i = m.index! + m[0].length;
    }
    nodes.push(s.slice(i));
  };
  for (const m of text.matchAll(CITATION)) {
    plain(text.slice(at, m.index));
    for (const n of m[1].split(/\s*[,，、]\s*/).map(Number)) {
      nodes.push(<Citation key={key++} number={n} source={byNumber.get(n)} />);
    }
    at = m.index! + m[0].length;
  }
  plain(text.slice(at));
  return <>{nodes}</>;
}

/** 參考資料清單：每一筆點開看出處；回答完後沒用到的變淡 */
function SourceList({ sources, cited }: { sources: AnswerSource[]; cited: Set<number> | null }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="text-sm font-medium">參考資料</div>
      <ol className="-mx-2 flex flex-col">
        {sources.map((s) => {
          const used = !cited || cited.has(s.number);
          return (
            <li key={s.number}>
              <Popover>
                <PopoverTrigger asChild>
                  <button
                    type="button"
                    className="flex w-full min-w-0 items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors outline-none hover:bg-accent focus-visible:ring-3 focus-visible:ring-ring/50"
                  >
                    <Badge variant="outline" className={cn("h-5 min-w-5 px-1 tabular-nums", used ? "text-brand" : "text-muted-foreground")}>
                      {s.number}
                    </Badge>
                    <span className={cn("min-w-0 truncate", !used && "text-muted-foreground")}>{fileName(s.file_path)}</span>
                    <span className="shrink-0 text-muted-foreground">第 {pages(s.page, s.page_end)} 頁</span>
                    {!used && (
                      <Badge variant="secondary" className="ml-auto">
                        沒有引用
                      </Badge>
                    )}
                  </button>
                </PopoverTrigger>
                <PopoverContent className="w-80" side="left" align="start">
                  <SourceDetail source={s} />
                </PopoverContent>
              </Popover>
            </li>
          );
        })}
      </ol>
    </div>
  );
}

export function AnswerCard({ query, unavailable }: { query: string; unavailable: string | null }) {
  const [running, setRunning] = useState(false);
  const [text, setText] = useState("");
  const [sources, setSources] = useState<AnswerSource[]>([]);
  const [outcome, setOutcome] = useState<AnswerOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [stopping, setStopping] = useState(false);

  // 換了查詢就清掉上一次的回答
  useEffect(() => {
    setText("");
    setSources([]);
    setOutcome(null);
    setError(null);
  }, [query]);

  const start = useCallback(async () => {
    setRunning(true);
    setStopping(false);
    setText("");
    setSources([]);
    setOutcome(null);
    setError(null);
    try {
      const result = await answer(query, (e) => {
        if (e.kind === "sources") setSources(e.sources);
        else setText((t) => t + e.text);
      });
      setOutcome(result);
      setSources(result.sources);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setRunning(false);
    }
  }, [query]);

  const stop = () => {
    setStopping(true);
    stopAnswer();
  };

  // Ctrl+Enter 產生回答
  const startRef = useRef<() => void>(start);
  startRef.current = running ? () => {} : start;
  useEffect(() => {
    if (unavailable) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        startRef.current();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [unavailable]);

  const g = outcome?.generation;
  const done = !!(outcome || error);
  const notes = [
    g?.cancelled && "已停止，上面是停止前產生的部分。",
    g?.finish_reason === "length" && "回答太長，後面被截斷。",
    outcome && outcome.skipped_images > 0 && `搜尋結果裡有 ${outcome.skipped_images} 筆掃描頁沒有交給模型，請直接看頁面。`,
  ].filter(Boolean) as string[];

  const actions = unavailable ? null : running ? (
    <Button variant="outline" onClick={stop} disabled={stopping}>
      {stopping ? <Spinner data-icon="inline-start" /> : <CircleStopIcon data-icon="inline-start" />}
      停止
    </Button>
  ) : done ? (
    <div className="flex items-center gap-2">
      {text && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" onClick={() => copyText(text.trim(), "回答")} aria-label="複製回答">
              <CopyIcon />
            </Button>
          </TooltipTrigger>
          <TooltipContent>複製回答</TooltipContent>
        </Tooltip>
      )}
      <Tooltip>
        <TooltipTrigger asChild>
          <Button variant="outline" onClick={start}>
            <RotateCcwIcon data-icon="inline-start" />
            重新回答
          </Button>
        </TooltipTrigger>
        <TooltipContent>{CTRL_ENTER}</TooltipContent>
      </Tooltip>
    </div>
  ) : (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button onClick={start}>
          <SparklesIcon data-icon="inline-start" />
          產生回答
        </Button>
      </TooltipTrigger>
      <TooltipContent>{CTRL_ENTER}</TooltipContent>
    </Tooltip>
  );

  return (
    <Card>
      <CardHeader className="border-b">
        <CardTitle className="flex items-center gap-2">
          <SparklesIcon className="size-4 text-brand" aria-hidden />
          AI 回答
        </CardTitle>
        <CardDescription>由這台電腦上的模型根據搜尋結果整理</CardDescription>
        {actions && <CardAction>{actions}</CardAction>}
      </CardHeader>

      <CardContent className="flex flex-col gap-4">
        {unavailable ? (
          <Alert>
            <InfoIcon />
            <AlertTitle>這台電腦不能產生回答</AlertTitle>
            <AlertDescription>{unavailable}，只提供搜尋。</AlertDescription>
          </Alert>
        ) : !running && !done ? (
          <p className="leading-6 text-muted-foreground">
            按「產生回答」，模型會讀搜尋結果裡最相關的幾段，整理出回答並標出出處。資料不會離開這台電腦。
          </p>
        ) : null}

        {running && !text && (
          <div className="flex flex-col gap-3" aria-live="polite">
            <p className="flex items-center gap-2 text-muted-foreground">
              <Spinner />
              正在讀資料，第一次使用要先載入模型，約需十幾秒……
            </p>
            <Skeleton className="h-4 w-11/12" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-3/4" />
          </div>
        )}

        {text && (
          <div className={cn("leading-6 whitespace-pre-wrap select-text", running && "answer-caret")}>
            <AnswerText text={text.trim()} sources={sources} />
          </div>
        )}

        {outcome?.not_found && (
          <Alert>
            <InfoIcon />
            <AlertTitle>搜尋結果裡沒有答案</AlertTitle>
            <AlertDescription>找到的資料裡沒有可以回答這個問題的內容，可以換個說法再搜尋一次。</AlertDescription>
          </Alert>
        )}

        {error && (
          <Alert variant="destructive">
            <TriangleAlertIcon />
            <AlertTitle>產生回答失敗</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        {notes.map((n) => (
          <p key={n} className="text-muted-foreground">
            {n}
          </p>
        ))}

        {sources.length > 0 && <SourceList sources={sources} cited={outcome ? new Set(outcome.cited) : null} />}
      </CardContent>

      {!unavailable && (text || running) && (
        <CardFooter className="flex-col items-start gap-1 text-muted-foreground">
          {g && g.total_ms > 0 && (
            <span className="tabular-nums">
              第一個字 {seconds(g.first_text_ms)}，全部 {seconds(g.total_ms)}
              {g.load_ms > 0 && `（含載入模型 ${seconds(g.load_ms)}）`}
            </span>
          )}
          <span>回答由模型根據參考資料產生，可能有錯，請以原始文件為準。</span>
        </CardFooter>
      )}
    </Card>
  );
}
