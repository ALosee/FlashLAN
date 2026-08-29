# FlashLAN Visual Acceptance Checklist

本文档用于每次 UI 修改后的自检和评审。通用规则见 [DESIGN.md](../../DESIGN.md)，页面结构见 [page-contracts.md](page-contracts.md)。

## 1. 检查环境

至少检查以下视口：

| 场景         | 视口         |
| ------------ | ------------ |
| 紧凑桌面窗口 | `1024 × 768` |
| 标准桌面窗口 | `1280 × 720` |
| 宽桌面窗口   | `1440 × 900` |
| 标准移动端   | `390 × 844`  |
| 窄移动端     | `360 × 800`  |

每个受影响页面至少检查：

- Light Mode。
- Dark Mode。
- `100%` 和 `200%` 页面缩放。
- 桌面键盘操作。
- 移动端触控目标和 Safe Area。

浏览器预览使用 Mock 数据。涉及真实权限、目录、网络、配对或系统集成的状态，还需要在对应 Tauri 平台验证。

## 2. Code Preflight

- [ ] 已完整阅读 `DESIGN.md` 和受影响页面合同。
- [ ] 已确定复用或新增的 Pattern。
- [ ] View 和 Layout 中没有新增任意视觉值。
- [ ] 没有新增直接 Hex、RGB、HSL、OKLCH 或具体调色板颜色。
- [ ] 没有在页面中重新实现已有基础组件。
- [ ] 重复结构已经按规则提取为 Pattern。
- [ ] 纯设计修改没有改变业务语义和数据流。

可以使用以下命令辅助检查新增代码：

```bash
rg "(rounded|text|shadow|gap|[mp][trblxy]?)-\\[" src/views src/layouts
rg "(violet|indigo|zinc|slate|gray)-[0-9]" src/views src/layouts
rg "#[0-9a-fA-F]{3,8}|rgb\\(|hsl\\(|oklch\\(" src/views src/layouts
```

既有命中不自动阻塞与其无关的修改，但本次 Diff **MUST NOT** 增加新的命中。

## 3. Global Visual Check

### Layout

- [ ] Sidebar、页面边距和内容最大宽度符合合同。
- [ ] 内容在宽窗口中没有无意义地拉伸。
- [ ] 页面标题区没有被 Card 包裹。
- [ ] 相同层级在不同页面具有一致密度。
- [ ] 没有横向滚动承载核心内容。
- [ ] Safe Area 没有被 Header、Drawer、Toast 或底部导航覆盖。

### Hierarchy

- [ ] 3 秒内能识别页面目标、当前状态和主操作。
- [ ] 同一工作区域最多一个视觉上最强的 Primary Action。
- [ ] 辅助文字、Metadata 和状态不会抢夺标题注意力。
- [ ] 品牌色只出现在主操作、选中、焦点和进度等语义位置。
- [ ] 成功、错误、离线和信任状态不只依赖颜色。

### Surface

- [ ] 没有 Card 套 Card。
- [ ] 设备、记录、会话和设置使用列表行而非 Card 堆叠。
- [ ] 普通内容没有阴影。
- [ ] 圆角不超过合同范围。
- [ ] 空状态、上传区和单条对象没有被无意义放大。

### Typography and Content

- [ ] 用户可见文字不小于 `12px`。
- [ ] 页面标题、Section 标题、正文和辅助文字层级一致。
- [ ] 长设备名、文件名、路径、IP 和 URL 不破坏布局。
- [ ] 机器信息保持准确、可复制，并在适合时使用等宽字体。
- [ ] 文案使用稳定术语，没有同义词漂移。

### Interaction

- [ ] Hover、Pressed、Focus、Selected、Disabled 和 Loading 状态可区分。
- [ ] 键盘焦点清楚且顺序合理。
- [ ] 图标按钮有可访问名称。
- [ ] 移动端触控目标至少 `44 × 44px`。
- [ ] 异步操作不会重复提交或造成布局跳动。
- [ ] 危险操作说明影响范围并提供确认。

## 4. Required State Matrix

受影响的 Pattern 必须从下表选择所有适用状态验证：

| 类别 | 状态                                             |
| ---- | ------------------------------------------------ |
| 数据 | 初始、空、单条、多条、长内容                     |
| 异步 | 加载、刷新、等待确认、成功、失败、重试           |
| 设备 | 在线、离线、扫描中、可信、未信任、指纹变化       |
| 传输 | 待处理、发送中、接收中、完成、失败、取消         |
| 表单 | 未修改、已修改、无效、保存中、保存成功、保存失败 |
| 操作 | 默认、Hover、Pressed、Focus、Selected、Disabled  |
| 平台 | 桌面、移动端、Light、Dark                        |

不得只验证 Mock 数据下最理想的正常状态。

## 5. Page-specific Check

### 传文件

- [ ] UploadZone 不超过合同高度。
- [ ] 主操作会随着文件选择状态正确转移。
- [ ] 设备选择和发送目标清楚。
- [ ] 传输中显示真实进度、方向和恢复操作。
- [ ] 无任务状态只占用必要空间。

### 消息

- [ ] 消息正文列限宽，阅读视线不横跨整个窗口。
- [ ] 会话选中态不是纯品牌色背景。
- [ ] 气泡宽度、圆角和正文换行符合合同。
- [ ] 离线、失败和重试路径清楚。
- [ ] 移动端没有压缩显示双栏。

### 附近设备

- [ ] 每台设备使用统一 `DeviceRow`。
- [ ] 扫描、添加和二维码操作没有同时成为主操作。
- [ ] 自动发现、手动添加、可信和本机状态容易区分。
- [ ] 操作在桌面和移动端都可发现。

### 传输记录

- [ ] 记录是列表而不是 Card 堆叠。
- [ ] 文件名、方向和状态优先于路径。
- [ ] 长路径正确截断。
- [ ] 清空操作明确说明不会删除文件。

### 设置

- [ ] 设置项使用分组行，不是独立 Card。
- [ ] 保存按钮只在有有效变化时强调。
- [ ] 自动接收的安全影响可见。
- [ ] 主题选择没有使用三个巨大选择 Card。
- [ ] 只读技术信息的视觉权重最低。

## 6. Engineering Verification

- [ ] `pnpm typecheck`
- [ ] `pnpm lint:check`
- [ ] `pnpm fmt:check`
- [ ] 浏览器预览无新增 Console Error。
- [ ] 相关 Tauri 平台无新增运行时错误。
- [ ] 必要时更新并人工检查 README 截图。

优先运行完整检查：

```bash
pnpm check
```

若存在与本次修改无关的既有失败，交付说明中必须列出具体命令和失败原因。

## 7. Review Record Template

UI 变更可以在 PR 或交付说明中使用以下模板：

```md
### Design verification

- Affected pages/patterns:
- Reused patterns:
- New patterns:
- Viewports checked:
- States checked:
- Light/Dark checked:
- Keyboard/Touch checked:
- Commands run:
- Known exceptions:
```
