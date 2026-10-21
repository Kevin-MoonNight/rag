// 和 Rust 端（src-tauri/src/main.rs）對應的指令與資料型別。
import { Channel, invoke } from "@tauri-apps/api/core";

export interface ResultItem {
  rank: number;
  /** 64 位元的編號，超過 JavaScript 數字能精確表示的範圍，所以用字串 */
  id: string;
  /** 這一筆是從哪一份名單來的 */
  source: "text" | "image" | "keyword";
  score: number;
  file_path: string;
  page: number;
  /** 段落跨頁時結尾所在的頁碼；不跨頁時和 page 相同 */
  page_end: number;
  is_image: boolean;
  heading: string | null;
  text: string | null;
  /** 段落裡要標示的關鍵字位置：[開始, 結束)，JavaScript 字串的位置 */
  highlights: [number, number][];
  image_path: string | null;
  same_content: string[];
}

export interface SearchResponse {
  results: ResultItem[];
  /** 搜尋花的時間（毫秒），含第一次載入模型 */
  milliseconds: number;
  /** 一次最多回幾筆；結果剛好這麼多時表示可能還有更多 */
  limit: number;
}

export interface IndexProgress {
  running: boolean;
  /** 這次更新是怎麼開始的 */
  reason: string | null;
  done: number;
  total: number;
  current: string | null;
  indexed: number;
  duplicates: number;
  unchanged: number;
  failed: number;
  /** 搬移或改名、只更新路徑的檔案 */
  moved: number;
  /** 已不存在、從索引拿掉的檔案 */
  removed: number;
  /** 找不到的來源（例如外接磁碟沒接上） */
  offline_sources: number;
  /** 換了模型、只重算向量的檔案 */
  reembedded: number;
  /** 不支援的檔案（最多 200 個） */
  unsupported_files: string[];
  /** 開始前的估計：要處理的檔案數、約略的文字段落數、秒數 */
  estimate_files: number;
  estimate_chunks: number;
  estimate_seconds: number;
  /** 建完之後的整理階段 */
  finishing: boolean;
  finished_at: string | null;
  /** 暫停中時的原因 */
  paused: string | null;
  /** 使用者按了暫停 */
  user_paused: boolean;
  /** 目前的執行方式（白天背景、夜間全速） */
  mode: string | null;
  error: string | null;
}

export interface ThrottleSettings {
  /** 夜間從幾點到幾點（夜間全速，白天背景執行） */
  night_start: number;
  night_end: number;
  /** 白天最多用幾個執行緒 */
  day_threads: number;
  cpu_pause: number;
  cpu_resume: number;
  memory_pause: number;
  memory_resume: number;
}

/** 使用者加入的資料夾或檔案 */
export interface SourceEntry {
  path: string;
  is_folder: boolean;
  /** 發現它不在的時間（Unix 秒） */
  offline_since: number | null;
}

export interface Schedule {
  on_startup: boolean;
  /** 每天幾點更新；null 是不每天更新 */
  daily_hour: number | null;
  throttle: ThrottleSettings;
}

export interface Status {
  files: number;
  indexed: number;
  failed: number;
  text_chunks: number;
  images: number;
  can_index_images: boolean;
  model_loaded: boolean;
  /** 不能產生回答時的原因 */
  answer_unavailable: string | null;
  data_dir: string;
  progress: IndexProgress;
}

/** 交給模型的一段資料；number 就是回答裡 [n] 的 n */
export interface AnswerSource {
  number: number;
  file_path: string;
  page: number;
  page_end: number;
  heading: string | null;
  text: string;
}

export type AnswerEvent = { kind: "sources"; sources: AnswerSource[] } | { kind: "text"; text: string };

export interface AnswerOutcome {
  sources: AnswerSource[];
  skipped_images: number;
  generation: {
    text: string;
    load_ms: number;
    first_text_ms: number;
    total_ms: number;
    finish_reason: string | null;
    cancelled: boolean;
  };
  cited: number[];
  not_found: boolean;
}

export const getStatus = () => invoke<Status>("status");
/** keywordOnly：只用關鍵字（不需要模型，馬上有結果） */
export const search = (query: string, keywordOnly = false) => invoke<SearchResponse>("search", { query, keywordOnly });
/** 用最近一次的搜尋結果產生回答；過程中的資料和文字經由 onEvent 送來 */
export const answer = (query: string, onEvent: (e: AnswerEvent) => void) => {
  const events = new Channel<AnswerEvent>();
  events.onmessage = onEvent;
  return invoke<AnswerOutcome>("answer", { query, events });
};
export const stopAnswer = () => invoke<void>("stop_answer");
/** 在背景預先載入模型（answer：回答用的模型，否則查詢用的），馬上回傳 */
export const preload = (answer: boolean) => invoke<void>("preload", { answer });
export const getSources = () => invoke<SourceEntry[]>("sources");
/** 加入資料夾或檔案，接著在背景更新索引 */
export const addSource = (path: string) => invoke<void>("add_source", { path });
/** 取消來源，回傳從索引拿掉的檔案數 */
export const removeSource = (path: string) => invoke<number>("remove_source", { path });
export const updateNow = () => invoke<void>("update_now");
export const stopIndex = () => invoke<void>("stop_index");
/** 暫停或繼續建索引（暫停時模型會卸載，記憶體還給使用者） */
export const pauseIndex = (paused: boolean) => invoke<void>("pause_index", { paused });
export const getSchedule = () => invoke<Schedule>("get_schedule");
export const setSchedule = (schedule: Schedule) => invoke<void>("set_schedule", { schedule });
export const getMaxFileMb = () => invoke<number>("get_max_file_mb");
export const setMaxFileMb = (mb: number) => invoke<void>("set_max_file_mb", { mb });
/** 用系統預設的程式開啟檔案（只能開索引裡的檔案） */
export const openFile = (path: string) => invoke<void>("open_file", { path });
/** 頁面圖片讀成可以直接放進 <img> 的 data URL */
export const pageImage = (path: string) => invoke<string>("page_image", { path });
/** 在檔案總管中顯示檔案（只能是索引裡的檔案） */
export const revealFile = (path: string) => invoke<void>("reveal_file", { path });
/** 用檔案總管打開程式的資料資料夾 */
export const openDataDir = () => invoke<void>("open_data_dir");
