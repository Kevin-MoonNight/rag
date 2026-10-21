// 規範 §3 搜尋頁：上方固定的搜尋列，下面結果和 AI 回答（寬的時候兩欄各自捲動，E14）
import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowRightIcon,
  FolderPlusIcon,
  HistoryIcon,
  SearchIcon,
  SearchXIcon,
  SparklesIcon,
  TriangleAlertIcon,
  TypeIcon,
  XIcon,
} from "lucide-react";
import { preload, ResultItem, search, Status } from "@/api";
import { AnswerCard } from "@/components/answer-card";
import { ResultCard } from "@/components/result-card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from "@/components/ui/input-group";
import { Kbd } from "@/components/ui/kbd";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { errorText, num, seconds } from "@/lib/format";
import { cn } from "@/lib/utils";

type Tab = "all" | "docs" | "images";

const TABS: [Tab, string][] = [
  ["all", "全部"],
  ["docs", "文件"],
  ["images", "頁面影像"],
];

interface Shown {
  query: string;
  results: ResultItem[];
  milliseconds: number;
  limit: number;
  keywordOnly: boolean;
  /** 模型還沒載入時先給的字詞相符結果，完整結果還在路上 */
  quick: boolean;
}

/** 頁面內距和區段間距（規範 1.5） */
const PAGE = "px-4 py-4 @3xl/main:px-6 @3xl/main:py-6";

function Tips() {
  const tips: [React.ReactNode, string][] = [
    [<SearchIcon key="i" />, "可以直接用一句話問，例如「請假超過三天要由誰核准」，意思相近但用字不同的內容也找得到。"],
    [<TypeIcon key="i" />, "要精確查代號、編號、病歷號時，開啟搜尋框右邊的「只用關鍵字」。"],
    [<SparklesIcon key="i" />, "找到結果後按「產生回答」，模型會整理出回答並標出每一句的出處。"],
  ];
  return (
    <ul className="flex flex-col gap-3">
      {tips.map(([icon, text]) => (
        <li key={text} className="flex items-start gap-3 text-sm leading-6 [&>svg]:mt-1 [&>svg]:size-4 [&>svg]:shrink-0 [&>svg]:text-brand">
          {icon}
          <span>{text}</span>
        </li>
      ))}
    </ul>
  );
}

/** 搜尋中：3 張和結果卡同形狀的 Skeleton（規範 T6） */
function ResultSkeletons() {
  return (
    <div className="flex flex-col gap-3" aria-label="搜尋中">
      {[0, 1, 2].map((i) => (
        <Card key={i} size="sm">
          <CardHeader>
            <Skeleton className="h-5 w-1/3" />
            <Skeleton className="h-4 w-1/2" />
          </CardHeader>
          <CardContent className="flex flex-col gap-2">
            <Skeleton className="h-5 w-24" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-11/12" />
            <Skeleton className="h-4 w-2/3" />
          </CardContent>
        </Card>
      ))}
    </div>
  );
}

export function SearchView({
  status,
  focusSignal,
  onAddSources,
}: {
  status: Status | null;
  /** 每次變動就把游標移到搜尋框（Ctrl+K） */
  focusSignal: number;
  onAddSources: () => void;
}) {
  const [query, setQuery] = useState("");
  const [shown, setShown] = useState<Shown | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  /** 只用關鍵字搜尋（精確查代號、病歷號時用） */
  const [keywordOnly, setKeywordOnly] = useState(false);
  const [tab, setTab] = useState<Tab>("all");
  /** 這次開程式之後搜尋過的字：只在記憶體，關掉程式就沒了（病歷號、客戶編號不落地） */
  const [recent, setRecent] = useState<string[]>([]);
  const input = useRef<HTMLInputElement>(null);
  const resultsPane = useRef<HTMLDivElement>(null);

  const focusInput = () => {
    input.current?.focus();
    input.current?.select();
  };

  useEffect(() => {
    if (focusSignal) focusInput();
  }, [focusSignal]);

  // 點到輸入框、開始打字、視窗回到前景時先載入查詢模型，載入時間（約 1.7 秒）藏在打字的時候（實驗 96）
  const lastPreload = useRef(0);
  const warmQuery = useCallback(() => {
    if (keywordOnly || status?.model_loaded || Date.now() - lastPreload.current < 30_000) return;
    lastPreload.current = Date.now();
    preload(false).catch(() => {});
  }, [keywordOnly, status?.model_loaded]);
  useEffect(() => {
    window.addEventListener("focus", warmQuery);
    return () => window.removeEventListener("focus", warmQuery);
  }, [warmQuery]);

  const run = async (text: string, onlyKeywords = keywordOnly) => {
    const q = text.trim();
    // 按鈕不因為還沒輸入而停用；空的時候按了就回到輸入框（規範 T6）
    if (!q) return focusInput();
    if (busy) return;
    setQuery(q);
    setBusy(true);
    setError(null);
    setShown(null);
    setRecent((r) => [q, ...r.filter((x) => x !== q)].slice(0, 8));
    const show = (results: ResultItem[], milliseconds: number, limit: number, quick: boolean) => {
      setShown({ query: q, results, milliseconds, limit, keywordOnly: onlyKeywords, quick });
      setTab("all");
      resultsPane.current?.scrollTo({ top: 0 });
    };
    try {
      if (!onlyKeywords && !status?.model_loaded) {
        // 模型還沒載入：先用關鍵字馬上給結果，模型載好後再換成完整的結果
        const quick = await search(q, true);
        show(quick.results, quick.milliseconds, quick.limit, true);
      }
      const response = await search(q, onlyKeywords);
      show(response.results, response.milliseconds, response.limit, false);
      // 使用者看結果的時候先載入回答模型（約 4.4 秒），按「產生回答」就不用等；記憶體不夠時後端不做
      if (response.results.length > 0 && !status?.answer_unavailable) preload(true).catch(() => {});
    } catch (err) {
      setError(errorText(err));
      // 完整的搜尋失敗時，留下已經顯示的字詞相符結果
      setShown((s) => (s?.quick ? { ...s, quick: false, keywordOnly: true } : s));
    } finally {
      setBusy(false);
    }
  };

  const results = shown?.results ?? [];
  const counts: Record<Tab, number> = {
    all: results.length,
    docs: results.filter((r) => !r.is_image).length,
    images: results.filter((r) => r.is_image).length,
  };
  const visible = results.filter((r) => tab === "all" || (tab === "images") === r.is_image);
  const capped = !!shown && shown.limit > 0 && results.length >= shown.limit;
  const nothingIndexed = status !== null && status.indexed === 0 && !status.progress.running;

  const errorAlert = error && (
    <Alert variant="destructive">
      <TriangleAlertIcon />
      <AlertTitle>搜尋失敗</AlertTitle>
      <AlertDescription>{error}</AlertDescription>
    </Alert>
  );

  let body: React.ReactNode;
  if (shown && results.length > 0) {
    body = (
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto @5xl/main:flex-row @5xl/main:overflow-hidden">
        {!shown.quick && (
          <aside className="px-4 pt-4 @3xl/main:px-6 @3xl/main:pt-6 @5xl/main:order-2 @5xl/main:max-w-xl @5xl/main:shrink-0 @5xl/main:basis-2/5 @5xl/main:overflow-y-auto @5xl/main:pb-6 @5xl/main:pl-0">
            <AnswerCard query={shown.query} unavailable={status?.answer_unavailable ?? null} />
          </aside>
        )}
        <section ref={resultsPane} className={cn("flex min-w-0 flex-1 flex-col gap-4 @5xl/main:overflow-y-auto", PAGE)}>
          {errorAlert}
          <div className="flex flex-wrap items-center justify-between gap-2">
            <Tabs value={tab} onValueChange={(v) => setTab(v as Tab)}>
              <TabsList>
                {TABS.map(([key, label]) => (
                  <TabsTrigger key={key} value={key} disabled={key !== "all" && counts[key] === 0}>
                    {label}
                    <Badge variant="secondary" className="tabular-nums">
                      {counts[key]}
                      {key === "all" && capped && "+"}
                    </Badge>
                  </TabsTrigger>
                ))}
              </TabsList>
            </Tabs>
            <p className="flex items-center gap-2 text-sm text-muted-foreground tabular-nums">
              {shown.quick ? (
                <>
                  <Spinner />
                  先顯示字詞相符的結果，模型載入中……
                </>
              ) : (
                <>
                  {capped ? `只列出最相關的 ${shown.limit} 筆` : `${num(results.length)} 筆結果`}・{seconds(shown.milliseconds)}
                  {shown.keywordOnly && "・只用關鍵字"}
                </>
              )}
            </p>
          </div>
          <ol className="flex flex-col gap-3">
            {visible.map((r) => (
              <ResultCard key={r.id} item={r} />
            ))}
          </ol>
          {capped && (
            <p className="text-sm text-muted-foreground">
              結果太多時只列出最相關的 {shown.limit} 筆。找不到要的內容時，可以把問題寫得更具體，或加上檔名、年份等字詞。
            </p>
          )}
        </section>
      </div>
    );
  } else {
    let content: React.ReactNode;
    if (busy) content = <ResultSkeletons />;
    else if (shown)
      content = (
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <SearchXIcon />
            </EmptyMedia>
            <EmptyTitle className="break-all">找不到和「{shown.query}」相關的內容</EmptyTitle>
            <EmptyDescription>
              {shown.keywordOnly
                ? "目前只用關鍵字搜尋，改用一般搜尋可以找到意思相近、用字不同的內容。"
                : "可以換個說法，或到「搜尋範圍」確認檔案所在的資料夾已經加入。"}
            </EmptyDescription>
          </EmptyHeader>
          {shown.keywordOnly && (
            <EmptyContent>
              <Button
                variant="outline"
                onClick={() => {
                  setKeywordOnly(false);
                  run(shown.query, false);
                }}
              >
                改用一般搜尋
              </Button>
            </EmptyContent>
          )}
        </Empty>
      );
    else if (nothingIndexed)
      content = (
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <FolderPlusIcon />
            </EmptyMedia>
            <EmptyTitle>還沒有可以搜尋的文件</EmptyTitle>
            <EmptyDescription>先加入要搜尋的資料夾或檔案，程式會在背景建立索引，建好的檔案就可以搜尋。</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button onClick={onAddSources}>
              加入資料夾
              <ArrowRightIcon data-icon="inline-end" />
            </Button>
          </EmptyContent>
        </Empty>
      );
    else
      content = (
        <div className="flex flex-col gap-8 @3xl/main:pt-6">
          <div className="flex flex-col gap-2">
            {/* 規範 E9：歡迎標題 */}
            <h2 className="text-2xl font-semibold">要找什麼？</h2>
            <p className="text-sm text-muted-foreground">
              {status
                ? `可以搜尋 ${num(status.indexed)} 個檔案、${num(status.text_chunks)} 個段落${
                    status.progress.running ? "。索引還在建立中，已經完成的檔案就搜尋得到" : ""
                  }。`
                : "讀取索引狀態……"}
            </p>
          </div>
          <Tips />
          {recent.length > 0 && (
            <div className="flex flex-col gap-2">
              <div className="flex items-center gap-2">
                <span className="flex items-center gap-1.5 text-sm font-medium">
                  <HistoryIcon className="size-4 text-muted-foreground" />
                  剛才搜尋過
                </span>
                <Button variant="ghost" size="sm" className="ml-auto" onClick={() => setRecent([])}>
                  清除
                </Button>
              </div>
              <div className="flex flex-wrap gap-2">
                {recent.map((r) => (
                  <Button key={r} variant="outline" size="sm" className="max-w-full" onClick={() => run(r)}>
                    <span className="truncate">{r}</span>
                  </Button>
                ))}
              </div>
              <p className="text-sm text-muted-foreground">只記在這次開啟的程式裡，關掉就會清掉。</p>
            </div>
          )}
        </div>
      );
    body = (
      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className={cn("mx-auto flex max-w-2xl flex-col gap-4", PAGE)}>
          {errorAlert}
          {content}
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* 搜尋列：固定不捲（規範 §3）；不包 <form>、關掉自動填入和拼字檢查，打的字不會被瀏覽器記下來 */}
      <div className="border-b px-4 py-3 @3xl/main:px-6">
        <InputGroup className="h-10">
          <InputGroupAddon>{busy ? <Spinner /> : <SearchIcon />}</InputGroupAddon>
          <InputGroupInput
            ref={input}
            autoFocus
            autoComplete="off"
            spellCheck={false}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              warmQuery();
            }}
            onFocus={warmQuery}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.nativeEvent.isComposing) {
                e.preventDefault();
                // Ctrl+Enter 是產生回答，不要重新搜尋
                if (!e.ctrlKey && !e.metaKey) run(query);
              }
              if (e.key === "Escape" && query) {
                e.preventDefault();
                setQuery("");
              }
            }}
            placeholder="輸入問題或關鍵字"
            aria-label="搜尋"
          />
          <InputGroupAddon align="inline-end">
            {query && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <InputGroupButton size="icon-xs" aria-label="清除" onClick={() => (setQuery(""), input.current?.focus())}>
                    <XIcon />
                  </InputGroupButton>
                </TooltipTrigger>
                <TooltipContent>
                  清除 <Kbd>Esc</Kbd>
                </TooltipContent>
              </Tooltip>
            )}
            <Tooltip>
              <TooltipTrigger asChild>
                <InputGroupButton
                  variant={keywordOnly ? "secondary" : "ghost"}
                  aria-pressed={keywordOnly}
                  onClick={() => setKeywordOnly(!keywordOnly)}
                  className={cn(keywordOnly && "text-brand")}
                >
                  <TypeIcon />
                  只用關鍵字
                </InputGroupButton>
              </TooltipTrigger>
              <TooltipContent className="max-w-64">精確查代號、編號時用；不會找到意思相近但用字不同的內容</TooltipContent>
            </Tooltip>
            <Separator orientation="vertical" className="data-[orientation=vertical]:h-4" />
            <Tooltip>
              <TooltipTrigger asChild>
                <InputGroupButton variant="secondary" size="sm" onClick={() => run(query)} disabled={busy}>
                  搜尋
                </InputGroupButton>
              </TooltipTrigger>
              <TooltipContent>
                <Kbd>Enter</Kbd>
              </TooltipContent>
            </Tooltip>
          </InputGroupAddon>
        </InputGroup>
      </div>
      {body}
    </div>
  );
}
