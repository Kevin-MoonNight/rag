import type { ResultItem } from "@/api";

/** 秒數寫成「約 N 分鐘／小時」；超過 10 小時再換算成晚上數（以每晚 10 小時計） */
export function formatDuration(seconds: number): string {
  if (seconds < 90) return `${Math.max(1, Math.round(seconds))} 秒`;
  if (seconds < 5400) return `${Math.round(seconds / 60)} 分鐘`;
  const hours = seconds / 3600;
  const text = `${hours.toFixed(1)} 小時`;
  return hours > 10 ? `${text}（約 ${Math.ceil(hours / 10)} 個晚上）` : text;
}

export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** 檔案所在的資料夾 */
export function folderOf(path: string): string {
  const at = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return at > 0 ? path.slice(0, at) : "";
}

/** 頁碼；段落跨頁時寫成「66–67」 */
export function pages(page: number, pageEnd: number): string {
  return pageEnd > page ? `${page}–${pageEnd}` : String(page);
}

export function seconds(ms: number): string {
  return `${(ms / 1000).toFixed(1)} 秒`;
}

export function isImageFile(path: string): boolean {
  return /\.(png|jpe?g)$/i.test(path);
}

/** 段落本身就是從標題開始的（切分時標題放在段落開頭），就不用再另外顯示一次標題 */
export function startsWithHeading(item: ResultItem): boolean {
  const squeeze = (s: string) => s.replace(/\s/g, "");
  return !!item.heading && !!item.text && squeeze(item.text).startsWith(squeeze(item.heading));
}

/** 路徑太長時省略中間：保留磁碟代號和最後兩層，例如「D:\…\授信規章\113年」（規範 1.9、E4） */
export function shortenPath(path: string): string {
  const sep = path.includes("\\") ? "\\" : "/";
  const unc = path.startsWith("\\\\");
  const parts = path.split(/[\\/]/).filter(Boolean);
  // 網路路徑的「\\伺服器\共用」整段當成開頭
  const head = unc ? `\\\\${parts.slice(0, 2).join("\\")}` : parts[0];
  const rest = unc ? parts.slice(2) : parts.slice(1);
  if (rest.length <= 3) return path;
  return [head, "…", ...rest.slice(-2)].join(sep);
}

/** 數字加千分位：1,221 */
export function num(n: number): string {
  return n.toLocaleString("en-US");
}

const pad = (n: number) => String(n).padStart(2, "0");

/** 日期寫成 2026/10/21（規範 T7）；可以傳 Unix 秒或 Date */
export function date(value: number | Date): string {
  const d = typeof value === "number" ? new Date(value * 1000) : value;
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())}`;
}

/** 日期和 24 小時制時間：2026/10/21 14:05 */
export function dateTime(value: number | Date): string {
  const d = typeof value === "number" ? new Date(value * 1000) : value;
  return `${date(d)} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
