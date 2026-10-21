import { toast } from "sonner";
import { openFile, revealFile } from "@/api";
import { errorText } from "@/lib/format";

/** 用系統預設的程式開啟檔案，失敗時跳出提示 */
export function openPath(path: string) {
  openFile(path).catch((e) => toast.error("無法開啟檔案", { description: errorText(e) }));
}

/** 在檔案總管中顯示檔案；檔案已經搬走或刪掉時跳出提示 */
export function revealPath(path: string) {
  revealFile(path).catch((e) => toast.error("無法在檔案總管中顯示", { description: errorText(e) }));
}

export function copyText(text: string, what = "內容") {
  navigator.clipboard
    .writeText(text)
    .then(() => toast.success(`已複製${what}`))
    .catch((e) => toast.error("無法複製", { description: errorText(e) }));
}
