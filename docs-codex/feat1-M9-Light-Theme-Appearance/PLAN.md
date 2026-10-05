# M9 PLAN — Light Theme & Appearance

> Status: IMPLEMENTATION PLAN  
> Depends on: `SPEC.md` and completed M8 UI/UX redesign  
> Scope: focused UI appearance increment

## 1. Delivery strategy

M9 采用“小步扩展现有 M8 semantic token system”的方式实现，不重写页面结构、不引入新的主题框架依赖，也不修改 Runtime contract。

实现顺序：

```text
Theme model
  ↓
Pre-render bootstrap
  ↓
Persistence + Settings
  ↓
Light token mapping
  ↓
Component polish
  ↓
Focused regression
```

关键原则：

- 先建立 preference/effective-theme 单一模型，再做 CSS；
- Dark 保持当前 baseline；
- Light 只通过 semantic token mapping 与极少数必要状态规则实现；
- Theme mutation 复用现有 config mutation queue；
- 不新增 Runtime restart / External acceptance gate。

## 2. P0 — Baseline audit

### Goal

确认 M8 当前实现中需要主题化的入口与少量 dark-only hardcode。

### Actions

- 审计 `ui/src/styles/global.css` 中现有 semantic tokens。
- 搜索直接颜色值、`rgba(...)`、shadow、`filter: brightness(...)`、native color assumptions。
- 确认 Settings presentation preference 区域与现有 mutation pattern。
- 确认 `UiConfig.theme` 当前读写路径，无需 schema migration。

### Output

形成实现时使用的 token checklist，不新增独立设计阶段。

## 3. P1 — Theme core and bootstrap

### Goal

建立可测试的 ThemePreference / EffectiveTheme 模型，并在首次 React render 前应用主题。

### Recommended files

```text
ui/src/theme/theme.ts              new
ui/src/theme/theme.test.ts         new
ui/src/main.tsx                    update
ui/src/i18n/index.ts               update or keep language-only
ui/src/i18n/bootstrap.test.ts      update if bootstrap is shared
```

### Design

`theme.ts` 负责：

- `ThemePreference = "system" | "light" | "dark"`
- 校验 persisted string；
- missing / invalid → dark；
- 根据 `matchMedia` 解析 effective theme；
- 写入 `document.documentElement.dataset.theme`；
- 同步 `color-scheme`；
- System 模式 media listener 的注册/清理。

Bootstrap 可采用二选一：

1. 扩展现有 `bootstrapPresentation()`，一次 config read 同时解析 language/theme；或
2. 新增 presentation bootstrap orchestration，让 language/theme 共用同一次 config read。

优先避免应用启动时重复读取 config。

### Acceptance

- dark/light/system 解析单测通过；
- missing/invalid fallback dark；
- theme attr 在 React render 前已设置；
- System listener 能正确更新并清理。

## 4. P2 — Persistence and Settings Appearance

### Goal

让 Theme 在 Settings 中可即时切换并安全持久化。

### Recommended files

```text
ui/src/pages/settings/SettingsPage.tsx
ui/src/pages/settings/SettingsPage.test.tsx
ui/src/app/providers/HarnessDataProvider.tsx   only if a small helper is valuable
ui/src/i18n/en.json
ui/src/i18n/zh-CN.json
```

### Persistence path

默认不增加新的 backend command。

使用现有：

```text
HarnessDataProvider.mutateConfig
  ↓
latest persisted config
  ↓
update latest.ui.theme only
  ↓
configSave
```

这样 Theme 与 Language / Workspace / Tool 等 mutation 共享现有序列化队列，避免旧 snapshot 覆盖新配置。

### Interaction

- 增加 `Appearance` section。
- Theme control 使用现有 `Segmented` 或同等紧凑控件：`System / Light / Dark`。
- 用户选择后先即时 apply，再 persist。
- persist 成功：保持。
- persist 失败：恢复 previous persisted preference，重新 apply previous effective theme，显示局部错误。
- System preference 激活/退出时正确管理 OS listener。

### Acceptance

- 三态 selection 与 config 一致；
- 保存成功持久化；
- 保存失败 rollback；
- Theme mutation 不覆盖其他 config 字段；
- 中英文文案完整；
- 不出现 Restart Required。

## 5. P3 — Light semantic tokens

### Goal

在不改组件结构的前提下建立完整浅色 visual mapping。

### Primary file

```text
ui/src/styles/global.css
```

### Structure

建议：

```css
:root,
[data-theme="dark"] {
  /* current M8 dark tokens */
}

[data-theme="light"] {
  /* M9 light token mapping */
}
```

Dark token 数值尽量保持不变。

Light 起点遵循 SPEC palette，并按真实 UI 做有限调整。

### Hardening

把 dark-only one-offs 语义化，例如：

- modal scrim → `--scrim`
- elevation shadow → `--shadow-elevated`
- explicit control hover surface → semantic surface/token

避免 light theme 继续依赖 `filter: brightness(1.15)` 这种在浅背景下方向不确定的效果。

### Acceptance

- 主背景、sidebar、selected/hover 层级清晰；
- input/button/switch/segmented 可辨识；
- overlay/dialog 与 background 有足够层级；
- danger/warning/success/info 在浅背景上可读；
- dark baseline 无明显漂移。

## 6. P4 — Component polish

### Goal

针对 Light Theme 做有限的真实界面校正，而不是组件重写。

### Focus areas

按以下页面/表面快速检查：

```text
Overview
Tools
Policies
Approvals
Activity
Settings
First Run / Recovery
Inspector / Sheet / Dialog
```

重点状态：

```text
hover
selected
focus
saving
disabled
warning
danger
success
loading
empty
modal overlay
code / mono
```

若发现问题，优先调整 shared token 或 shared primitive；只有无法通过 shared rule 正确表达时才增加局部样式。

## 7. P5 — Focused verification

M9 不重新跑完整 External ChatGPT acceptance。

### Automated commands

在 `ui/`：

```text
npm test
npm run typecheck
npm run build
```

若实现意外触及 Rust/Tauri/Protocol，再执行对应范围测试；在正常 M9 边界内不要求无关 Runtime 全套测试。

### Required automated coverage

- theme preference parser/resolver
- invalid/missing fallback
- System media change
- bootstrap before render
- Settings selection and persistence
- persistence failure rollback
- config mutation preservation
- dark/light DOM contract

### Focused visual spot check

仅检查 M9 相关外观，不做整体验收：

- Dark 仍与 M8 基线一致；
- Light 下主页面和 overlay surfaces 无明显 unreadable / invisible states；
- System 实际跟随 OS；
- 重启后 preference 恢复；
- 启动无明显反色 flash。

## 8. Expected change boundary

正常情况下：

```text
ui/src/theme/**                     new
ui/src/main.tsx                     small update
ui/src/pages/settings/**            focused update
ui/src/i18n/*.json                  strings
ui/src/styles/global.css            theme mapping/polish
相关 frontend tests                  update/new
```

正常情况下不修改：

```text
runtime/**
protocol/**
ui/src-tauri/**
```

`protocol/src/v1/config.rs` 已有 `ui.theme`，因此 M9 不应为了类型美化而主动制造 schema / generated-contract churn。

## 9. Completion definition

当 SPEC 的 focused verification 全部满足，且 frontend test/typecheck/build 通过，即可标记：

```text
M9 — DEVELOPMENT COMPLETE
```

后续是否发布、打 Portable、做更大范围产品验收，由独立发布决策决定，不属于 M9 的完成门槛。
