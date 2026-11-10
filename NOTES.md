# Surf — Working Notes

Milestone 1（全骨架＋可執行垂直切片）的狀態真相檔。context 會丟失，這裡才是進度依據。

## 目標

1. 四層架構（UI / Rust core / Web view / Tauri shell）以可編譯的 workspace 骨架落地，模組邊界照架構圖。
2. 桌面垂直切片：tab strip + address bar + WKWebView，能真的開網頁、切換/關閉 tab。
3. Android / iOS 完成 scaffold 並可 build（行動端 multiwebview 為 M2）。
4. `npm test` / `npm run lint` 對齊全域 AGENTS.md 的 test commands 慣例。

## 驗收條件

- [ ] `cargo test --workspace` 全綠
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 無錯
- [ ] UI vitest 全綠；lint（svelte-check / biome）無錯
- [ ] `pnpm dev`（tauri dev）：開窗 → chrome strip 渲染 → 輸入 URL 載入頁面 → tab 新增/切換/關閉 → address bar 反映導航 → back/forward/reload 可用
- [ ] Android：`tauri android build` 產出 APK（NDK 需先補裝）
- [ ] iOS：`tauri ios build` 通過（或明確記錄失敗原因）
- [ ] surf-ext-host 不依賴 tauri（cargo tree 驗證）
- [ ] fresh-context subagent 獨立核對：實作與架構圖逐層一致

## 進度

| 日期 | 事項 |
|---|---|
| 2026-11-10 | Bootstrap：repo init、DESIGN.md（方向定案）、AGENTS.md、workspace 骨架開工 |

## 已知風險 / 待辦

- Android NDK 缺失 → 需 `sdkmanager "ndk;27.*"`（Tauri 2 需求版本以官網為準）後才能 android build。
- WKContentRuleList 只能 block，redirect/header rules 在 Apple 平台降級（capability flags 已在 rules 模組表達）。
- 行動端 tab 為 instance-swap 模式（Tauri mobile 無單窗 multiwebview），M1 只 scaffold。
