// 規範 T3 設定頁：一欄 max-w-2xl，每個區段一張 Card，每個設定一個水平 Field；改了就生效，成功不跳 toast
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { CopyIcon, FolderOpenIcon } from "lucide-react";
import { getMaxFileMb, getSchedule, openDataDir, Schedule, setMaxFileMb, setSchedule, Status } from "@/api";
import { Page } from "@/components/page-header";
import { Theme, THEMES, useTheme } from "@/components/theme-provider";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldSeparator,
  FieldTitle,
} from "@/components/ui/field";
import { InputGroup, InputGroupAddon, InputGroupInput, InputGroupText } from "@/components/ui/input-group";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Table, TableBody, TableCell, TableRow } from "@/components/ui/table";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { copyText } from "@/lib/actions";
import { errorText, shortenPath } from "@/lib/format";
import { TEXT_SIZES, TextSize, useTextSize } from "@/lib/text-size";

const HOURS = Array.from({ length: 24 }, (_, h) => h);
const hour = (h: number) => `${String(h).padStart(2, "0")}:00`;

const SHORTCUTS: [string, string][] = [
  ["回到搜尋框", "Ctrl+K 或 /"],
  ["搜尋", "Enter"],
  ["清除搜尋框", "Esc"],
  ["產生回答", "Ctrl+Enter"],
  ["收合或展開側欄", "Ctrl+B"],
];

/** 時間一律用 Select（規範 T3：不用 <input type="time">） */
function HourSelect({ id, value, onChange }: { id: string; value: number; onChange: (hour: number) => void }) {
  return (
    <Select value={String(value)} onValueChange={(v) => onChange(Number(v))}>
      <SelectTrigger id={id} className="w-40">
        <SelectValue />
      </SelectTrigger>
      <SelectContent position="popper" className="max-h-72">
        {HOURS.map((h) => (
          <SelectItem key={h} value={String(h)}>
            {hour(h)}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

/** 設定區段的骨架：Card 標題＋說明＋內容 */
function Section({ title, description, children }: { title: string; description?: string; children: React.ReactNode }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        {description && <CardDescription>{description}</CardDescription>}
      </CardHeader>
      <CardContent>{children}</CardContent>
    </Card>
  );
}

/** 模型狀態：可以用的是 success 圓點，不能用的是中性的 */
function Availability({ ok, yes, no }: { ok: boolean; yes: string; no: string }) {
  return (
    <Badge variant="outline">
      <span aria-hidden className={ok ? "size-1.5 rounded-full bg-success" : "size-1.5 rounded-full bg-muted-foreground"} />
      {ok ? yes : no}
    </Badge>
  );
}

export function SettingsView({ status }: { status: Status | null }) {
  const { theme, setTheme } = useTheme();
  const [textSize, setTextSize] = useTextSize();
  const [schedule, setScheduleState] = useState<Schedule | null>(null);
  const [maxMb, setMaxMb] = useState("");
  const [savedMb, setSavedMb] = useState<number | null>(null);
  const [mbError, setMbError] = useState<string | null>(null);

  useEffect(() => {
    getSchedule()
      .then(setScheduleState)
      .catch((e) => toast.error("讀不到更新設定", { description: errorText(e) }));
    getMaxFileMb()
      .then((mb) => {
        setSavedMb(mb);
        setMaxMb(String(mb));
      })
      .catch((e) => toast.error("讀不到檔案大小上限", { description: errorText(e) }));
  }, []);

  const change = (next: Schedule) => {
    const before = schedule;
    setScheduleState(next);
    setSchedule(next).catch((e) => {
      setScheduleState(before);
      toast.error("設定沒有存起來", { description: errorText(e) });
    });
  };

  // 數字在失焦或按 Enter 時套用；不合規定的值顯示在欄位下面（規範 T3）
  const saveMb = () => {
    const text = maxMb.trim();
    const mb = Number(text);
    if (!/^\d+$/.test(text) || mb < 1) {
      setMbError("請輸入 1 以上的整數。");
      return;
    }
    setMbError(null);
    if (mb === savedMb) return;
    setMaxFileMb(mb)
      .then(() => {
        setSavedMb(mb);
        setMaxMb(String(mb));
      })
      .catch((e) => toast.error("設定沒有存起來", { description: errorText(e) }));
  };

  const t = schedule?.throttle;

  return (
    <Page width="max-w-2xl">
      <Section title="自動更新索引" description="檔案有新增或修改時，索引要更新才搜尋得到。">
        {!schedule ? (
          <Skeleton className="h-24" />
        ) : (
          <FieldGroup>
            <Field orientation="horizontal">
              <FieldContent>
                <FieldLabel htmlFor="on-startup">程式啟動時更新</FieldLabel>
                <FieldDescription>每次打開程式就檢查有沒有新的或改過的檔案。</FieldDescription>
              </FieldContent>
              <Switch id="on-startup" checked={schedule.on_startup} onCheckedChange={(on) => change({ ...schedule, on_startup: on })} />
            </Field>
            <FieldSeparator />
            <Field orientation="horizontal">
              <FieldContent>
                <FieldLabel htmlFor="daily">每天定時更新</FieldLabel>
                <FieldDescription>程式開著的時候，每天在這個時間更新一次。</FieldDescription>
              </FieldContent>
              <Select
                value={schedule.daily_hour === null ? "off" : String(schedule.daily_hour)}
                onValueChange={(v) => change({ ...schedule, daily_hour: v === "off" ? null : Number(v) })}
              >
                <SelectTrigger id="daily" className="w-40">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent position="popper" className="max-h-72">
                  <SelectItem value="off">不定時更新</SelectItem>
                  {HOURS.map((h) => (
                    <SelectItem key={h} value={String(h)}>
                      每天 {hour(h)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
          </FieldGroup>
        )}
      </Section>

      <Section title="電腦資源" description="白天在背景慢慢做，不影響使用電腦；夜間全速。電腦忙或記憶體不夠時會自動暫停。">
        {!schedule || !t ? (
          <Skeleton className="h-24" />
        ) : (
          <FieldGroup>
            <Field orientation="horizontal">
              <FieldContent>
                <FieldLabel htmlFor="night-start">夜間開始</FieldLabel>
                <FieldDescription>從這個時間開始全速建立索引。</FieldDescription>
              </FieldContent>
              <HourSelect id="night-start" value={t.night_start} onChange={(h) => change({ ...schedule, throttle: { ...t, night_start: h } })} />
            </Field>
            <FieldSeparator />
            <Field orientation="horizontal">
              <FieldContent>
                <FieldLabel htmlFor="night-end">夜間結束</FieldLabel>
                <FieldDescription>到這個時間改回白天的背景模式。</FieldDescription>
              </FieldContent>
              <HourSelect id="night-end" value={t.night_end} onChange={(h) => change({ ...schedule, throttle: { ...t, night_end: h } })} />
            </Field>
            <FieldSeparator />
            <Field orientation="horizontal">
              <FieldContent>
                <FieldLabel htmlFor="day-threads">白天使用的執行緒</FieldLabel>
                <FieldDescription>數字越大白天建得越快，但電腦可能變慢。</FieldDescription>
              </FieldContent>
              <Select value={String(t.day_threads)} onValueChange={(v) => change({ ...schedule, throttle: { ...t, day_threads: Number(v) } })}>
                <SelectTrigger id="day-threads" className="w-40">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent position="popper">
                  {[1, 2, 3, 4].map((n) => (
                    <SelectItem key={n} value={String(n)}>
                      最多 {n} 個
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
          </FieldGroup>
        )}
      </Section>

      <Section title="檔案">
        <FieldGroup>
          <Field orientation="horizontal" data-invalid={!!mbError}>
            <FieldContent>
              <FieldLabel htmlFor="max-mb">單一檔案上限</FieldLabel>
              <FieldDescription>超過這個大小的檔案不建立索引，避免一個很大的檔案佔掉太多時間。下次更新索引時生效。</FieldDescription>
              {mbError && <FieldError>{mbError}</FieldError>}
            </FieldContent>
            <InputGroup className="w-40">
              <InputGroupInput
                id="max-mb"
                inputMode="numeric"
                autoComplete="off"
                value={maxMb}
                disabled={savedMb === null}
                aria-invalid={!!mbError}
                onChange={(e) => setMaxMb(e.target.value)}
                onBlur={saveMb}
                onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
                className="tabular-nums"
              />
              <InputGroupAddon align="inline-end">
                <InputGroupText>MB</InputGroupText>
              </InputGroupAddon>
            </InputGroup>
          </Field>
        </FieldGroup>
      </Section>

      <Section title="外觀">
        <FieldGroup>
          <Field orientation="horizontal">
            <FieldContent>
              <FieldTitle>主題</FieldTitle>
              <FieldDescription>「跟隨系統」會依照 Windows 的淺色或深色設定。</FieldDescription>
            </FieldContent>
            {/* 單選 ToggleGroup 再點一次會變成空值，空值不理會 */}
            <ToggleGroup type="single" variant="outline" spacing={0} value={theme} onValueChange={(v) => v && setTheme(v as Theme)} aria-label="主題">
              {THEMES.map((item) => (
                <ToggleGroupItem key={item.value} value={item.value}>
                  {item.icon}
                  {item.label}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          </Field>
          <FieldSeparator />
          <Field orientation="horizontal">
            <FieldContent>
              <FieldTitle>文字大小</FieldTitle>
              <FieldDescription>整個畫面一起放大。也可以按住 Ctrl 轉動滑鼠滾輪縮放。</FieldDescription>
            </FieldContent>
            <ToggleGroup
              type="single"
              variant="outline"
              spacing={0}
              value={textSize}
              onValueChange={(v) => v && setTextSize(v as TextSize)}
              aria-label="文字大小"
            >
              {TEXT_SIZES.map((s) => (
                <ToggleGroupItem key={s.value} value={s.value}>
                  {s.label}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          </Field>
        </FieldGroup>
      </Section>

      <Section title="關於" description="所有文件、索引和模型都在這台電腦上執行，不需要網路，資料不會傳出去。">
        {!status ? (
          <Skeleton className="h-24" />
        ) : (
          <FieldGroup>
            <Field orientation="horizontal">
              <FieldContent className="min-w-0">
                <FieldTitle>資料位置</FieldTitle>
                <FieldDescription>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <span className="block truncate select-text">{shortenPath(status.data_dir)}</span>
                    </TooltipTrigger>
                    <TooltipContent className="max-w-md break-all">{status.data_dir}</TooltipContent>
                  </Tooltip>
                </FieldDescription>
              </FieldContent>
              <div className="flex shrink-0 items-center gap-2">
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button variant="ghost" size="icon" onClick={() => copyText(status.data_dir, "路徑")} aria-label="複製路徑">
                      <CopyIcon />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>複製路徑</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => openDataDir().catch((e) => toast.error("無法開啟資料夾", { description: errorText(e) }))}
                      aria-label="開啟資料夾"
                    >
                      <FolderOpenIcon />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>開啟資料夾</TooltipContent>
                </Tooltip>
              </div>
            </Field>
            <FieldSeparator />
            <Field orientation="horizontal">
              <FieldContent>
                <FieldTitle>查詢模型</FieldTitle>
                <FieldDescription>搜尋時自動載入，閒置一段時間就釋放記憶體。</FieldDescription>
              </FieldContent>
              <Availability ok={status.model_loaded} yes="已載入" no="沒有載入" />
            </Field>
            <FieldSeparator />
            <Field orientation="horizontal">
              <FieldContent>
                <FieldTitle>產生回答</FieldTitle>
                <FieldDescription>{status.answer_unavailable ?? "用搜尋結果整理出回答並標出出處。"}</FieldDescription>
              </FieldContent>
              <Availability ok={!status.answer_unavailable} yes="可以使用" no="不能使用" />
            </Field>
            <FieldSeparator />
            <Field orientation="horizontal">
              <FieldContent>
                <FieldTitle>頁面影像比對</FieldTitle>
                <FieldDescription>
                  {status.can_index_images ? "掃描的頁面也會以整頁影像比對。" : "找不到影像模型，掃描的頁面只用辨識出的文字搜尋。"}
                </FieldDescription>
              </FieldContent>
              <Availability ok={status.can_index_images} yes="可以使用" no="不能使用" />
            </Field>
            <FieldSeparator />
            <Field>
              <FieldTitle>快速鍵</FieldTitle>
              <Table>
                <TableBody>
                  {SHORTCUTS.map(([what, keys]) => (
                    <TableRow key={what}>
                      <TableCell className="text-muted-foreground">{what}</TableCell>
                      <TableCell className="text-right font-medium">{keys}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </Field>
            <FieldSeparator />
            <Field>
              <FieldTitle>字型授權</FieldTitle>
              <FieldDescription>介面使用 Geist 和思源黑體（Noto Sans TC），兩者都以 SIL Open Font License 1.1 授權。</FieldDescription>
            </Field>
          </FieldGroup>
        )}
      </Section>
    </Page>
  );
}
