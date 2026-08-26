# FlashLAN - 局域网快传

> 基于 Tauri 2 + Vue 3 + TypeScript + SoybeanUI + UnoCSS 的跨平台局域网快传桌面应用

## 技术栈

| 层级     | 技术                                   | 版本   |
| -------- | -------------------------------------- | ------ |
| 桌面壳   | Tauri 2                                | 2.x    |
| 前端框架 | Vue 3                                  | 3.5.x  |
| 语言     | TypeScript                             | 5.6    |
| UI库     | SoybeanUI (@soybeanjs/ui)              | 0.30.0 |
| CSS引擎  | UnoCSS + @soybeanjs/ui-uno presetSbean | 66.x   |
| 状态管理 | Pinia                                  | 3.x    |
| 路由     | Vue Router                             | 4.x    |
| 后端     | Rust                                   | 1.7x   |

## 快速开始

```bash
# 安装依赖
pnpm install

# 开发 (Web)
pnpm dev

# 开发 (Tauri 桌面)
pnpm tauri dev

# 构建 (Web)
pnpm build

# 构建 (Tauri App)
pnpm tauri build
```

## 项目结构

```
FlashLAN/
├── src/
│   ├── components/        # 业务组件
│   ├── composables/       # 组合式函数
│   ├── layouts/           # 布局 (MainLayout)
│   ├── views/             # 页面 (Home/Devices/History/Settings)
│   ├── stores/            # Pinia (device, transfer)
│   ├── router/            # Vue Router
│   ├── ui/                # SoybeanUI 组件 (sbean 管理)
│   ├── styles/            # 全局样式
│   ├── App.vue
│   └── main.ts
├── src-tauri/
│   ├── src/lib.rs         # Tauri commands: greet, get_device_info, discover_devices, send_file
│   ├── Cargo.toml
│   └── tauri.conf.json
├── uno.config.ts          # UnoCSS + presetSbean
├── vite.config.ts         # Vite + Vue + UnoCSS + SoybeanResolver
├── sbean.json             # SoybeanUI 配置
└── opencode.json          # MCP: sbean + pencil
```

## SoybeanUI 使用

```bash
# 查看可用组件
npx sbean list

# 添加组件
npx sbean add button card dialog input

# 查看组件源码
npx sbean view button
```

已安装: button, card, dialog, icon, link, toast, config-provider

## Tauri Commands (Rust)

- `greet(name: string) -> string` - 示例
- `get_device_info() -> DeviceInfo` - 本机设备信息
- `discover_devices() -> DeviceInfo[]` - mDNS 发现 (TODO)
- `send_file(path: string, target_ip: string) -> Result<string, string>` - 文件发送

接收文件在 Android 10+ 使用 MediaStore 写入公共 `Download/FlashLAN`，接收期间以 pending 条目保存，完成后发布到系统文件管理器；旧版本或 MediaStore 不可用时回退到应用数据目录。前端通过 `transfer_started`、`transfer_progress`、`transfer_complete` 事件展示收发方向、进度、速度与保存位置。

## 下一步

- [ ] 实现 mDNS 设备发现 (mdns-sd)
- [x] 实现 TCP 文件传输 + 进度事件
- [ ] 拖拽/剪贴板集成
- [ ] 托盘、通知
- [x] 接收确认与自动接收设置

```

---
```
