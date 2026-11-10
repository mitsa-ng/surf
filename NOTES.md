# Surf — Working Notes

Milestone 1（全骨架＋可執行垂直切片）的狀態真相檔。context 會丟失，這裡才是進度依據。

## 目標

1. 四層架構（UI / Rust core / Web view / Tauri shell）以可編譯的 workspace 骨架落地，模組邊界照架構圖。
2. 桌面垂直切片：tab strip + address bar + WKWebView，能真的開網頁、切換/關閉 tab。
3. Android / iOS 完成 scaffold 並可 build。
4. `npm test` / `npm run lint` 對齊全域 AGENTS.md 的 test commands 慣例。

## 驗收條件

- [x] `cargo test --workspace` 全綠（surf-core 15＋surf-ext-host 25）
- [x] `cargo clippy --workspace --all-targets -- -D warnings` 無錯
- [x] UI vitest 全綠（8）；svelte-check 0 errors 0 warnings
- [x] `pnpm dev`（tauri dev）：開窗 → chrome strip 渲染 → Cmd+L 輸入 example.com → IANA 頁面實際載入 → tab 標題 localhost→example.com 更新（截圖＋像素驗證）
- [x] iOS：`pnpm tauri ios build --target aarch64-sim` 產出 Surf.app
- [x] Android：`pnpm tauri android build --apk` 產出 app-universal-release-unsigned.apk（NDK 27）
- [x] surf-ext-host 不依賴 tauri（cargo tree 驗證：0 參照）
- [x] 架構自檢：ext-host 七模組（manifest/permission/runtime/api/router/planner/rules）＝架構圖七子系統；TabView＋RequestFilter trait、commands 全部就位（本工具無 subagent，逐層最小讀取核對）

## 進度

| 日期 | 事項 |
|---|---|
| 2026-11-10 | Bootstrap：repo init、DESIGN.md（方向定案）、AGENTS.md、workspace 骨架 |
| 2026-11-10 | surf-core＋surf-ext-host 全綠 commit；tauri app multiwebview manager；Svelte 5 UI |
| 2026-11-10 | 實機驗證修三案：window 重複建立 panic、macOS titlebar offset、chrome focus |
| 2026-11-11 | iOS Surf.app＋Android APK 皆產出；M1 驗收清單完成 |

## 已知風險 / 待辦

- **rquickjs-sys 無 mobile bindings**：以 `.cargo/config.toml` 的 `[env] BINDGEN_EXTRA_CLANG_ARGS_<target>` 提供真 triple＋sysroot。SDK/NDK 路徑是機器相關的（Xcode 27.2 / NDK 27.0.12077973），換機器或升級 toolchain 要改。
- Xcode script phase 不繼承 shell env → bindgen 參數只能放 cargo config，不能靠環境變數。
- WKContentRuleList 只能 block；redirect/header rules 在 Apple 平台降級（`FilterCapabilities` 已表達）。
- 行動端 tab 為 instance-swap 模式（M2）；M1 行動端 commands 一律回 M2 提示。
- `frontendDist` 是 `../../ui/dist`（相對 src-tauri）。
- 導航用 `location.replace` 模擬 load；`TabView::add_init_script` 佔位（M2 injection planner 前置）。
