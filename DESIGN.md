# Surf — Design Direction

作者是產品負責人（使用者本人）。以下「方向」區塊為負責人 2026-11-10 的逐字回答， agent 僅排版，未增刪語意。其後的「Tokens」區塊是把方向轉為可實作數值的推導，供 UI 層使用。

## 方向（負責人原话）

- **產品個性**：安静極簡 —— chrome 極輕、退到內容後面，幾乎感覺不到它的存在（Arc / Safari 式）。
- **主題**：跟隨系統 light/dark，兩套都要顧。
- **配色**：灰階＋海藍 accent（呼應 Surf 命名），只用在關鍵焦點。
- **字體與密度**：系統字＋舒適密度。

## Tokens（由方向推導）

| Token | 值 | 依據 |
|---|---|---|
| 基底色（light） | 近白暖灰 `#f6f5f4`／chrome 面板 `#ecebe9` | 灰階基底、安静 |
| 基底色（dark） | 近黑 `#1d1d1f`／chrome 面板 `#26262a` | 同上，跟隨系統 |
| 文字 | light `#1d1d1f` / dark `#f5f5f7`；次層 `opacity .55` | 中性對比 |
| Accent | light `#0a7fa8` / dark `#4fb3d9`（海藍） | 只用於：active tab 指示、address bar focus ring。其餘一律灰階 |
| 分隔線 | `opacity 8%` 的前景色 | 極簡、無陰影無 glow |
| 字體 | `-apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI", Roboto, sans-serif` | 系統字 |
| 密度 | toolbar 44px 高、tab strip 36px、圓角 6px（accent 與 pill 類一律不用） | 舒適、非緊湊 |
| Motion | 只允許 active-tab 切換的 150ms ease 與 focus 過渡；無 loop 動畫 | 安静 |

Dials：ENERGY 1 / RHYTHM 1 / MOTION 1。
