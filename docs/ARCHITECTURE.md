# Surf Architecture

```
┌──────────────────────────────────────────────────────┐
│ 1. UI layer (TypeScript, Svelte 5)                   │  shared
│    tab strip, address bar, menus, settings,          │
│    extension toolbar, install/permission prompts,    │
│    extension popup/options pages (small web views)   │
├──────────────────────────────────────────────────────┤
│ 2. Rust core (Cargo workspace)                       │  shared
│    ├─ surf-core: storage (SQLite), sync, crypto,     │
│    │        URL logic, tab suspension                │
│    └─ surf-ext-host (independent extension engine)   │
│         ├─ Loader / Registry (manifest, packages)    │
│         ├─ Permission Manager (API + host access)    │
│         ├─ Background Runtime (QuickJS, 1 per ext)   │
│         ├─ API Host: storage, tabs, bookmarks,       │
│         │            alarms, fetch                   │
│         ├─ Message Router (ext ↔ content ↔ popup)    │
│         ├─ Injection Planner (URL match → scripts)   │
│         └─ Rules Engine (adblock-rust + declarative  │
│                          block/redirect/header rules)│
├──────────────────────────────────────────────────────┤
│ 3. Web view layer (one per tab)                      │  platform-specific
│    WebView2 / WebKitGTK / WKWebView /                │
│    Android WebView (or CEF on desktop)               │
│    ├─ TabView trait: load, eval, add_init_script,    │
│    │                 post_message                    │
│    ├─ Content-script bridge (token-bound IPC)        │
│    └─ Request-filter adapter per platform:           │
│         WebView2 WebResourceRequested,               │
│         Android shouldInterceptRequest,              │
│         Apple WKContentRuleList, WebKitGTK filters   │
├──────────────────────────────────────────────────────┤
│ 4. Platform shell (Tauri 2)                          │  mostly shared
│    windows, keychain, deep links, downloads          │
└──────────────────────────────────────────────────────┘
```

## 模組落點（repo → 架構圖）

| 架構圖區塊 | Repo 位置 |
|---|---|
| UI layer | `apps/ui`（Svelte 5；desktop tab strip 與 mobile bottom bar 為兩種 deliberate layout） |
| core | `crates/surf-core`：`urlbox`（URL logic）、`storage`（SQLite）、`tabs`（tab model）、`suspension`/`crypto`/`sync`（M1 為 trait 邊界） |
| ext-host | `crates/surf-ext-host`：`manifest`、`permission`、`runtime`（rquickjs）、`api`、`router`、`planner`、`rules`。不得依賴 tauri |
| Web view layer | `apps/surf/src-tauri/src/tabview`（TabView trait＋Tauri multiwebview 實作）、`filters`（RequestFilter trait＋平台 capability） |
| Platform shell | `apps/surf/src-tauri`（Tauri 2；`gen/android`、`gen/ios` 為行動端 scaffold） |

## 關鍵決策

- **狀態真相在 Rust**：tab 清單、active tab、每 tab 的 URL/title/loading 由 `surf-core::tabs` 管理，UI 透過 Tauri event `tabs://changed` mirror。
- **桌面 multiwebview**：Tauri 2 單一 window 內多 child webview（chrome webview 固定頂部條、content webview 填滿其餘，active 顯示其餘隱藏，resize 時重排）。
- **行動端 tab = instance-swap**：Tauri mobile 無單窗 multiwebview，Android 走 Kotlin adapter 管理 N 個 WebView instance（M2），M1 僅 scaffold。
- **Request filter 能力差異**：Apple WKContentRuleList 僅支援 declarative block（無 redirect/header rewrite）；WebView2 WebResourceRequested 與 Android shouldInterceptRequest 支援較完整。差異以 `FilterCapabilities` 表達，rules engine 依能力降級而非失敗。
- **Extension engine 獨立**：surf-ext-host 只認 trait（storage、prompt），不認 Tauri 型別，未來可單獨測試/重用。

## M2 狀態（已實作）

- Extension engine 端到端：manifest→registry→背景 QuickJS（每擴充一條 OS thread＋promise pump）→權限→API Host（storage/tabs/bookmarks/alarms/fetch 真實實作）→router；content scripts 以 token bridge 注入（`cs_bootstrap`/`cs_send`，capability 僅開這兩個給遠端頁）；`ext://` scheme 服務擴充檔案；popup 視窗。
- 桌面全功能：tabs、導航、書籤、歷史、設定（搜尋引擎＋封鎖清單）、擴充管理面板、native menus、下載事件、deep links（surf://）、macOS keychain。
- 導航 adblock：NavFilter 專用執行緒（adblock Engine 非 Send）。
- 行動端：tab model 端到端（Android/iOS 驗證）；build 產出 APK／Surf.app。

## 範圍邊界（平台限制，誠實標明）

- 行動端 per-tab webview instance（隔離＋back stack）＝M3 原生 Kotlin/Swift 工作（Tauri mobile 無單窗 multiwebview）。
- WKContentRuleList subresource blocking、iOS/Android request-filter adapter ＝ M3。
- suspension（wry 無 API）、sync 傳輸層＝trait 就緒待 M3。
- content-script token 在頁面可讀（偽造限於自己的 tab）；isolated world 需平台 API。
