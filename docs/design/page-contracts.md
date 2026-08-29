# FlashLAN Page and Pattern Contracts

本文档定义各页面的信息结构和可复用 Pattern。所有数值和通用规则以根目录 [DESIGN.md](../../DESIGN.md) 为准。

## 1. Shared Patterns

### `AppSidebar`

职责：桌面端一级导航、本机状态和品牌入口。

约束：

- 固定宽度 `224px`，不随窗口内容扩张。
- 品牌区高度 `56px`。
- 导航项使用 `32–36px` 的紧凑高度。
- 选中项为浅色品牌背景和品牌色文字，不使用纯品牌色填充。
- 本机状态固定在底部，IP 截断但保留可访问完整值。
- 导航和本机状态之间不得加入统计 Card 或装饰内容。

### `MobileAppBar`

职责：移动端品牌、当前页面上下文和菜单入口。

约束：

- 包含顶部 Safe Area。
- 功能按钮触控区域至少 `44px`。
- 不重复显示页面内部已有的大标题。
- Drawer 中的导航结构和名称与桌面端一致。

### `PageHeader`

职责：页面标题、简短说明、状态摘要和页面级操作。

建议接口：

```ts
interface PageHeaderProps {
  title: string
  description?: string
  status?: string
}
```

约束：

- 标题和说明左对齐。
- 页面级操作位于桌面端右侧、移动端标题下方。
- 移动端最多两个紧凑图标操作可以与标题同行，避免形成只有按钮的独立空白行。
- 状态摘要使用文字、状态点或紧凑 Badge。
- 不使用外层 Card、阴影或彩色背景。

### `SectionHeader`

职责：Section 名称、数量和局部操作。

约束：

- 标题 `16px`，数量使用辅助文字或单个紧凑 Badge。
- 最多一个局部 Primary Action。
- 不重复 PageHeader 已经表达的信息。

### `DeviceRow`

信息顺序：

```text
[设备图标] 设备名称                         在线状态
           IP:端口 · 平台 · 发现来源         行操作
```

约束：

- 高 `72–80px`。
- 图标或 Avatar 为 `32px`。
- 设备名称是唯一主要文字。
- 在线、离线、可信和手动添加通过稳定的语义状态表达。
- 常用操作最多一个直接显示，其余进入 Overflow Menu。
- 设备集合使用列表和 Divider，不使用 Card Grid。

### `TransferRow`

信息顺序：

```text
[方向] 文件名                               状态
       对端设备 · 大小 · 时间               行操作
       路径（仅在有价值时显示）
```

约束：

- 高 `64–72px`；需要显示路径时允许自然增高一行。
- 文件名、状态和传输方向优先于本机绝对路径。
- “打开目录”可以直接显示，删除操作默认进入 Overflow Menu。
- 多条记录共享一个列表 Surface，不创建独立 Card。

### `SettingsRow`

信息顺序：

```text
[图标] 名称与说明                           当前值 / 控件
```

约束：

- 高 `56–64px`；复杂选项可以展开为两行布局。
- 图标为 `16px`，可选 `32px` 的弱背景容器。
- 同一分组中的行使用 Divider。
- 不为每一项创建 Card。
- 可编辑值、保存操作和成功反馈保持在同一个上下文中。
- 分组 Surface 不提供额外内容 Padding；首尾边界与中间行均由 `SettingsRow` 自身的 Padding 定义。
- Switch、Badge 和只读值在移动端保持同行；输入组合和紧凑选择器可以在移动端展开为两行。

### `EmptyState`

建议接口：

```ts
interface EmptyStateProps {
  icon: string
  title: string
  description: string
  actionLabel?: string
}
```

约束：

- 最多一个操作。
- 描述包含为空的原因或下一步建议。
- 默认无 Card、无阴影、无大面积品牌色。

### `StatusIndicator`

职责：统一在线、离线、扫描、可信、传输和反馈状态。

约束：

- 使用语义色 + 文字或图标。
- 相同状态在所有页面使用相同文案和颜色。
- 不允许页面自行组合新的在线 Badge 或状态点样式。

## 2. 传文件页 `/`

### 任务目标

用户应在一个连续流程中完成：

```text
选择内容 → 选择设备 → 发送 → 查看进度
```

### 桌面结构

```text
PageHeader

UploadZone

Destination Section
  selected device summary
  compact device selector
  primary send action

Transfer Activity
```

约束：

- 内容最大宽度 `1024px`。
- UploadZone 高度 `200–240px`，不得占据大半个窗口。
- 未选择文件时，“选择文件”是主操作，“发送文件”降低强调或禁用。
- 已选择文件后，显示紧凑文件摘要，“发送文件”成为主操作。
- 设备选择优先使用紧凑行、Chip 或横向列表，不使用每台设备一张 Card。
- Active Transfer 使用列表行显示真实进度、方向、速度和恢复操作。
- 无活动传输时使用单行辅助 Empty Hint，不创建大 Card。

### 必须覆盖的状态

- 无文件、无设备。
- 无文件、有设备。
- 已选择一个或多个文件。
- 拖拽进入和无效拖拽。
- 设备离线或发送前连接失败。
- 等待接收端确认。
- 发送中、接收中、完成、失败、取消。

## 3. 消息页 `/messages`

### 任务目标

用户应快速选择设备、阅读上下文并发送短消息。

### 桌面结构

```text
Conversation Rail 224px | Conversation Header
                        | Message Column max 720px
                        | Composer
```

约束：

- 页面可占满主工作区，但消息内容列最大 `720px` 并居中。
- 会话行高 `56–64px`，使用浅色选中态，不使用实色品牌背景。
- 消息气泡最大宽度 `34rem`，圆角 `10–12px`。
- 自己的消息可以使用品牌色，接收消息使用中性 Surface。
- 普通气泡不使用阴影。
- 时间、方向和状态保持次级，不得与正文竞争。
- Composer 始终靠近消息列底部；离线时说明原因和恢复方式。
- 单条删除和清空记录属于危险操作，必须有明确确认和影响说明。

### 移动结构

- 会话列表和消息详情使用分步导航。
- 当前设备通过 Header 返回入口和标题表达。
- 不压缩显示桌面双栏。

### 必须覆盖的状态

- 无设备。
- 有设备但无会话。
- 未选择会话。
- 设备在线、离线和重新在线。
- 发送中、发送失败和重试。
- 长文本、长 URL 和多行文本。

## 4. 附近设备页 `/devices`

### 任务目标

发现、识别、配对和管理同一局域网内的设备。

### 结构

```text
PageHeader + 扫描 / 添加 / 二维码

发现状态摘要

Nearby Devices
  DeviceRow

Trusted / Manual Devices
  DeviceRow

Local Device
  DeviceRow
```

约束：

- 内容最大宽度 `960px`。
- 扫描、添加和二维码操作不得全部使用 Primary 样式；当前最重要操作只有一个。
- 设备按来源或信任状态分组，但每组使用列表，不使用 Card Grid。
- 每台设备保持 `72–80px` 行高。
- 本机使用同一个 `DeviceRow` 结构，通过“当前设备”标识区分。
- 别名、移除信任和删除手动设备进入行操作或 Overflow Menu。
- 扫描状态在 PageHeader 下方以紧凑状态摘要显示，不创建统计 Card。
- 初次发现和手动刷新期间使用与 `DeviceRow` 等高的列表骨架，加载结束后过渡到设备列表或空状态。

### 必须覆盖的状态

- 正在扫描。
- 未发现设备。
- 自动发现设备。
- 手动添加设备。
- 可信、未信任、指纹变化。
- 在线、离线、连接测试中和连接失败。
- 仅有本机设备。

## 5. 传输记录页 `/history`

### 任务目标

快速查看历史结果、定位文件并处理失败记录。

### 结构

```text
PageHeader + 数量摘要 + 清空操作

可选筛选 / 状态摘要

Transfer List
  TransferRow
```

约束：

- 内容最大宽度 `960px`。
- 历史记录是一个列表，不是一组独立 Card。
- 默认按时间倒序。
- 文件名、方向和状态是主要信息；IP、大小和时间是次级信息。
- 路径只在有打开或复制价值时显示。
- “打开目录”可以直接显示；单条删除进入 Overflow Menu。
- “清空记录”保持次级危险操作，必须说明不会删除文件。

### 必须覆盖的状态

- 无记录。
- 发送完成、接收完成。
- 失败、取消和文件不存在。
- 长文件名和长路径。
- 单条删除和全部清空确认。

## 6. 设置页 `/settings`

### 任务目标

低风险地修改本机身份、保存路径、接收策略和主题。

### 结构

```text
PageHeader

Device Group
  SettingsRow
  SettingsRow

Transfer Group
  SettingsRow
  SettingsRow

Appearance Group
  SettingsRow / compact choice control

About
```

约束：

- 内容最大宽度 `720px`。
- 可以使用一个分组 Surface，但不得为每个设置项创建 Card。
- 修改设备名称时，保存操作只在有变化且输入有效时强调。
- 保存路径显示可读路径，并提供选择和完整值查看能力。
- 自动接收需要说明安全影响。
- 主题模式使用紧凑选择控件；三个巨大选择 Card 不符合约束。
- 端口和本机信息属于只读辅助信息，视觉权重最低。
- About 信息放在内容底部，不使用独立宣传 Card。

### 必须覆盖的状态

- 初始加载。
- 修改未保存、保存中、保存成功和保存失败。
- 路径不可用或没有权限。
- 自动接收开关成功和失败。
- 浅色、深色、跟随系统。

## 7. Cross-page Overlays

### `IncomingTransferDialog`

- 明确发送设备、文件名、文件数量和总大小。
- “接收文件”是 Primary，“拒绝”是 Secondary。
- 多个待确认请求显示队列数量，但不把所有内容塞入一个弹窗。
- 文件名和路径必须安全换行或截断。
- Dialog 不使用大于 `12px` 的圆角。

### `TransferActivityPanel`

- 仅在用户离开传文件页且仍有活动任务时出现。
- 显示任务数量、最多两条摘要和真实进度。
- 点击后返回传文件页。
- 是浮层，允许边框、轻微背景模糊和浮层阴影。
- 不遮挡移动端底部导航和 Safe Area。

## 8. Pattern Migration Order

建议按以下顺序迁移现有实现：

1. `StatusIndicator`、Button、Card、Typography Token。
2. `AppSidebar`、`PageHeader`、`SectionHeader`。
3. `SettingsRow` 和设置页。
4. `DeviceRow` 和附近设备页。
5. `TransferRow` 和传输记录页。
6. 传文件页工作流。
7. 消息页双栏、气泡和 Composer。
8. Dialog、Popover 和跨页面传输面板。
