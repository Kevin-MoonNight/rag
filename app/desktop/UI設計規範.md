# 桌面版 UI 設計規範（v4）

做法：**先定標準，再把功能放進標準的樣板**。數值只能從下面的標準來；標準沒有的數值不能自己發明。
需要例外時登記在附錄 B 並說明原因。
v2 經過第 1 輪審查（`work/ui-review/review1.md`，含對比計算程式 `contrast.js`）修正成 v3，
v3 經過第 2 輪審查（`work/ui-review/review2.md`）和實作截圖（`work/shot/`）修正成這一版。

## 0. 採用的標準

| 標準 | 管什麼 | 來源 |
|---|---|---|
| **shadcn/ui**（radix-nova 風格，CLI 4.22） | 元件的尺寸、內距、圓角、狀態、變體；版面樣板 | `src/components/ui/*`（CLI 產生，不手改）；shadcn-ui/ui GitHub 的 blocks（sidebar-07、dashboard-01）、Field 範例（field-switch、field-responsive）、cards/forms |
| **shadcn 官方 Blue 主題** | 顏色 token | shadcn-ui/ui `apps/v4/registry/_legacy-base-colors.ts` 的 `baseColorsV4.blue`（存在 `work/shadcn-themes/`）；對比不合格的 5 個 token 換成同色系 Tailwind 色階（§1.1） |
| **Tailwind CSS v4 預設尺度** | 間距、字級、字重、動畫時間；新增顏色用 Tailwind 調色盤 | tailwindcss 4.3 `theme.css` |
| **WCAG 2.2 AA** | 文字對比 ≥ 4.5:1、圖形與狀態 ≥ 3:1、點擊目標 ≥ 24px、鍵盤焦點可見 | W3C |
| **W3C 中文排版需求（clreq）** | 中文段落行距 | W3C |

不採用 Windows／Fluent 樣式（使用者要網頁式的好看介面）。Fluent 調查留在 `work/fluent-tokens/` 當參考。

**最重要的一條：用 shadcn 元件和它的變體組畫面；不自己用 div 拼元件、不覆寫元件內部的尺寸。**
shadcn 元件做不到的，先找 shadcn 的 block／範例；都沒有才自己寫，登記在附錄 B。
下面「只用某些值」的規定都只管**自己寫的 class**；元件內建的不動。

## 1. Token

### 1.1 顏色

shadcn Blue 主題原值，以下 5 個依對比驗算調整（全部仍在 Tailwind 調色盤內）：

| token | 淺色 | 深色 | 調整原因（對比，門檻） |
|---|---|---|---|
| `primary` | blue-700 | **blue-600**（原 blue-700） | 深色 Switch 開啟 2.60→3.37（3） |
| `primary-foreground` | **white**（原 blue-50） | blue-50 | 淺色主色按鈕 hover 4.21→4.59（4.5） |
| `destructive` | **red-700**（原 red-600） | red-400 | 淺色 destructive 按鈕（紅字＋紅 10% 底）3.99→5.33（4.5） |
| `ring` | **neutral-500**（原 neutral-400） | neutral-500 | 淺色焦點邊框 2.59→4.73（3） |
| `sidebar-ring` | **neutral-500**（原 neutral-400） | **neutral-500**（原 neutral-600） | 側欄項目的焦點只有 2px 光圈、沒有邊框幫忙：2.48／2.27→4.53／3.75（3） |

其餘照原值：`background` 白／zinc-950、`foreground` zinc-950／zinc-50、`card`/`popover` 白／zinc-900、`secondary`/`muted`/`accent` zinc-100／zinc-800、`muted-foreground` zinc-500／zinc-400、`border`/`input` zinc-200／white 10%·15%、`sidebar` zinc-50／zinc-900、`sidebar-primary` blue-600／blue-500、`radius` 0.625rem。

主題沒有、畫面需要的顏色（Tailwind 調色盤，固定階）：

| token | 淺色 | 深色 | 用途 |
|---|---|---|---|
| `brand` | blue-700 | blue-400 | 連結、可點的文字、引用編號、品牌圖示 |
| `brand-subtle` | blue-50 | blue-950 | 保留，目前不用（AI 回答卡標頭改成 `text-brand` 圖示，見 §3） |
| `success` | emerald-700 | emerald-400 | 成功狀態的圖示和短字 |
| `warning` | amber-700 | amber-400 | 警告狀態的圖示和短字 |
| `highlight` / `highlight-foreground` | yellow-200 / yellow-950 | yellow-400 25% / yellow-100 | 關鍵字標示 `<mark>` |

使用規則：
- **主色按鈕一個畫面最多一顆。** 搜尋頁的「搜尋」按鈕放在搜尋框裡、用 `secondary`（按 Enter 才是主要操作）；主色留給「產生回答」和空白狀態的主要動作。
- 「選取中」的開關按鈕（例如「只用關鍵字」開啟）：`secondary` 底＋`text-brand`；關閉時 `ghost`。`InputGroupButton` 沒有 `aria-pressed` 的樣式，要自己換 variant。
- **連結和可點的文字一律 `text-brand`**；不用 `Button variant="link"`／`Badge variant="link"`（寫死 `text-primary`，深色只有 2.6）。
- **顏色只表達「品牌（可點、選取）」和「狀態」。** 分類（結果的比對方式、來源類型）用中性的 `Badge variant="secondary"／"outline"`。
- **狀態色只用在圖示和短字**（Badge 裡的字、Alert 標題、6px 狀態圓點），不塗大塊底色。
- `muted-foreground` 不能放在實色 `muted` 或 `brand-subtle` 底上（淺色 4.39／4.43）；那兩種底上的次要文字用 `secondary-foreground`。`CardFooter` 內建的 `bg-muted/50` 底可以（淺色 4.63）。
- `Kbd` 只放在 Tooltip 裡（單獨放在畫面上淺色只有 4.39）；快速鍵清單用一般文字。
- `sidebar-primary` 底上只放圖示（深色放文字 3.45）。
- 不用漸層。

### 1.2 已知的對比例外

- 裝飾性的線（`border`、Card 外框、Separator、清單框）不要求 3:1（WCAG 1.4.11 只管辨識元件需要的視覺資訊）。
- 輸入框框線（`input`）約 1.3–1.6：所以**每個輸入都要有可見標籤，或前置圖示＋placeholder**；焦點時 `border-ring` ≥ 3:1。
- Switch 關閉的軌道：Switch 一定和 `FieldLabel` 同一列；開啟時的顏色合格。
- 停用狀態不要求對比（WCAG 例外）。
- **Progress 旁一律附文字（已處理 / 全部、百分比）**——深色進度條對軌道只有 2.83。
- 客戶若要做無障礙稽核，把 `--input` 改 zinc-500 即可，只改 token 不改元件。

### 1.3 字型

```css
--font-sans: 'Geist Variable', 'Noto Sans TC Variable', 'Microsoft JhengHei UI', 'Microsoft JhengHei', sans-serif;
```

- 英數 Geist（radix-nova 預設，176 KB）、中文思源黑體可變版（`@fontsource-variable/noto-sans-tc`，108 塊、4.2 MB，OFL-1.1，授權清單要列）。都打包，不連網。
- Geist 在前（英數、檔名、數字較精緻）；微軟正黑體不當主要字型，只放最後補罕用字（擴充 B 的人名用字）。
- 已知風險：思源黑體沒有 hinting，Windows 100% 縮放下 14px 中文可能偏淡。**要在目標 AI PC 上 100% 和 125% 縮放各截圖確認**；太糊的話把正黑體移到 Noto 前面（只改這一行）。
- 標點：引號用「」『』，刪節號用「……」，破折號用「──」（避免落到 Geist 的窄西文符號）。
- 不用 `tracking-tight`（負字距讓中文擠在一起）；元件內建的（EmptyTitle）不改。
- 大段中文最多 `font-medium`（500）。

### 1.4 字級

| 角色 | Tailwind | 大小/行高 |
|---|---|---|
| 頁首的頁面名稱 | `text-base font-medium` | 16/24 |
| 搜尋頁歡迎標題（例外 E9）、統計數字 | `text-2xl font-semibold` | 24/32 |
| 卡片標題 | `CardTitle`（內建） | 16/22；`Card size="sm"` 14 |
| 介面文字（預設） | `text-sm` | 14/20 |
| 強調的介面文字（檔名、清單標題） | `text-sm font-medium` | 14/20 |
| 輔助文字 | `text-sm text-muted-foreground` | 14/20 |
| **閱讀文字**（段落、AI 回答） | `text-sm leading-6` | 14/24 |
| 很小的標記 | `text-xs` | 12/16 |

- **中文內容最小 `text-sm`（14px）。** `text-xs` 只用在 Badge、Kbd、時間、數字、側欄群組標題（內建）。
- 閱讀文字行高 24（1.71 倍）：clreq 建議中文段落 1.5–2 倍。欄寬上限 `max-w-2xl`（裸放約 48 字、卡片內約 45 字）。
- 按鈕盡量用預設尺寸；`sm`（字 12.8px）只給次要動作。
- **文字大小設定**：標準 16px／大 17px／特大 18px（改 `<html>` 的 font-size；shadcn 和 Tailwind 都用 rem，整體放大）。自己寫的版面不用 px 寫死寬高（1px 線除外）。Ctrl＋滾輪縮放保留。

### 1.5 間距（gap、padding、margin；只管自己寫的版面）

寬高（`w-40`、`max-h-96` 這類）不在此限，照 §1.6 和各樣板。

只用 Tailwind 的 `1 1.5 2 3 4 6 8 12`（4、6、8、12、16、24、32、48 px）。

| 情境 | 值 | 依據 |
|---|---|---|
| 元件內部 | 不動 | shadcn |
| 圖示和字、標籤之間 | `gap-1.5`／`gap-2` | shadcn 元件慣例 |
| 按鈕和按鈕 | `gap-2` | shadcn 範例 |
| 卡片之間 | `gap-4` | dashboard-01 |
| 搜尋結果卡之間 | `gap-3` | 審查建議：結果多時密度要高 |
| 頁面區段之間 | `gap-4`，內容區 ≥ 48rem 時 `gap-6` | dashboard-01 `gap-4 md:gap-6`（改成容器查詢） |
| 頁面內距 | `px-4 py-4`，內容區 ≥ 48rem 時 `px-6 py-6` | dashboard-01 |
| `ItemGroup` 裡的項目 | 內建 `gap-4`，不另外寫 | shadcn |

### 1.6 尺寸、視窗與斷點

| 項目 | 值 |
|---|---|
| Button／Input／Select／Toggle | 預設 `h-8`（32），`sm` 28，`lg` 36 |
| 圖示按鈕 | 預設 `icon`（32，觸控一體機也好點）；`icon-xs`（24）只用在輸入框內的清除鈕 |
| 搜尋框 | `InputGroup className="h-10"`（例外 E1）；「搜尋」按鈕放在框內（`InputGroupButton variant="secondary" size="sm"`），不和框比高度 |
| 側欄 | 展開 16rem、收合 3rem（內建） |
| 頁首 | `h-12`（dashboard-01 的 `--header-height`）、`border-b` |
| **視窗最小** | **1024 × 640**（`tauri.conf.json`）：shadcn Sidebar 在 < 768 會變抽屜、Input 會變 16px |
| 視窗預設 | 開啟時最大化（`maximized: true`）：1240 寬時內容區只有約 968px，到不了兩欄的 64rem |
| 版面切換 | **容器查詢**：內容區加 `@container/main`，用 `@3xl/main:`（48rem）、`@5xl/main:`（64rem）；不用 `md:`／`lg:`（側欄展開／收合時內容寬差 13rem） |
| 內容寬 | 一般頁 `max-w-5xl` 置中；設定頁 `max-w-2xl`；搜尋頁不設上限 |

### 1.7 圓角、陰影、動畫、焦點

全部用元件內建的，不另外加。（radius 0.625rem → sm 6、md 8、lg 10、xl 14；控制項 `rounded-lg`、卡片 `rounded-xl`；Card 用 ring 不用陰影；浮出層內建 `shadow-md`。）
自己寫的轉場用 `transition-colors`（150ms）。`prefers-reduced-motion` 時：回答游標改成不閃的實心方塊。
焦點用內建 `focus-visible:ring-3 ring-ring/50` ＋ `border-ring`（光暈是裝飾，1px 邊框負責 3:1）。

### 1.8 圖示

lucide-react（shadcn 預設），大小跟元件走。**只有圖示的按鈕一定有 `aria-label` 和 Tooltip**（Tooltip 裡放 Kbd 快速鍵）。

### 1.9 長文字（銀行／醫院的路徑常常 80 字以上）

- 檔名：單行 `truncate`（外層 `min-w-0`），滑過 Tooltip 顯示全名；不換行、不縮字。
- 路徑：前段省略，保留磁碟代號和最後兩層，例如「D:\…\授信規章\113年」（`shortenPath()`，例外 E4）；全名放 Tooltip。
- 檔名、路徑、段落、AI 回答都加 `select-text`（全域 `user-select: none`，像桌面程式一樣介面字不會被拖選）。
- 結果卡的段落最多 4 行（`line-clamp-4`），「顯示整段」展開。
- 每筆結果的「更多」選單固定有：開啟檔案、在檔案總管中顯示、複製檔案路徑、複製段落文字。
- 隱私：側欄、頁首、Toast、Alert 不顯示完整路徑和文件內文（路徑常含員工姓名）。

## 2. 版面樣板

### T1 外框：sidebar-07 的側欄 ＋ dashboard-01 的內容區

- `SidebarProvider` → `AppSidebar collapsible="icon" variant="inset"`（dashboard-01 的 inset：內容區是浮在側欄底色上的圓角面板）＋ `SidebarInset`。
- 側欄頁首：`SidebarMenuButton size="lg"`：`size-8 rounded-lg bg-sidebar-primary` 標誌（只放圖示）＋程式名稱（`font-semibold`）＋副標（`text-xs`）。sidebar-07 TeamSwitcher 的排法，不做下拉。
- 側欄內容：一個 `SidebarGroup`：搜尋、搜尋範圍、索引。數量用 `SidebarMenuBadge` 只放數字；要表達狀態時數字前加 6px 狀態圓點（`size-1.5 rounded-full bg-warning`），不用彩色字。
- 側欄底部（dashboard-01 NavSecondary 的位置）：設定 ＋ 索引狀態小卡；收合時小卡換成一個狀態圖示按鈕＋Tooltip。
- 頁首：`h-12 border-b`，`SidebarTrigger -ml-1`＋`Separator` 垂直 `h-4`＋`<h1 class="text-base font-medium">頁面名稱</h1>`，右邊 ghost 圖示按鈕（主題）。不用麵包屑（只有一層）。
- 捲動骨架：`SidebarProvider h-svh` → `SidebarInset min-h-0 overflow-hidden`（flex 直欄）→ 頁首 `shrink-0` → 內容區 `@container/main relative min-h-0 flex-1` → 每頁 `absolute inset-0` 自己的捲動區。頁首永遠不捲走。
- 每頁只有一個捲動區（頁首下面）；唯一例外是搜尋頁的寬版兩欄（E14）。
- 切換頁面時其他頁只是藏起來（`hidden`），搜尋結果和回答都保留。

### T2 一般頁面（dashboard-01 的內容區）

```
<div class="@container/main">
  <div class="mx-auto flex max-w-5xl flex-col gap-4 px-4 py-4 @3xl/main:gap-6 @3xl/main:px-6 @3xl/main:py-6">
    第一列：說明文字（text-sm text-muted-foreground）｜右邊動作按鈕（gap-2）
    內容：Card，Card 之間 gap-4
```

頁面名稱只在頁首出現一次，內容區**不再放大標題**。

### T3 設定頁（Field 範例 field-switch、field-responsive ＋ cards/forms）

- 一欄、`max-w-2xl`；每個區段一張 `Card`：`CardHeader`（`CardTitle`＋`CardDescription`）＋`CardContent` 裡 `FieldGroup`（Card 已有標題，不再放 FieldLegend）。
- 每個設定 `Field orientation="horizontal"`：左 `FieldContent`（`FieldLabel`＋`FieldDescription`），右控制項；設定之間 `FieldSeparator`。
- 右邊控制項寬度固定（Select `w-40`），右緣對齊。
- 控制項：開關 `Switch`；從選項選一個 `Select`；兩三個選項而且要一眼看到全部的（主題、文字大小）`ToggleGroup variant="outline"`；數字 `InputGroup`＋`InputGroupAddon` 單位。時間一律 Select（不用 `<input type="time">`，WebView2 會畫成 Windows 原生樣式）。
- 改了就生效、**成功不跳 toast**，失敗才 toast；數字在失焦或 Enter 時套用，錯誤用 `FieldError` 顯示在欄位下。
- 最下面「關於」卡：資料位置（複製、開啟資料夾）、模型狀態、快速鍵清單、字型授權。

### T4 清單

- 來源清單：`ItemGroup`＋`Item variant="outline"`：`ItemMedia variant="icon"`（radix-nova 無框，接受）、`ItemContent`（`ItemTitle` 名稱、`ItemDescription` 路徑）、`ItemActions`（ghost 圖示按鈕）。
- 搜尋結果：每筆 `Card size="sm"`：`CardHeader`（`CardTitle` 檔名、`CardDescription` 路徑、`CardAction` 動作）＋`CardContent`（Badge 列、段落）。
- 長清單（不支援的檔案）：`ScrollArea` `max-h-96` 裡放 `Table`。

### T5 統計（dashboard-01 SectionCards 的結構，不用它的漸層和陰影）

`Card`：`CardHeader`（`CardDescription` 名稱、`CardTitle className="text-2xl font-semibold tabular-nums"` 數字、可選 `CardAction` Badge）＋可選 `CardFooter` 說明。排列 `grid gap-4 @xl/main:grid-cols-2 @5xl/main:grid-cols-4`。

### T6 回饋

| 情況 | 元件 |
|---|---|
| 要使用者知道或處理的持續狀態（找不到來源、索引錯誤、不能產生回答） | `Alert`（default／destructive）；警告是 default＋圖示 `text-warning!`（例外 E3） |
| 暫時的狀態（模型載入中、搜尋中） | 一行 `text-sm text-muted-foreground`＋`Spinner`，不用 Alert |
| 一次性的結果（已加入、已移除、已複製、存檔失敗） | sonner toast，`bottom-center`（不擋右側回答欄） |
| 會刪資料的動作 | `AlertDialog`，按鈕寫出動作（「移除」不寫「確定」），`variant="destructive"` |
| 能算進度的等待 | `Progress`＋文字 |
| 內容載入中 | `Skeleton`（形狀和內容一樣） |
| 按鈕處理中 | 按鈕內 `Spinner`、停用。**按鈕不因為「還沒輸入」停用**（停用的主色按鈕像壞掉）；空的時候按了就把焦點移回輸入框 |
| 沒有內容 | `Empty`：`EmptyMedia variant="icon"`＋`EmptyTitle`＋`EmptyDescription`＋最多一個主要動作 |
| 整個後端錯誤（讀不到狀態） | 全頁 `Empty`＋destructive 圖示＋「重試」；詳細錯誤放 `Collapsible` |
| 補充說明 | `Tooltip`；需要多行內容、要能用鍵盤／觸控打開的（引用出處）用 `Popover` |

### T7 文字與格式

- 繁體中文（台灣）、全形標點。按鈕用動詞開頭。錯誤說明發生什麼、可以怎麼做。
- 數字千分位 1,221；統計數字 `tabular-nums`。日期 2026/10/21、時間 24 小時制 14:05。頁碼「第 3 頁」「第 66–67 頁」。
- 快速鍵：Ctrl+K 或 / 聚焦搜尋框、Esc 清除、Enter 搜尋、Ctrl+Enter 產生回答、Ctrl+B 收合側欄（內建）。

## 3. 功能放進樣板

| 功能 | 頁面 | 樣板／元件 |
|---|---|---|
| 搜尋列 | 搜尋 | 內容區最上方、固定不捲（`border-b`）：`InputGroup h-10`（前置放大鏡／Spinner＋清除 `InputGroupButton icon-xs`＋「只用關鍵字」`InputGroupButton` 帶 `aria-pressed`（開啟時 secondary＋text-brand）＋垂直 Separator＋框內 secondary「搜尋」） |
| 搜尋框不落地 | 搜尋 | 不包 `<form>`；`autoComplete="off"`、`spellCheck={false}`；Rust 啟動時關掉 WebView2 的一般自動填入和密碼儲存（E15）。**要在目標 PC 的 WebView2 使用者資料夾確認一次沒有留下輸入值** |
| 還沒搜尋 | 搜尋 | 歡迎標題（E9）＋說明＋用法提示；「剛才搜尋過」`Button variant="outline" size="sm"`＋ghost「清除」，只存在記憶體（關掉程式就清掉——病歷號、客戶編號不落地，隱私優先於審查建議的「存本機」） |
| 沒有任何索引 | 搜尋 | `Empty`＋主色「加入資料夾」 |
| 搜尋中 | 搜尋 | 3 張 `Skeleton` 結果卡 |
| 模型載入中先給關鍵字結果 | 搜尋 | Tabs 那一列右邊：一行 muted 文字＋Spinner |
| 結果分類 | 搜尋 | `Tabs`：全部／文件／頁面影像，`TabsTrigger` 裡 `Badge variant="secondary"` 數量；0 筆的停用 |
| 搜尋結果 | 搜尋 | T4 結果卡：檔名、路徑（§1.9）、`Badge variant="outline"` 頁碼和比對方式、段落（閱讀文字、4 行）、「更多」選單 |
| 找不到結果 | 搜尋 | `Empty`＋建議；只用關鍵字時 outline「改用一般搜尋」 |
| 結果很多 | 搜尋 | 後端一次回固定筆數（`SearchResponse.limit`，目前 20），**不做「載入更多」**。剛好到上限時要明講：Tabs「全部」寫「20+」、右邊寫「只列出最相關的 20 筆」、清單最後一行建議把問題寫具體 |
| 頁面影像 | 搜尋 | 縮圖按鈕 → `Dialog`（`sm:max-w-4xl`，E2）＋`DialogFooter` outline「開啟原始檔案」 |
| AI 回答 | 搜尋 | `Card`：`CardHeader border-b`（`CardTitle` 前放 `text-brand` 的 Sparkles 圖示，不塗底色——Card 有上內距，塗了上方會多一條白邊）、`CardContent`（閱讀文字；引用編號 `Badge variant="outline"` 小方標 → `Popover` 出處＋「開啟檔案」）、參考資料清單（每筆也是 Popover；沒引用的變淡＋「沒有引用」）、`CardFooter`（時間、免責） |
| 回答的狀態 | 搜尋 | 未開始：主色「產生回答」；產生中：outline「停止」；完成：ghost 圖示「複製」＋outline「重新回答」；失敗：卡內 `Alert variant="destructive"`；不能用：卡內 `Alert` |
| 寬視窗 | 搜尋 | 內容區 ≥ 64rem（`@5xl/main`）兩欄、各自 `overflow-y-auto`（E14）：結果 `flex-1 min-w-0`、回答 `basis-2/5 max-w-xl shrink-0`；不到就一個捲動區、回答在結果上面。（`sticky` 在 flex 橫列裡黏不住，第 2 輪審查） |
| 加入資料夾／檔案 | 搜尋範圍 | 第一列右邊：主色「加入資料夾」、outline「加入檔案」；選擇對話框是系統原生（E8） |
| 沒有任何來源 | 搜尋範圍 | `Empty`＋主色「加入資料夾」 |
| 來源清單 | 搜尋範圍 | T4 `Item`；找不到的來源 `Badge variant="outline"`＋warning 圓點＋「10/3 起找不到」。每個來源的檔案數、上次更新時間後端沒有，先不顯示 |
| 有來源找不到 | 搜尋範圍 | `Alert`（警告）。「重新指定位置」後端沒有，先不做 |
| 移除來源 | 搜尋範圍 | ghost 圖示按鈕 → `AlertDialog` |
| 索引統計 | 索引 | T5 四張卡 |
| 建立中 | 索引 | `Card`：`CardAction` outline「暫停／繼續」「停止」；`CardContent`：「已處理 812 / 1,221（66%）· 估計約 45 分鐘」＋`Progress`＋目前檔名（§1.9）。「停止」不跳確認：已完成的檔案會保留、下次接著做，不會丟資料。兩顆都有 Tooltip 說差別：暫停＝停下來並釋放記憶體，按「繼續」接著做；停止＝結束這次更新，下次更新從沒做完的接著做 |
| 立即更新 | 索引 | 第一列右邊主色；建立中時不顯示 |
| 上次更新結果 | 索引 | `Card`：`CardHeader`（標題、時間）＋內容裡 6 個數字（`dl` 格，E5） |
| 不支援的檔案 | 索引 | `Card`＋`Collapsible` → `ScrollArea max-h-96`＋`Table`＋「複製清單」 |
| 自動更新、電腦資源、檔案上限 | 設定 | T3 |
| 主題、文字大小 | 設定（外觀卡）＋頁首主題選單 | `ToggleGroup variant="outline" spacing={0}`（接成一組）；單選再點一次會變空值，空值不理會。文字大小存本機（`text-size`），開程式時在畫面出來前先套用 |
| 資料位置、模型狀態、快速鍵 | 設定（關於卡） | T3 |
| 索引狀態摘要 | 側欄底部 | T1 |
| 讀不到後端 | 全部 | T6 全頁錯誤 |

## 附錄 A：對比驗算

程式 `work/ui-review/contrast.js`，完整結果 `work/ui-review/contrast-result.md`、`review1.md` 附錄 A。調整後（§1.1）的重點：

| 組合 | 淺色 | 深色 |
|---|---|---|
| foreground／background | 19.89 | 19.05 |
| muted-foreground／background、card | 4.83 | 7.56／6.74 |
| primary-foreground／primary | 6.82（hover 4.59） | 4.82 |
| brand／background、card | 6.82 | 7.54／6.72 |
| destructive 按鈕（淡底） | 5.33 | 5.35 |
| success／card | 5.37 | 9.17 |
| warning／card | 5.05 | 10.33 |
| mark | 12.52 | 8.96 |
| Switch 開啟／card（3:1） | 6.82 | 3.37 |
| 焦點 border-ring／background（3:1） | 4.73 | 4.20 |
| Progress／軌道（3:1） | 6.20 | 2.83（附文字，§1.2） |
| sidebar-ring 焦點光圈（3:1） | 4.53 | 3.75 |
| muted-foreground／CardFooter（`bg-muted/50`） | 4.63 | ≥ 6 |
| destructive 按鈕／card（深色） | ── | 4.58（剛好過） |

**改任何色彩 token 都要重跑 `node work/ui-review/contrast.js`。**

## 附錄 B：自己寫的、或例外

| # | 項目 | 原因 |
|---|---|---|
| E1 | 搜尋框 `InputGroup className="h-10"` | 頁面最主要的輸入 |
| E2 | 頁面影像 `DialogContent className="sm:max-w-4xl"` | 預設 384px 放不下頁面 |
| E3 | 警告 Alert 圖示 `text-warning!` | 元件的 CSS 權重蓋過一般 class |
| E4 | `shortenPath()` 路徑前段省略 | shadcn 沒有 |
| E5 | 上次更新結果的 `dl` 數字格 | shadcn 沒有 |
| E6 | `<mark>` 樣式＋`forced-colors` 時加外框 | shadcn 沒有；Windows 高對比模式會拿掉底色 |
| E7 | 回答串流的游標（減少動畫時不閃） | shadcn 沒有 |
| E8 | 選資料夾／檔案用系統原生對話框 | Tauri 原生，沒辦法套網頁樣式 |
| E9 | 搜尋頁歡迎標題 `text-2xl font-semibold` | 第一眼的畫面，比 Empty 的 14px 標題好看 |
| E10 | 全域 `user-select: none`＋內容 `select-text` | 桌面程式慣例：拖曳時介面字不會被反白 |
| E11 | 引用編號小方標（`Badge variant="outline"` 縮成 `h-5 min-w-5 px-1`） | 回答文字裡要放得下 |
| E12 | 列印樣式（隱藏側欄、引用列成註腳） | 之後再做 |
| E13 | 頁面影像 Dialog `max-h-[calc(100svh-2rem)]`，圖片區自己捲動 | 頁面比視窗高 |
| E14 | 搜尋頁寬版兩欄各自捲動 | 結果很長時回答要一直看得到；sticky 在 flex 橫列不生效 |
| E15 | Rust 關掉 WebView2 一般自動填入和密碼儲存（`ICoreWebView2Settings4`） | 病歷號、客戶編號不能被瀏覽器記到使用者資料夾 |
| E16 | 上次更新的 6 個數字用 `text-base font-medium`（E5 的數字格） | 比統計卡的 `text-2xl` 次要 |
| E17 | 側欄底部索引狀態小卡（`button`，`border bg-background`） | sidebar-07 的 NavUser 位置；shadcn 沒有狀態卡元件 |
