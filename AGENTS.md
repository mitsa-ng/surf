# AGENTS.md — Surf

全域制度見 `~/.agents/AGENTS.md`（五條核心規則＋playbooks 路由）。本檔僅放本專案事項。

## Test commands

npm test
npm run lint

## 專案慣例

- Cargo workspace：`crates/`（surf-core、surf-ext-host）＋`apps/surf/src-tauri`。surf-ext-host 不得依賴 tauri。
- Rust core 是 tab/狀態的真相來源；UI 靠 Tauri events mirror，不自行推導。
- UI 在 `apps/ui`（Svelte 5），desktop 與 mobile 是兩種 deliberate layout，不是 resize 的同一份。
- 設計決策以 `DESIGN.md` 為準（作者是使用者）。
- 工作進度記在 `NOTES.md`。

<!-- antislop:start -->
## antislop
For UI, copy, people, mobile layout, or code comments work, read these installed skill files directly (use these paths even if a same-named global skill exists):
- Core filter, always on: `antislop`: `/Users/mac/.agents/skills/antislop/SKILL.md`
- UI / visual: `antislop-ui`: `/Users/mac/.agents/skills/antislop-ui/SKILL.md`
- Copy & text: `antislop-copywriting`: `/Users/mac/.agents/skills/antislop-copywriting/SKILL.md`
- People: `antislop-human`: `/Users/mac/.agents/skills/antislop-human/SKILL.md`
- Mobile / responsive: `antislop-layoutmobile`: `/Users/mac/.agents/skills/antislop-layoutmobile/SKILL.md`
- Code comments: `antislop-code`: `/Users/mac/.agents/skills/antislop-code/SKILL.md`
Before starting, follow the core's "Two Usage Modes" section in strict order: explicit session instruction first, then global preference, then ask. A session instruction always wins. For a resolved mode, say `antislop active: <mode> (session override).` or `antislop active: <mode> (global preference).` once before presenting findings or making edits, using the actual mode and source. Acknowledging the user's request without naming the source does not replace this notice.
Only an explicit choice of antislop during or after selects a session mode. A request to review, audit, or avoid file edits does not select a mode; read the global preference in that case. Another skill's mode does not select antislop's mode.
If the mode is unresolved, ask during/after and end the response; wait for the answer before any UI review, planning, or concept. For read-only tasks, put the active-mode notice only at the start of the final answer, never in progress messages. For editing tasks, announce before the first edit and omit it from the final answer.
To update antislop later: `npx antislop-ai --update`, or run `npx antislop-ai` and pick Overwrite them.
<!-- antislop:end -->
