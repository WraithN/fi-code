# 设置弹窗设计规格书

**日期：** 2026-05-29
**模块：** frontend
**范围：** 新增设置弹窗，统一收拢语言切换和主题选择

---

## 1. 背景与动机

当前前端存在以下问题：
- **语言切换** 暴露在 header 中，占用宝贵的头部空间，且与主题设置分散在不同入口
- **主题切换** 只能通过 `/themes` slash 命令触发，没有可视化入口
- **主题不持久化**：刷新页面后重置为 `deep_ocean`
- Header 中有一个**无功能的齿轮按钮**，用户体验断裂

设计目标：把语言和主题统一收进一个设置弹窗，通过齿轮按钮打开，提升界面整洁度和可用性。

---

## 2. 方案选择

采用**方案 A：统一收进设置弹窗**。

- 移除 header 中的 `LanguageSwitcher`
- 给齿轮按钮添加 `onClick` 打开 `Dialog`
- 弹窗内包含语言和主题两个设置项
- 主题持久化到 `localStorage`

---

## 3. 组件设计

### 3.1 SettingsDialog

**新建文件**：`frontend/src/components/settings/SettingsDialog.tsx`

复用现有的 `Dialog` 组件，内容布局：

```
┌─────────────────────────────┐
│  设置                    ✕  │
├─────────────────────────────┤
│  语言 / Language             │
│  [🇨🇳 中文]  [🇬🇧 English]   │
│                              │
│  主题 / Theme                │
│  ┌────────────────────────┐ │
│  │ deep_ocean          ▼  │ │
│  └────────────────────────┘ │
└─────────────────────────────┘
```

#### 语言选择

- 两个并排按钮：`🇨🇳 中文` / `🇬🇧 English`
- 当前选中的语言：`bg-brand text-white rounded-lg px-4 py-2 text-sm`
- 未选中的语言：`bg-bg-overlay text-text-muted rounded-lg px-4 py-2 text-sm hover:bg-bg transition-colors`
- 点击调用 `i18n.changeLanguage()`，弹窗保持打开，用户可立即看到效果

#### 主题选择

- 一个 `<select>` 下拉框
- 选项来自 `themePresets`（32 个主题）
- 当前值绑定 `themeName`
- 切换时调用 `setThemeName()` + `applyTheme()`，弹窗保持打开，用户可预览效果
- 样式：`w-full bg-bg border border-border rounded-lg px-3 py-2 text-sm text-text focus:outline-none focus:border-brand`

### 3.2 Header 改造

**修改文件**：`frontend/src/components/layout/AppLayout.tsx`

1. 移除 `<LanguageSwitcher />` 导入和渲染
2. 给齿轮按钮添加 `onClick={() => setIsSettingsOpen(true)}`
3. 新增本地状态：`const [isSettingsOpen, setIsSettingsOpen] = useState(false);`
4. 在布局中渲染 `<SettingsDialog isOpen={isSettingsOpen} onClose={() => setIsSettingsOpen(false)} />`

---

## 4. 持久化

### 语言
`i18next-browser-languagedetector` 已自动持久化到 `localStorage`（key：`i18nextLng`），无需额外处理。

### 主题
在 `uiStore.ts` 中增加持久化：

```ts
// 初始化时读取 localStorage
const savedTheme = localStorage.getItem('fi-code-theme');

// setThemeName 时写入
setThemeName: (name) => {
  localStorage.setItem('fi-code-theme', name);
  set({ themeName: name });
},
```

---

## 5. 初始化流程

`AppLayout.tsx` 现有的 `useEffect` 监听 `themeName`，变化时调用 `applyTheme()`。只需确保 `uiStore` 初始化时从 `localStorage` 读取主题名即可。

---

## 6. 实时预览

| 设置项 | 切换行为 |
|--------|---------|
| 语言 | `i18n.changeLanguage()` 立即生效，不关闭弹窗 |
| 主题 | `setThemeName()` + `applyTheme()` 立即生效，不关闭弹窗 |

用户可以在弹窗打开时反复切换，实时看到效果，确认后再点击 ✕ 关闭。

---

## 7. 相关文件

| 文件 | 操作 | 说明 |
|------|------|------|
| `frontend/src/components/settings/SettingsDialog.tsx` | 新增 | 设置弹窗组件 |
| `frontend/src/components/layout/AppLayout.tsx` | 修改 | 移除 LanguageSwitcher，添加齿轮按钮 onClick，渲染 SettingsDialog |
| `frontend/src/stores/uiStore.ts` | 修改 | themeName 初始化从 localStorage 读取，setThemeName 写入 localStorage |

---

## 8. 风险评估

| 风险 | 缓解措施 |
|------|---------|
| 移除 header LanguageSwitcher 后用户找不到语言切换 | 齿轮按钮直观，弹窗内语言选项醒目 |
| 主题下拉框 32 个选项过长 | 使用原生 `<select>`，浏览器自带滚动，无需额外处理 |
| localStorage 主题名与预设不匹配（如预设被删除） | 初始化时用 `getPresetByName()` 兜底，找不到则回退 `deep_ocean` |
