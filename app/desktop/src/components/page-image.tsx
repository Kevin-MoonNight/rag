import { useEffect, useState } from "react";
import { ExternalLinkIcon, ImageOffIcon } from "lucide-react";
import { pageImage } from "@/api";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

/** 讀過的頁面影像留著，切換分頁、重新顯示時不用再讀一次（最多 40 張） */
const cache = new Map<string, string>();

function usePageImage(path: string) {
  const [src, setSrc] = useState<string | null>(() => cache.get(path) ?? null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (cache.has(path)) {
      setSrc(cache.get(path)!);
      return;
    }
    let alive = true;
    setSrc(null);
    setFailed(false);
    pageImage(path)
      .then((s) => {
        if (cache.size >= 40) cache.delete(cache.keys().next().value!);
        cache.set(path, s);
        if (alive) setSrc(s);
      })
      .catch(() => alive && setFailed(true));
    return () => {
      alive = false;
    };
  }, [path]);
  return { src, failed };
}

/** 掃描頁或圖片檔的頁面影像：先顯示縮圖，點一下用 Dialog 放大（規範 E2） */
export function PageImage({
  path,
  title,
  description,
  onOpenFile,
}: {
  path: string;
  title: string;
  description: string;
  onOpenFile: () => void;
}) {
  const { src, failed } = usePageImage(path);
  const [zoom, setZoom] = useState(false);

  if (failed)
    return (
      <p className="flex items-center gap-2 text-sm text-muted-foreground">
        <ImageOffIcon className="size-4 shrink-0" />
        頁面影像讀不到，請開啟原始檔案。
      </p>
    );
  if (!src) return <Skeleton className="h-64 w-48 rounded-lg" />;

  return (
    <>
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            onClick={() => setZoom(true)}
            aria-label="放大頁面"
            className="block w-fit overflow-hidden rounded-lg border bg-white transition-colors outline-none hover:border-ring focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
          >
            <img src={src} alt="" className="max-h-64 w-auto max-w-full object-contain object-top" />
          </button>
        </TooltipTrigger>
        <TooltipContent>放大頁面</TooltipContent>
      </Tooltip>
      <Dialog open={zoom} onOpenChange={setZoom}>
        <DialogContent className="flex max-h-[calc(100svh-2rem)] flex-col sm:max-w-4xl">
          <DialogHeader>
            <DialogTitle className="truncate pr-8">{title}</DialogTitle>
            <DialogDescription>{description}</DialogDescription>
          </DialogHeader>
          <div className="min-h-0 flex-1 overflow-auto rounded-lg border bg-white">
            <img src={src} alt={`${title} ${description}的頁面影像`} className="mx-auto w-full" />
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={onOpenFile}>
              <ExternalLinkIcon data-icon="inline-start" />
              開啟原始檔案
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
