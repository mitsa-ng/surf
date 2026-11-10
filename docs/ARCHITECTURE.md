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

## Milestone 1 範圍邊界

- 內容腳本注入（content-script bridge token IPC）、popup/options、API Host 真實實作、rules 下載 → M2。
- suspension/crypto/sync → trait＋stub。
- 行動端 multi-tab → M2。
