# M9 SPEC — Light Theme & Appearance

> Status: ACCEPTED / FROZEN
> Milestone: M9 — Light Theme & Appearance  
> Baseline: M8 Codex-inspired Minimal Developer Desktop UI  
> Scope: UI appearance only; no full V3 external acceptance
> Final acceptance: `FINAL_ACCEPTANCE.md`

## 1. Objective

在不改变 M0–M8 产品能力、信息架构、交互模型、Runtime 行为与安全语义的前提下，为 Harness 增加完整的 Appearance 能力：

- 保留 M8 当前深色主题；
- 新增 Codex-inspired 浅色主题；
- Settings 提供 `System / Light / Dark`；
- 主题即时切换、持久化，并在应用重启后恢复；
- `System` 跟随操作系统主题并在系统主题变化时实时更新；
- 应用首次可见渲染前应用正确主题，避免明显的深浅色闪烁。

浅色主题借鉴 Codex 的视觉方向：中性、克制、低饱和、低装饰、薄边框、高信息清晰度。不得复制具体页面、品牌资产或像素级样式。

## 2. Scope

### 2.1 In scope

- Theme preference 与 effective theme 的前端模型。
- `System / Light / Dark` 三态解析。
- 应用启动时的 theme bootstrap。
- OS theme change 监听。
- `config.ui.theme` 持久化。
- Settings > Appearance UI 与中英文文案。
- M8 semantic color tokens 的 light mapping。
- 与浅色主题有关的 hover / selected / focus / disabled / overlay / modal / code / scrollbar 等状态调整。
- 聚焦的前端自动化测试与视觉 spot check。

### 2.2 Out of scope

- 不重新设计 M8 Information Architecture、页面结构或交互模型。
- 不新增一级页面。
- 不修改 Runtime、MCP、Tunnel、Policy、Approval、WorkspaceGuard、Bash enforcement 或 Activity 语义。
- 不要求 Runtime restart，不产生 `Restart required`。
- 不新增 Full V3 External Acceptance 里程碑或总体验收流程。
- 不引入新的字体文件、品牌素材或复杂主题编辑器。
- 不支持用户自定义颜色、Accent Color 或更多主题变体。

## 3. Theme model

定义 UI-only preference：

```text
ThemePreference = "system" | "light" | "dark"
EffectiveTheme  = "light" | "dark"
```

解析规则：

| Saved preference | Effective theme |
| --- | --- |
| `dark` | dark |
| `light` | light |
| `system` | OS `prefers-color-scheme` |
| missing / null | dark |
| unsupported value | dark, fail safe |

`missing / null → dark` 是兼容性要求：已有 M8 用户升级到 M9 后，首次启动外观不得被静默改变。

不要求为了 M9 修改 config schema version。现有 `UiConfig.theme` 槽位直接承载上述字符串值；Theme 仍属于 Host/UI preference，不进入 Runtime snapshot。

## 4. Persistence behavior

Settings 修改 Theme 时：

1. 用户选择 `System / Light / Dark`。
2. UI 立即应用新的 effective theme。
3. 通过现有 config mutation 协调链写入 `ui.theme`。
4. 保存成功后保持当前外观。
5. 保存失败时恢复上一个已持久化 preference/effective theme，并显示局部错误。

要求：

- Theme mutation 必须走现有共享 config mutation queue，不以长期持有的整份 config 覆盖其他并发保存。
- Theme 保存不得修改 `configRevision` 相关 Runtime effective 语义，不得触发 Harness restart。
- 进入其他页面、关闭 Inspector、刷新资源均不得重置当前已保存主题。

## 5. System theme behavior

当 preference = `system`：

- 使用 `window.matchMedia("(prefers-color-scheme: dark)")` 解析 effective theme。
- 系统主题改变时，在应用运行期间实时更新。
- 只改变 effective theme，不重复写入配置。
- 切离 `system` 后，停止以 OS 变化覆盖用户显式选择。

在测试环境或 `matchMedia` 不可用时，安全 fallback 为 dark。

## 6. Bootstrap and flash prevention

主题必须在 React 主 UI 首次 render 前完成解析与 DOM 应用。

推荐顺序：

```text
read config
  ↓
resolve language + theme preference
  ↓
resolve effective theme
  ↓
set <html data-theme="light|dark">
set color-scheme
  ↓
render React
```

允许把现有 presentation bootstrap 扩展为同时处理 language/theme，或新增独立 theme bootstrap；但不得在 React 已经可见后才首次套用主题。

若启动时 config 读取失败：

- Theme fallback dark；
- 不阻断现有 Provider 的配置恢复流程；
- 后续成功读取到配置后允许同步到正确主题。

## 7. DOM contract

统一使用根节点 attribute：

```html
<html data-theme="dark">
<html data-theme="light">
```

并让 browser-native controls 与滚动条获得正确 color scheme：

```css
:root { color-scheme: dark; }
[data-theme="light"] { color-scheme: light; }
```

组件不得通过 JavaScript 判断 light/dark 后分别写颜色；组件继续消费 semantic tokens。

## 8. Visual direction

### 8.1 Design character

Light Theme 必须延续 M8 的视觉语言：

- neutral grayscale foundation
- warm-neutral off-white background
- restrained semantic status colors
- thin borders
- little or no persistent shadow
- no gradient / glow
- compact controls
- content-first / status-first
- IDE-like desktop composition

浅色主题不能退化成传统 SaaS dashboard、纯白卡片墙或高饱和彩色 UI。

### 8.2 Starting token palette

以下是实现起点；允许为可读性和真实控件状态做小范围调整，但应保持整体色相关系：

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
```

Light semantic status colors应比 dark theme 更深、更克制，以保证浅背景上的可读性。建议起点：

```css
--status-success: #2f7d4f;
--status-warning: #946716;
--status-danger:  #b34a53;
--status-info:    #426f99;
```

### 8.3 Token hardening

M9 应清理少量只适合 dark theme 的硬编码视觉值，至少包括：

- modal scrim
- persistent shadow color
- hover behavior that relies on `filter: brightness(...)`
- scrollbar / native-control contrast where applicable

优先新增/复用 semantic token，例如：

```text
--scrim
--shadow-elevated
--control-hover
```

不得创建 `--light-*` / `--dark-*` 这类组件绑定型 token；主题差异应体现在同一 semantic token 的不同映射。

## 9. Component coverage

Light Theme 至少必须覆盖并检查：

- App background / Sidebar / navigation selected & hover
- Page header / section separators / rows
- Primary / secondary / quiet / danger / warning buttons
- Input / Select / Textarea
- Switch / Segmented
- Status badge / status dot
- Notice / inline feedback / validation error
- Table / list selected & hover
- Inspector / Sheet / Dialog / modal scrim
- Code / mono / path / preformatted content
- Loading / empty / disabled states
- Focus-visible outline
- Scrollbar and native color-scheme behavior
- First Run / Recovery surfaces

Dark theme 在 M9 中属于 regression baseline：除为 semantic token architecture 必须进行的等价重构外，不应出现明显视觉漂移。

## 10. Settings UX

在 Settings 中增加 `Appearance` 区域，使用现有 M8 控件语言，不新增独立页面。

推荐：

```text
Appearance
Theme
[ System ] [ Light ] [ Dark ]
```

要求：

- 三个选项语义明确；
- 选择即生效；
- saving / failure 使用现有局部反馈模式；
- English / 简体中文文案完整；
- Theme 与 Language 都属于 presentation preference，但互不覆盖；
- 不显示 Restart Required。

## 11. Accessibility and contrast

- 正文、次级文字、disabled、边框和 focus 必须在 light theme 中仍可辨识。
- 不依赖颜色作为唯一状态信号；继续保留现有文字、图标、位置等语义。
- Focus ring 在两套主题中都必须明显。
- Danger / warning / success 需在浅背景上可读，避免过浅的粉彩状态色。
- Hover 与 selected 必须可区分，但不得使用大面积强对比底色破坏 M8 的克制风格。

## 12. Implementation boundary

默认允许修改：

```text
ui/src/**
```

预期不需要修改：

```text
runtime/**
protocol/**
ui/src-tauri/**
```

如果实现过程中发现必须修改上述边界，先证明现有 UI config contract 无法满足需求；不得借 M9 修改 Runtime/security contract。

## 13. Focused verification

M9 不进行 V3 总体验收。完成条件只验证本里程碑：

1. `System / Light / Dark` 均可选择并即时生效。
2. Theme preference 正确持久化，应用重启后恢复。
3. `System` 能响应 OS theme change，且不重复写配置。
4. 首次可见 render 前已应用正确主题，无明显 dark/light flash。
5. Light Theme 使用 semantic tokens 覆盖 M8 主要 UI surface 与控件状态。
6. Dark Theme 保持 M8 baseline，没有明显回归。
7. Theme 保存失败能回退到上一个 persisted preference，并给出局部错误。
8. Theme 切换不改变 Runtime、Tool、Policy、Approval、Activity 或 Restart Required 语义。
9. 前端 unit/integration regression、typecheck 与 production build 通过。

达到以上条件即可认为 M9 Development Complete；本 Milestone 的 Portable 构建与人工视觉验收由 `FINAL_ACCEPTANCE.md` 记录；不附加 Full V3 External Acceptance gate。
