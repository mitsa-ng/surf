# Surf — Working Notes

Milestone 2（產品功能完備）的狀態真相檔。context 會丟失，這裡才是進度依據。

## 目標

1. 四層架構完整落地：extension engine 端到端可用（content scripts、背景 runtime、真實 API、popup、權限流程）。
2. 桌面完整瀏覽體驗：tabs、導航、書籤、歷史、設定（搜尋引擎＋封鎖清單）、擴充管理、下載事件、deep links、native menus、macOS keychain。
3. 行動端：tab model 端到端（Android/iOS），build 產出 APK/Surf.app。
4. `npm test` / `npm run lint` 全綠。

## 驗收條件（M2 完成時全勾）

- [x] Extension engine 端到端（tauri dev 實測）：manifest→registry→背景 QuickJS 執行緒→callApi→storage（log 顯示 `bg ready, hits = 1`）；content-script 注入（example.com 頁面藍色 banner 截圖）；ext:// scheme＋popup 視窗＋ext_api_call（popup 顯示 `storage hits：1` 截圖）
- [x] 桌面導航（Cmd+L → example.com 載入，截圖＋像素驗證）
- [x] 導航 adblock：NavFilter 專用執行緒（channel verdict），單元測試 block `||example.com^`／fail-open；設定面板可編輯規則
- [x] 行動端 Android：Pixel_6 模擬器實測 tab model（badge 0→1、sheet 顯示 tab row＋關閉鈕，截圖）
- [x] 行動端 iOS：iPhone 16 Pro 模擬器啟動＋BottomBar 完整渲染（dvh 修復，截圖）
- [x] `cargo test --workspace` 全綠（surf-core 16＋surf-ext-host 26＋surf-app 5）
- [x] `cargo clippy --workspace --all-targets -- -D warnings` 無錯；UI vitest 8＋svelte-check 0/0
- [x] keychain roundtrip（真 macOS keychain）實測通
- [x] surf-ext-host 不依賴 tauri（cargo tree 0 參照）

## 進度

### 本次目標：修復桌面工具列被瀏覽區域遮擋

- [x] macOS 使用不延伸到標題列下方的視窗內容區，避免 child WebView 座標與可視區錯位。
- [x] Chrome 固定 80 logical px；頁面從 y=80 開始，初始載入、resize 與新分頁使用同一套幾何。
- [x] Vite 將 `start.html` 獨立打包，空白分頁不再載入完整瀏覽器 UI。
- [x] Retina app 畫面確認工具列完整可見；網址列可點選並取得焦點。

| 日期 | 事項 |
|---|---|
| 2026-11-12 | 補齊前輪四項驗證限制（全部 live 實證，改走原生選單點擊＋CGEvent 通道避開 IME）：**find 計數**（Edit 選單「尋找…」＋「Paste」選單注入，查詢 "domain" 顯示計數 5、與頁面 5 處藍色反白完全吻合——find://count cs_send 回報端到端通）；**NTP**（原生選單「新分頁」開啟：中央搜尋 pill＋真歷史 top-sites tiles（E example.org/G gemini/C cloudflare/D MDN），ntp://ready→ntp://sites bridge 往返實證）；**分頁右鍵選單**（CGEvent 原地右鍵：選單錨定指標處、群組分隔線、danger 紅字、overlay 透明擴展下頁面可見）；**冷啟 deep link**（bundle Surf.app 經 LaunchServices 帶 `surf://open?url=` 啟動直達目標頁——修復：on_open_url 事件在 UI 掛載前到達會丟失，改為 emit＋佇列雙路，UI 掛載後單次 take；bundle binary 同步為最新 build）。附帶：原生 Edit 選單新增「尋找…」（Cmd+F，chrome 對齊）。驗證實體已清、release binary 重啟為常駐 |
| 2026-11-12 | 桌面 chrome UI/UX 對齊 Google Chrome 基準（決策：Chrome 管結構／DESIGN.md 管色彩，使用者三項擇一確認）。**幾何**：分頁列 36→40px（tab 34px、上留 6px、tab-search 鈕 x92、紅綠燈直接疊列＝`TitleBarStyle::Overlay`+`hidden_title`）、工具列 44→47px（omnibox 34px pill）、堆疊 80→87px；分頁等寬分配（flex-basis 0、max 240、pinned 改 icon-only 34px）。**狀態**：back/forward disabled——tauri 無 `can_go_back`，自建 Rust journal（`TabModel.history` 記錄 committed 導航、back/forward 樂觀移動 index，`TabState.can_back/can_forward` serde default）；reload↔stop；**15s loading 看門狗**（wry 無失敗訊號，DNS 死 URL 曾永久轉圈——實證卡點全清）。**識別**：favicon 首字母彩色 placeholder（網域 hash→低飽和色相、明暗變體，分頁/書籤列/建議/tab 搜尋共用；真 favicon 管線另輪）；工具列文字按鈕改圖示鈕（download/history/list/puzzle/settings 手繪 16px 單線寬）；書籤星移入 omnibox 欄內右側。**overlay**：建議下拉（40px 列、「Google 搜尋」首列、匹配子串粗體、selected 左緣 accent 條、圓角 16）、tab 選單鍵盤導航（↑↓/Esc、開啟進焦點、關閉還焦點、群組分隔線）、tab 搜尋錨定分頁搜尋鈕下、find bar 移 omnibox 下方＋匹配計數（FIND_SCRIPT 經 cs_send 攔截回報 `find://count`）。**NTP**：中央搜尋 pill＋真歷史 top-sites tiles（`ntp://ready`/`ntp://search` 走 cs_send 攔截，無歷史顯示提示不造假）。**其他**：`chrome-expanded` body 透明（overlay/panel 開啟時頁面保持可見）、冷啟 deep link 佇列（`launch_deeplink`+`deeplink_take`）、chrome.test.ts +4。實證：release app 截圖 light/dark（像素掃描：strip 40/toolbar 47/內容 87、omnibox 34、紅綠燈疊列、明暗高度結構皆對齊 Chrome 實測基準）、Cmd+L 聚焦全選、導航後 journal back 變可用、建議下拉含搜尋列與透明擴展下頁面可見、看門狗清卡。限制：find 計數未 live 實證（鍵盤注入通道受 IME 干擾，UI render＋單元測試通）；NTP 截圖未實證（同因，程式碼層通）；冷啟 argv deep link 在未 bundle binary 上 LaunchServices 不路由（佇列已備，bundle 生效待驗）；tab 選單右鍵未 live 點擊（單元測試通） |
| 2026-11-12 | 修復分頁右鍵選單與所有 chrome overlay 被內容 webview 遮擋：chrome-rendered overlays 會延伸到 80/108px chrome stack 下方、原生內容 webview 疊在其上；新增 `overlay_set_open` 指令＋`overlay_open` flag，relayout 在 panel／分頁選單／分頁搜尋／網址列建議開啟時把 chrome webview 擴展到全視窗（透明、reparent 到內容之上）——與 side panel 既有機制同一條路。release app 實證：六個 seeded tabs 下建議列表與分頁搜尋面板完整顯示在工具列線下方，修復機制由兩個 overlay 越過 chrome 顯示實證；右鍵選單走同路徑（未單獨截圖）。驗證期間歷史 0 筆為操作交錯狀態（探針作用分頁＝active tab、tabs 表顯示導航成功），非導航問題。注意：建議開啟時透明 chrome 蓋住頁面，第一次點頁面會先關閉建議 |
| 2026-11-12 | 分割畫面多工＋三項小功能：**分割畫面**（`split_set/split_clear`，layout.rs 兩個內容 webview 左右各半、其他隱藏；分頁右鍵「並排顯示」或 `surf://split`（作用分頁＋第一個非無痕分頁）；成員關閉或切到非成員分頁自動退出；不持久化（視圖狀態）——release app 截圖實證 split 兩頁並排、unsplit 回全幅無殘留）；`Cmd+1..9` 跳分頁；網址列建議（書籤★＋歷史↺各取前 6、鍵盤上下導航＋Enter）；分頁右鍵選單（複製網址/複製分頁/釘選/並排/關閉；剪貼簿用 navigator.clipboard）。右鍵選單與建議未做 runtime 點擊實證（不注入鍵盤），程式碼＋型別層通過 |
| 2026-11-12 | 釘選分頁＋重開分頁：`TabState.pinned`（serde default、storage v3 migration 加欄位、save/load roundtrip 測試）；釘選排序不變量（釘選群組恆在最左，pin/unpin 移動位置，模型測試）；TabStrip 星星釘選鈕＋釘選緊縮樣式＋隱藏關閉鈕；`tab_set_pinned`＋`tab_reopen_closed`（關閉堆疊上限 20、無痕不記錄、capture-before-close 單元測試）；`Cmd+Shift+T`／`surf://pin`／`surf://reopen`。實機 DB 實證：v3 對既有 DB 乾淨遷移（user_version 3、舊列保留）、pin→`pinned=1`、unpin→`pinned=0` |
| 2026-11-12 | 無痕分頁：`Cmd+Shift+N`／`surf://private` 開無痕 tab（TabState 帶 `incognito` 旗標、serde default 向下相容）；不寫歷史、不進持久化 session（restore 自然不含）；TabStrip「無痕」徽章＋網址列圓角 pill＋placeholder 標示。DB 對照實證：一般分頁導航有歷史紀錄（對照組=1）、無痕分頁兩次導航皆 0 筆、persisted tabs 不含無痕。限制：WebviewStorage/cookie 仍為共用 profile（真隔离需 wry 的 ephemeral profile 支援），徽章視覺只驗到型別層 |
| 2026-11-12 | Chrome 對齊第二批判（全部 release app 實證）：頁內搜尋（`Cmd+F` FindBar＋`find_in_page` 指令；`window.find` 在 WKWebView 無效，改用 TreeWalker＋Range 選取，實證藍底選取與捲動定位）；分頁搜尋（`Cmd+Shift+A` overlay，過濾＋點擊切換＋Enter 選第一個）；下載管理（`downloads_list`/`download_reveal` 指令＋Rust 端紀錄（上限 100 筆）＋`Cmd+Shift+J` 下載面板＋「顯示」在 Finder（僅列出的紀錄路徑可開），實證 cloudflare 下載進面板）。macOS 下載 Finished 的 path 可為空（儲存面板流程），「顯示」按鈕僅在有路徑且成功時出現 |
| 2026-11-12 | Chrome 對齊批次：書籤列（桌面，28px、極簡風；`Cmd+Shift+B` 切換、持久化 `setting:bookmarks_bar`、Rust `chrome_height` 動態幾何＋relayout；release app 實證開/關雙向、內容無覆蓋、持久化 1↔空）；清除瀏覽紀錄（`history_clear` 指令＋storage 測試＋歷史面板兩步確認按鈕，雙平台）；ACL 補兩個新指令。手機端書籤列刻意不做（BottomBar 佈局），清除歷史可用 |
| 2026-11-12 | vendor wry 0.57.2（`vendor/wry`＋`[patch.crates-io]`）：Android 多 webview 的 per-webview eval 路由修復（`webviews` map＋`ForWebview` 訊息），content webview eval 實證 by-id 正確送達；診斷出第二層問題——主視窗的 invoke 回應/事件 eval 在 tauri-runtime-wry 的 registry 查找處靜默丟棄（證據與下一步見誠實限制）；無實體裝置（instance-swap 實機驗證仍待裝置） |
| 2026-11-11 | M3 一批：mobile 導航過濾接線（on_navigation/on_page_load/title/initialization_script 進 ensure_tab_window）＋Android 模擬器實證（`||example.com^` 擋下、example.net 對照載入、bridge ready、歷史記錄；桌面 release app 也實證 blocked log＋對照組）；WKContentRuleList 整合（adblock crate `content-blocking` 轉換器＋objc2 膠），但發現 macOS 27.2 WebKit `addContentRuleList` segfault（純 Swift 可重現）→ 預設關閉＋env opt-in＋重現腳本入 repo；cs_send/cs_bootstrap 綁定呼叫端 webview label＋token 移出 `window.__surf`；Android deep link 修復（manifest intent-filter＋plugin mobile scheme）；桌面檔案自動同步（reconciler 雙向 merge，macOS 實證外部書籤併入＋本地回寫＋併回舊歷史）；補上 sync_export/import 漏掉的 ACL；發現 wry Android 多 webview eval 路由限制（見誠實限制） |
| 2026-11-11 | 修復設定持久化：點選搜尋引擎直接寫入 SQLite，修正回讀字串型別；release app 實測 Google 重啟後讀回，並恢復 DuckDuckGo；封鎖清單讀取成功後才可儲存 |
| 2026-11-11 | 分離桌面設定與擴充功能面板；設定頁僅保留搜尋、同步、封鎖規則，擴充按鈕開啟獨立安裝／管理頁；release app 畫面與無障礙樹驗收通過 |
| 2026-11-11 | 修復桌面浮動面板被瀏覽 WebView 遮擋：面板開啟時擴展透明 chrome、提升原生層級；release app 畫面確認面板完整浮在內容上方，關閉後恢復工具列與瀏覽區 |
| 2026-11-11 | 修復桌面工具列遮擋：macOS 改用 `TitleBarStyle::Transparent`，讓 WebView 座標原點從原生標題列下方開始；保留 80px chrome 與獨立 `start.html`；release app 畫面及網址列點擊驗收通過 |
| 2026-11-10 | M1：骨架＋桌面垂直切片＋mobile build 產出 |
| 2026-11-11 | M2：extension engine 端到端驗證（banner/hits=1/popup）；ACL 關鍵修復（見下）；mobile tab model 雙平台驗證 |
| 2026-11-11 | 修復：rquickjs promise pump、api bridge 位置、grant race、deep-link config 格式、viewport-fit=cover、iOS dvh |

## 關鍵教訓（踩雷紀錄）

- **Tauri ACL：app 有 permissions manifest 後，所有自訂 commands 都要明確 allow**。加入 content-bridge.toml 後 tab_open 等被靜默拒絕（遠端被拒會回錯誤，local 則直接不執行）；修復＝permissions/app-commands.toml 列全表＋capability 引用。sync_export/import 也踩過同坑（M3 只驗了 storage 測試層，按鈕實際被 ACL 擋）。
- WKContentRuleList 的 `url-filter` 是精簡 regex 子集：不支援非捕捉群組 `(?:...)` 與交替 `|`（「Disjunctions are not supported yet」）；一般捕捉群組可以。用 adblock crate 的 `content-blocking` 轉換器（記得 `NetworkFilter::parse(line, true, ..)`，否則 NeedsDebugMode）。
- Rust `eprintln!` 在 Android 不會進 logcat；命令錯誤要靠 JS 端 console（`Tauri/Console` tag）回傳才能看到。WebView console 在 debug build 經 CDP（`adb forward tcp:9222 localabstract:webview_devtools_remote_<pid>`）可直接驅動頁面 JS，是不用 tap 的自動化路徑。
- rquickjs 的 promise 需手動 pump：`Runtime::execute_pending_job()`，否則 async 擴充程式碼永遠停在第一個 await。
- Xcode script phase 不繼承 shell env → bindgen 參數放 `.cargo/config.toml` `[env]`。
- Android WebView 的 `env(safe-area-inset-bottom)` 恆為 0（即使 viewport-fit=cover）→ 需要 CSS 常數 fallback，否則按鈕被手勢列蓋住。
- Android WebView 109 + swiftshader 有 tile memory 警告；截圖前先 HOME→重開強制重繪。
- `.cargo/config.toml` 的 SDK/NDK 路徑是機器相關（Xcode 27.2 / NDK 27.0.12077973），換機要改。

## 已知限制（誠實標明）

- **wry Android 多 webview eval 路由（已本地修復一層，還有第二層）**：
  - **第一層（已修，vendor/wry）**：wry 0.57.2 在同 activity 上只保留「最後建立的 webview」的 handle，eval/load/getUrl 等全部路由到它。已 vendor wry 進 `vendor/wry`（`[patch.crates-io]`），改為 `ActivityProxy.webviews: HashMap<WebviewId, GlobalRef>`＋`ForWebview { webview_id, op }` 訊息精準路由。實證：content webview 的 eval（cs_bootstrap 回應、事件）以 by-id 正確送達（logcat 標記驗證）、`surfBridge=ready`。上游修好後刪 patch 即可。
  - **第二層（未解，卡在 tauri/tauri-runtime-wry）**：所有指向**主視窗**的 eval（invoke 回應、`emit_to` 事件、deep link 轉發）連 wry `InnerWebView::eval` 都沒進入（eval 頂端標記零輸出），但在 wry 修復前同一批 eval 會送到 content webview（`Couldn't find callback id`）。證據鏈：主視窗 invoke 的 fetch(ipc.localhost) 因 asset-loader 分支不被攔截→JS fallback postMessage→**命令有執行**（settings_set 寫入 DB 實證）→回應 eval 失蹤；deep link 事件 emit 同樣無 eval。最可疑處：tauri-runtime-wry `lib.rs` ~3644 `windows.0.borrow().get(&window_id)`→`webviews.iter().find(|w| w.id == webview_id)` 查找失敗時**靜默跳過**（`if let Some((Some(window), Some(webview)))` 無 else）。下一步：vendor tauri-runtime-wry，把該查找失敗顯性化（console/現有 webview eval 出錯誤），釐清主視窗的 (window_id, webview_id) 為何對不上。在這層解開前，Android 多 webview 下主 UI 的 invoke/事件仍不通；單一 webview（無分頁視窗）時全部正常。
  - iOS 無第一層機制問題（每個 WKWebView 獨立 handle），runtime 未測。
- 行動端 tab＝model 層＋per-tab WebviewWindow instance-swap（程式碼完成、雙 target 編譯過）；Android 受上述限制、實機驗證仍未做。
- `cs_send` token 已移出 `window`、指令綁定呼叫端 webview label（跨 tab token 無法重放）；殘餘限制：content script 與頁面同一 JS world，頁面仍可呼叫 `send()` 偽造「自己 tab」的訊息。真正 isolated world 需平台 API。
- Subresource 過濾（WKContentRuleList）：整合已完成（EasyList→content-blocker 轉換＋objc2 膠＋首次 page load 後安裝、規則變更 refresh），但 **macOS 27.2 的 WebKit 讓 `addContentRuleList:` 在 null proxy 上 segfault**——純 Swift 三種時序都可重現（`docs/wkcontentrulelist-crash-repro.swift`）。因此安裝預設關閉，`SURF_WK_CONTENT_RULES=1` opt-in，等 WebKit 修好再打開。轉換器單元測試覆蓋；導航層 block 不受影響（三平台可用）。iOS runtime 未測。Android shouldInterceptRequest／Windows WebResourceRequested／Linux WebKitUserContentFilter：wry 未暴露對應 hook，adapter 維持 NotImplemented。
- Android deep link `surf://` 已可達（manifest intent-filter＋plugin `mobile: [{scheme:[\"surf\"]}]`）；模擬器上事件送達已由 pm query-activities 與 intent delivery 證實，但 UI 端 navigateActive 受上述 eval 路由限制影響，Android 上完整鏈路待 wry 修復。
- Android APK：release keystore 在 `~/surf-release.keystore`（alias `surf`，密碼 surf-release-2026）；`zipalign`＋`apksigner verify` 流程已驗證。
- sync：檔案匯出/匯入＋**桌面自動同步**（選一個雲端資料夾內的 surf-sync.json，5 秒 reconciler 雙向去重合併）已完成並在 macOS dev 實證；行動端維持手動匯出/匯入。
- Desktop release build：`target/release/surf-app` 已產出（bundle.active=false，只有 binary）。
