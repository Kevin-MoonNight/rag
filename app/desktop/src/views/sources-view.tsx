// 規範 §3 搜尋範圍：T2 頁面＋T4 Item 清單
import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { FilePlusIcon, FileTextIcon, FolderIcon, FolderPlusIcon, HardDriveIcon, InfoIcon, Trash2Icon, TriangleAlertIcon, UnplugIcon } from "lucide-react";
import { addSource, removeSource, SourceEntry } from "@/api";
import { Page, PageIntro } from "@/components/page-header";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogMedia,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemMedia, ItemTitle } from "@/components/ui/item";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { date, errorText, fileName, shortenPath } from "@/lib/format";

function SourceItem({ source, canRemove, onRemove }: { source: SourceEntry; canRemove: boolean; onRemove: () => void }) {
  const name = fileName(source.path) || source.path;
  return (
    <Item variant="outline">
      <ItemMedia variant="icon">
        {source.offline_since ? (
          <UnplugIcon className="text-warning" />
        ) : source.is_folder ? (
          <FolderIcon />
        ) : (
          <FileTextIcon />
        )}
      </ItemMedia>
      <ItemContent className="min-w-0">
        <ItemTitle className="flex min-w-0 items-center gap-2">
          <span className="min-w-0 truncate select-text">{name}</span>
          <Badge variant="secondary">{source.is_folder ? "資料夾" : "檔案"}</Badge>
          {source.offline_since && (
            <Badge variant="outline">
              <span aria-hidden className="size-1.5 rounded-full bg-warning" />
              {date(source.offline_since)} 起找不到
            </Badge>
          )}
        </ItemTitle>
        <ItemDescription>
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="block truncate select-text">{shortenPath(source.path)}</span>
            </TooltipTrigger>
            <TooltipContent className="max-w-md break-all">{source.path}</TooltipContent>
          </Tooltip>
        </ItemDescription>
      </ItemContent>
      <ItemActions>
        <Tooltip>
          <TooltipTrigger asChild>
            {/* 停用的按鈕收不到滑鼠事件，外面包一層讓 Tooltip 還能說明原因 */}
            <span tabIndex={canRemove ? -1 : 0}>
              <Button variant="ghost" size="icon" disabled={!canRemove} onClick={onRemove} aria-label={`移除「${name}」`}>
                <Trash2Icon />
              </Button>
            </span>
          </TooltipTrigger>
          <TooltipContent>{canRemove ? "不再搜尋這個來源" : "建立索引時不能移除"}</TooltipContent>
        </Tooltip>
      </ItemActions>
    </Item>
  );
}

export function SourcesView({
  sources,
  error,
  running,
  onChanged,
}: {
  sources: SourceEntry[] | null;
  error: string | null;
  running: boolean;
  /** 加入或移除之後：重新讀來源和索引狀態 */
  onChanged: () => void;
}) {
  const [removing, setRemoving] = useState<SourceEntry | null>(null);
  const [working, setWorking] = useState<"folder" | "file" | "remove" | null>(null);

  const add = async (directory: boolean) => {
    const picked = await open({
      directory,
      multiple: true,
      title: directory ? "選擇要搜尋的資料夾" : "選擇要搜尋的檔案",
      filters: directory ? undefined : [{ name: "PDF 和圖片", extensions: ["pdf", "png", "jpg", "jpeg"] }],
    });
    const paths = typeof picked === "string" ? [picked] : (picked ?? []);
    if (paths.length === 0) return;
    setWorking(directory ? "folder" : "file");
    let added = 0;
    try {
      for (const p of paths) {
        await addSource(p);
        added++;
      }
      toast.success(`已加入 ${added} 個${directory ? "資料夾" : "檔案"}`, { description: "正在背景建立索引，建好的檔案就可以搜尋。" });
    } catch (e) {
      toast.error(added > 0 ? `已加入 ${added} 個，其餘的加入失敗` : "加入失敗", { description: errorText(e) });
    } finally {
      setWorking(null);
      onChanged();
    }
  };

  const remove = async (entry: SourceEntry) => {
    setWorking("remove");
    try {
      const removed = await removeSource(entry.path);
      toast.success(`已移除「${fileName(entry.path) || entry.path}」`, {
        description: removed > 0 ? `從索引拿掉 ${removed} 個檔案，檔案本身沒有動。` : "檔案本身沒有動。",
      });
    } catch (e) {
      toast.error("移除失敗", { description: errorText(e) });
    } finally {
      setWorking(null);
      setRemoving(null);
      onChanged();
    }
  };

  const offline = sources?.filter((s) => s.offline_since).length ?? 0;
  const actions = (
    <>
      <Button variant="outline" onClick={() => add(false)} disabled={!!working}>
        {working === "file" ? <Spinner data-icon="inline-start" /> : <FilePlusIcon data-icon="inline-start" />}
        加入檔案
      </Button>
      <Button onClick={() => add(true)} disabled={!!working}>
        {working === "folder" ? <Spinner data-icon="inline-start" /> : <FolderPlusIcon data-icon="inline-start" />}
        加入資料夾
      </Button>
    </>
  );

  return (
    <Page>
      <PageIntro
        description="加入的資料夾（含子資料夾）和檔案會建立索引。支援 PDF 和圖片檔（PNG、JPG），掃描的頁面會自動辨識文字。"
        actions={sources && sources.length > 0 ? actions : null}
      />

      {error && (
        <Alert variant="destructive">
          <TriangleAlertIcon />
          <AlertTitle>讀不到搜尋範圍</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      {offline > 0 && (
        <Alert>
          <UnplugIcon className="text-warning!" />
          <AlertTitle>有 {offline} 個來源找不到</AlertTitle>
          <AlertDescription>
            可能是外接磁碟或網路磁碟沒接上。接上之後到「索引」按「立即更新」，或等下次自動更新就會恢復；超過 30 天都找不到，才會從索引拿掉。
          </AlertDescription>
        </Alert>
      )}

      {running && sources && sources.length > 0 && (
        <Alert>
          <InfoIcon />
          <AlertDescription>正在建立索引，完成或停止之後才能移除來源；這時候仍然可以加入新的來源。</AlertDescription>
        </Alert>
      )}

      {sources === null ? (
        <ItemGroup>
          {[0, 1, 2].map((i) => (
            <Item key={i} variant="outline">
              <ItemMedia variant="icon">
                <Skeleton className="size-4" />
              </ItemMedia>
              <ItemContent>
                <Skeleton className="h-4 w-1/3" />
                <Skeleton className="h-4 w-1/2" />
              </ItemContent>
            </Item>
          ))}
        </ItemGroup>
      ) : sources.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HardDriveIcon />
            </EmptyMedia>
            <EmptyTitle>還沒有加入任何資料夾</EmptyTitle>
            <EmptyDescription>加入要搜尋的資料夾或檔案。資料只會在這台電腦上處理，不會上傳。</EmptyDescription>
          </EmptyHeader>
          <EmptyContent className="flex-row justify-center">{actions}</EmptyContent>
        </Empty>
      ) : (
        <ItemGroup>
          {sources.map((s) => (
            <SourceItem key={s.path} source={s} canRemove={!running && !working} onRemove={() => setRemoving(s)} />
          ))}
        </ItemGroup>
      )}

      <AlertDialog open={!!removing} onOpenChange={(o) => !o && !working && setRemoving(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogMedia className="bg-destructive/10 text-destructive">
              <Trash2Icon />
            </AlertDialogMedia>
            <AlertDialogTitle>不再搜尋「{removing && (fileName(removing.path) || removing.path)}」？</AlertDialogTitle>
            <AlertDialogDescription>
              <span className="block break-all select-text">{removing?.path}</span>
              索引裡它的資料會拿掉，檔案本身不會動。之後想搜尋可以再加入。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={!!working}>取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={!!working}
              onClick={(e) => {
                e.preventDefault();
                if (removing) remove(removing);
              }}
            >
              {working === "remove" && <Spinner data-icon="inline-start" />}
              移除
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Page>
  );
}
