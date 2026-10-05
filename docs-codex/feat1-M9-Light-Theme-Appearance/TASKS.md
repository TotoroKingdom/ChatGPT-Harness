# M9 TASKS — Light Theme & Appearance

> Status: M9 — ACCEPTED / FROZEN
> Source of truth: `SPEC.md` → `PLAN.md` → this file  
> Milestone boundary: focused UI appearance feature, no full V3 acceptance
> Final acceptance: `FINAL_ACCEPTANCE.md`

## Execution rules

- 按任务顺序执行；每个 Task 完成代码、对应 unit/integration tests 后再进入下一个。
- 优先复用 M8 semantic tokens、shared primitives 与 config mutation queue。
- 不重新设计页面结构或交互。
- 除非发现现有 contract 无法满足需求，否则不得修改 `runtime/**`、`protocol/**`、`ui/src-tauri/**`。
- Dark Theme 是 regression baseline，不能为了 Light Theme 随意改变现有深色视觉。

---

## M9-T01 — Theme model and DOM controller

### Goal

建立 ThemePreference / EffectiveTheme 的单一前端模型。

### Implement

新增建议模块：

```text
ui/src/theme/theme.ts
ui/src/theme/theme.test.ts
```

实现：

- `ThemePreference = "system" | "light" | "dark"`
- `EffectiveTheme = "light" | "dark"`
- persisted value validator
- missing / null / unsupported → dark
- `resolveEffectiveTheme(preference, systemDark)`
- root `<html data-theme>` apply helper
- `color-scheme` synchronization
- System `matchMedia("(prefers-color-scheme: dark)")` observer，支持 cleanup
- `matchMedia` unavailable 时 fallback dark

### Unit tests

至少覆盖：

- explicit dark
- explicit light
- system + dark OS
- system + light OS
- missing/null
- unsupported string
- DOM attribute application
- System listener change + unsubscribe

### Done when

Theme core 不依赖 React 页面即可独立测试通过。

---

## M9-T02 — Pre-render theme bootstrap

### Goal

应用首次可见 React render 前应用正确 Theme，避免反色 flash。

### Implement

调整启动 presentation bootstrap：

```text
configGet
  ↓
resolve saved language + theme
  ↓
apply effective theme
  ↓
initialize language
  ↓
React render
```

优先复用一次 `configGet()`；不要为了 language/theme 各读一次 config。

要求：

- config read failure → theme dark fallback；
- 不阻断现有 Provider 后续 recovery read；
- persisted `system` 在 bootstrap 时按当前 OS 生效；
- React render callback 执行前 theme DOM contract 已建立。

### Tests

扩展 bootstrap tests：

- saved light 在 render 前设置 light；
- saved dark 在 render 前设置 dark；
- missing theme 保持 dark；
- config read error 仍可 render 且 dark fallback；
- language bootstrap 行为保持原有回归。

### Done when

测试能证明 theme apply 发生在 render callback 之前。

---

## M9-T03 — Settings Appearance and persistence

### Goal

提供 `System / Light / Dark` 的即时切换和持久化。

### Implement

在 Settings 增加 `Appearance` section：

```text
Theme
[ System ] [ Light ] [ Dark ]
```

行为：

1. 从 `config.ui.theme` 得到 persisted preference；missing/invalid 显示 Dark。
2. 用户点击后立即 apply 新 preference。
3. 使用现有 `mutateConfig`，只修改 `latest.ui.theme`。
4. mutation 成功保持选择。
5. mutation 失败恢复 previous persisted preference/effective theme，并显示 inline error。
6. System 模式启用 OS listener；离开 System 后 listener 不再覆盖 explicit theme。
7. Theme 保存不得产生 Restart Required。

不要新增 backend `uiThemeSet`，除非现有 `mutateConfig` 经实际实现证明无法满足安全序列化语义。

### i18n

补齐 English / zh-CN：

- Appearance / 外观
- Theme / 主题
- System / 跟随系统
- Light / 浅色
- Dark / 深色
- 必要的保存反馈

优先复用已有通用 saved/error 文案。

### Tests

至少覆盖：

- initial missing theme → Dark selected
- select Light → immediate DOM light + config save
- select Dark → immediate DOM dark + config save
- select System → current OS effective theme
- save failure → rollback previous theme
- Theme mutation 保留 language/workspace/tools/policies 等其他字段
- 不产生 restart-required 假状态

### Done when

Settings theme lifecycle 完整且失败路径可恢复。

---

## M9-T04 — Light semantic token mapping

### Goal

在 `global.css` 中建立 Codex-inspired Light Theme，同时保持当前 Dark Theme baseline。

### Implement

整理 theme block：

```css
:root,
[data-theme="dark"] { /* M8 current values */ }

[data-theme="light"] { /* M9 values */ }
```

Light baseline：

```css
--bg:               #f7f7f5;
--surface-1:        #f2f2ef;
--surface-2:        #ecece8;
--surface-selected: #e7e7e2;
--overlay:          #ffffff;
--border:           #deded8;
--border-strong:    #c9c9c2;
--text-primary:     #20201e;
--text-secondary:   #60605b;
--text-muted:       #85857e;
--text-disabled:    #a7a7a0;
--status-success:   #2f7d4f;
--status-warning:   #946716;
--status-danger:    #b34a53;
--status-info:      #426f99;
```

允许为真实对比度小幅调整。

### Token hardening

至少把以下 dark-only 值收敛成语义规则/token：

- modal scrim
- elevated shadow
- hover treatment based on `filter: brightness(...)`
- 必要的 native scrollbar/control contrast

不得给组件增加成片的 `[data-theme="light"] .component-x` 特判，除非 semantic token 无法表达。

### Done when

主要视觉层级只依赖 semantic token 即可在两套主题成立。

---

## M9-T05 — Shared component and page polish

### Goal

修正 Light Theme 下真实控件状态的可读性，避免“只是换背景色”。

### Check surfaces

逐页快速检查：

- Overview
- Tools
- Policies
- Approvals
- Activity
- Settings
- First Run / Recovery

共享 surface：

- Sidebar / nav
- rows / tables
- Button variants
- Input / Select / Textarea
- Switch / Segmented
- Status badges
- Notice / validation
- Inspector / Sheet / Dialog
- modal scrim
- code / mono / path
- loader / empty / disabled
- scrollbar

### Check states

```text
normal
hover
selected
focus-visible
disabled
saving
success
warning
danger
loading
empty
```

### Rule

发现问题时优先：

1. 修 shared semantic token；
2. 修 shared primitive；
3. 最后才修 page-local CSS。

不得借此任务改页面 IA、文案结构或业务交互。

### Done when

Light Theme 在主要页面没有明显不可读、状态消失、层级反转或过度高对比问题；Dark Theme 无明显视觉回归。

---

## M9-T06 — Focused regression and development closeout

### Goal

只对 M9 及受影响前端做验证，不重新执行 Full V3 External Acceptance。

### Required commands

在 `ui/`：

```text
npm test
npm run typecheck
npm run build
```

全部必须 PASS。

若实际代码修改越过 UI 边界，则追加对应范围测试，并在 development report/提交说明中解释为什么必须越界。

### Focused verification

确认：

- System / Light / Dark 即时切换；
- preference 持久化并在应用重启后恢复；
- System 随 OS 实时变化；
- 启动无明显 theme flash；
- Light 主要 surfaces / controls / overlays 状态清晰；
- Dark 保持 M8 baseline；
- 保存失败 rollback；
- Runtime/Tool/Policy/Approval/Activity 状态不因 Theme 变化；
- 不出现 Theme 导致的 Restart Required。

### Completion

满足上述条件后，记录：

```text
M9 — DEVELOPMENT COMPLETE
```

开发阶段不要求：

- External ChatGPT full-chain manual acceptance
- 整体 V3 人工验收
- Portable/installer 发布验收
- `V3 — ACCEPTED / FROZEN` 标记

这些与 M9 最终收口相关的结果记录在 `FINAL_ACCEPTANCE.md`；本次人工视觉验收仅覆盖 M9 Light Theme & Appearance，不代表 Full V3 External Acceptance。

---

## M9 Final Closeout

- Focused QA：PASS。
- M9 Portable 构建：PASS，最终版本为 `0.1.2`。
- Light Theme palette corrective：已完成。
- 人工视觉验收：PASS（用户确认）。
- Vite chunk size warning 与既有 Rust warning：非阻塞项。

最终状态：

```text
M9 — ACCEPTED / FROZEN
```

本次收口不修改 M9 功能范围，不将整个 V3 标记为 `ACCEPTED / FROZEN`。
