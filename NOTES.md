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

| 日期 | 事項 |
|---|---|
| 2026-11-10 | M1：骨架＋桌面垂直切片＋mobile build 產出 |
| 2026-11-11 | M2：extension engine 端到端驗證（banner/hits=1/popup）；ACL 關鍵修復（見下）；mobile tab model 雙平台驗證 |
| 2026-11-11 | 修復：rquickjs promise pump、api bridge 位置、grant race、deep-link config 格式、viewport-fit=cover、iOS dvh |

## 關鍵教訓（踩雷紀錄）

- **Tauri ACL：app 有 permissions manifest 後，所有自訂 commands 都要明確 allow**。加入 content-bridge.toml 後 tab_open 等被靜默拒絕（遠端被拒會回錯誤，local 則直接不執行）；修復＝permissions/app-commands.toml 列全表＋capability 引用。
- rquickjs 的 promise 需手動 pump：`Runtime::execute_pending_job()`，否則 async 擴充程式碼永遠停在第一個 await。
- Xcode script phase 不繼承 shell env → bindgen 參數放 `.cargo/config.toml` `[env]`。
- Android WebView 的 `env(safe-area-inset-bottom)` 恆為 0（即使 viewport-fit=cover）→ 需要 CSS 常數 fallback，否則按鈕被手勢列蓋住。
- Android WebView 109 + swiftshader 有 tile memory 警告；截圖前先 HOME→重開強制重繪。
- `.cargo/config.toml` 的 SDK/NDK 路徑是機器相關（Xcode 27.2 / NDK 27.0.12077973），換機要改。

## 已知限制（誠實標明）

- 行動端 tab＝model 層＋per-tab WebviewWindow instance-swap（程式碼完成、雙 target 編譯過）；模擬器上的 runtime 切換驗證受阻於 swiftshader 的 tap 自動化（EGL 座標漂移），需實機測試。
- `cs_send` 的 token 在頁面 context 可讀——頁面可為「自己的 tab」偽造訊息；真正 isolated world 需平台 API。
- iOS/Android 的 request-filter adapter 仍是 NotImplemented stub（`FilterCapabilities` 已表達差異）；macOS 導航層 block 已可用。
- WKContentRuleList 整合（subresource blocking）＝M3。
- Android APK：release keystore 在 `~/surf-release.keystore`（alias `surf`，密碼 surf-release-2026）；`zipalign`＋`apksigner verify` 流程已驗證。
- sync/crypto：檔案匯出/匯入（書籤＋歷史，去重合併）已完成；裝置間自動同步仍待做。
- Desktop release build：`target/release/surf-app` 已產出（bundle.active=false，只有 binary）。
