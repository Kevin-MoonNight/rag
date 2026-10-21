// 只在開發時（vite 開發伺服器、不在 Tauri 裡）載入：用假的後端資料預覽介面。正式版不會打包進去。
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { AnswerEvent, IndexProgress, ResultItem, Schedule, SourceEntry, Status } from "@/api";

const params = new URLSearchParams(location.search);
const empty = params.has("empty");
let running = params.has("running");
let paused = false;
let done = 37;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

let sources: SourceEntry[] = empty
  ? []
  : [
      { path: "D:\\共用\\授信規章", is_folder: true, offline_since: null },
      { path: "D:\\共用\\作業手冊", is_folder: true, offline_since: null },
      { path: "E:\\外接備份\\歷年公文", is_folder: true, offline_since: 1760000000 },
      { path: "C:\\Users\\王小明\\Desktop\\個金會議紀錄.pdf", is_folder: false, offline_since: null },
    ];

let schedule: Schedule = {
  on_startup: true,
  daily_hour: 12,
  throttle: { night_start: 19, night_end: 7, day_threads: 2, cpu_pause: 85, cpu_resume: 60, memory_pause: 90, memory_resume: 80 },
};
let maxMb = 200;
let modelLoaded = false;

function progress(): IndexProgress {
  return {
    running,
    reason: running ? "啟動時更新" : "手動更新",
    done,
    total: 128,
    current: running ? "D:\\共用\\授信規章\\消費者貸款作業準則（113年修正）.pdf" : null,
    indexed: 12,
    duplicates: 3,
    unchanged: 1204,
    failed: 1,
    moved: 2,
    removed: 0,
    offline_sources: 1,
    reembedded: 0,
    unsupported_files: ["D:\\共用\\作業手冊\\舊版\\流程圖.vsd", "D:\\共用\\作業手冊\\附件\\表單.docx", "D:\\共用\\作業手冊\\附件\\範本.xlsx"],
    estimate_files: running ? 128 : 0,
    estimate_chunks: 5400,
    estimate_seconds: 2700,
    finishing: false,
    finished_at: running ? null : "2026/10/21 12:04",
    paused: null,
    user_paused: paused,
    mode: "白天背景執行（最多 2 個執行緒）",
    error: null,
  };
}

function status(): Status {
  if (running && !paused && done < 128) done++;
  return {
    files: empty ? 0 : 1225,
    indexed: empty ? 0 : 1221,
    failed: empty ? 0 : 1,
    text_chunks: empty ? 0 : 48213,
    images: empty ? 0 : 312,
    can_index_images: true,
    model_loaded: modelLoaded,
    answer_unavailable: params.has("noanswer") ? "記憶體不足（可用 3.1 GB，需要 6 GB）" : null,
    data_dir: "C:\\Users\\王小明\\AppData\\Local\\local.rag.desktop",
    progress: progress(),
  };
}

const text1 =
  "第十二條 提前清償違約金\n借款人於貸款期間提前清償者，提前清償違約金之計收方式，應符合公平合理原則，以「貸款餘額」或「提前償還金額（不包含正常攤還部分）」為計收基礎，並考慮時間因素，計收之比率採逐年遞減。";
const results: ResultItem[] = [
  {
    rank: 1,
    id: "1",
    source: "text",
    score: 0.82,
    file_path: "D:\\共用\\授信規章\\個人購屋貸款作業要點.pdf",
    page: 12,
    page_end: 13,
    is_image: false,
    heading: "第三章 還款及提前清償",
    text: text1,
    highlights: [
      [8, 15],
      [31, 38],
    ],
    image_path: null,
    same_content: ["D:\\共用\\作業手冊\\附件\\個人購屋貸款作業要點（副本）.pdf"],
  },
  {
    rank: 2,
    id: "2",
    source: "keyword",
    score: 0.61,
    file_path: "C:\\Users\\王小明\\Desktop\\個金會議紀錄.pdf",
    page: 3,
    page_end: 3,
    is_image: false,
    heading: "參、討論事項",
    text:
      "案由二：個人購屋貸款提前清償違約金計收方式案。\n決議：目前實務上多已回歸以提前還款金額之精神辦理計收違約金，不宜以「原貸金額（本金）」為基礎計收，以符合公平、合理原則。倘其提前清償違約金計收方式經法遵單位確認確為對消費者較為有利，本會原則尊重其所採計收方式。\n其餘事項依原決議辦理，並請各分行於下月底前完成內部規章修訂，修訂後之版本送法遵單位備查。如有疑義，請洽個人金融部。\n附帶決議：請資訊單位配合調整系統計算邏輯，並於上線前完成測試。",
    highlights: [[9, 17]],
    image_path: null,
    same_content: [],
  },
  {
    rank: 3,
    id: "3",
    source: "image",
    score: 0.44,
    file_path: "E:\\外接備份\\歷年公文\\109年\\金管會函-違約金.pdf",
    page: 1,
    page_end: 1,
    is_image: true,
    heading: null,
    text: null,
    highlights: [],
    image_path: "page.png",
    same_content: [],
  },
  {
    rank: 4,
    id: "4",
    source: "text",
    score: 0.4,
    file_path: "D:\\共用\\作業手冊\\消費金融業務問答集.pdf",
    page: 48,
    page_end: 48,
    is_image: false,
    heading: "Q32 客戶提前還款是否一定要收違約金？",
    text: "Q32 客戶提前還款是否一定要收違約金？\n不一定。依契約約定，若借款人因死亡、重大傷病或遭受天災等不可抗力因素而提前清償，得免收提前清償違約金。",
    highlights: [],
    image_path: null,
    same_content: [],
  },
];

const answerText =
  "個人購屋貸款提前清償違約金的計收方式如下：\n\n1. 應以「貸款餘額」或「提前償還金額（不包含正常攤還部分）」為計收基礎，並考慮時間因素，計收比率逐年遞減 [1]。\n2. 不宜以「原貸金額（本金）」為基礎計收；若經法遵單位確認對消費者較有利，可尊重其計收方式 [2]。\n3. 借款人因死亡、重大傷病等不可抗力因素提前清償時，**得免收**違約金 [4]。";

// 假的頁面影像：一張畫了幾行字的 SVG
const pageSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="595" height="842"><rect width="100%" height="100%" fill="#fff"/><text x="297" y="90" font-size="26" text-anchor="middle" font-family="serif">金融監督管理委員會　函</text>${Array.from(
  { length: 22 },
  (_, i) => `<rect x="70" y="${150 + i * 28}" width="${380 + ((i * 53) % 80)}" height="9" fill="#bbb"/>`,
).join("")}</svg>`;

let stopAnswer = false;

mockWindows("main");
mockIPC(async (cmd, args) => {
  const a = (args ?? {}) as Record<string, unknown>;
  switch (cmd) {
    case "status":
      return status();
    case "search": {
      await sleep(a.keywordOnly ? 120 : modelLoaded ? 300 : 1500);
      if (!a.keywordOnly) modelLoaded = true;
      const q = String(a.query);
      if (q.includes("沒有")) return { results: [], milliseconds: 210, limit: 20 };
      // ?many：結果剛好到上限 20 筆，檢查「只列出最相關的 20 筆」
      const list = params.has("many") ? Array.from({ length: 20 }, (_, i) => ({ ...results[i % 4], rank: i + 1, id: String(i + 1) })) : results;
      return { results: a.keywordOnly ? list.filter((r) => r.source === "keyword") : list, milliseconds: a.keywordOnly ? 120 : 340, limit: 20 };
    }
    case "answer": {
      stopAnswer = false;
      const channel = a.events as { onmessage: (e: AnswerEvent) => void };
      const answerSources = results
        .filter((r) => !r.is_image)
        .map((r, i) => ({ number: i < 2 ? i + 1 : i + 2, file_path: r.file_path, page: r.page, page_end: r.page_end, heading: r.heading, text: r.text! }));
      answerSources.splice(2, 0, { ...answerSources[0], number: 3, page: 14, page_end: 14, text: "第十三條 其他費用……" });
      await sleep(1200);
      channel.onmessage({ kind: "sources", sources: answerSources });
      let sent = "";
      for (const ch of answerText) {
        if (stopAnswer) break;
        await sleep(18);
        sent += ch;
        channel.onmessage({ kind: "text", text: ch });
      }
      return {
        sources: answerSources,
        skipped_images: 1,
        generation: { text: sent, load_ms: 4400, first_text_ms: 5600, total_ms: 9800, finish_reason: stopAnswer ? null : "stop", cancelled: stopAnswer },
        cited: [1, 2, 4],
        not_found: false,
      };
    }
    case "stop_answer":
      stopAnswer = true;
      return;
    case "preload":
      return;
    case "sources":
      return sources;
    case "add_source":
      sources = [...sources, { path: String(a.path), is_folder: true, offline_since: null }];
      running = true;
      done = 0;
      return;
    case "remove_source":
      sources = sources.filter((s) => s.path !== a.path);
      return 42;
    case "update_now":
      running = true;
      done = 0;
      return;
    case "stop_index":
      running = false;
      return;
    case "pause_index":
      paused = !!a.paused;
      return;
    case "get_schedule":
      return schedule;
    case "set_schedule":
      schedule = a.schedule as Schedule;
      return;
    case "get_max_file_mb":
      return maxMb;
    case "set_max_file_mb":
      maxMb = Number(a.mb);
      return;
    case "open_file":
      if (String(a.path).startsWith("E:")) throw "找不到檔案（磁碟沒接上？）";
      return;
    case "reveal_file":
      if (String(a.path).startsWith("E:")) throw "找不到檔案（磁碟沒接上？）";
      return;
    case "open_data_dir":
      return;
    case "page_image":
      return `data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(pageSvg)))}`;
    case "plugin:dialog|open":
      return ["D:\\共用\\新的資料夾"];
  }
  throw `沒有模擬的指令：${cmd}`;
});
