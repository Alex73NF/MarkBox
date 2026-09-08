# MarkBox

<img src="src-tauri/icons/icon.png" width="88" alt="MarkBox 图标" align="right">

Windows 托盘常驻小工具：从主界面或托盘一键唤起微信截图式圈选，在屏幕上留下一个不挡任何操作的边框标记——用来标记"我要点的位置"。

- 主窗口 / 托盘菜单：开始圈选（松开鼠标后可拉宽/拉高/移动，✓ 或回车确认）、清除标记
- 标记框样式可调：6 色糖果色板或自定义取色、边框宽度、圆角
- 无全局快捷键、无开机自启，所有操作都在软件界面和托盘完成

## 下载

Windows 安装包（NSIS）与免安装版 zip（解压即用）见 [Releases](../../releases)。

## 开发与构建

前置要求：Node ≥ 22.12、pnpm 12、Rust stable。

- 本地调试（macOS/Windows）：`pnpm install` 安装依赖后运行 `pnpm tauri dev`
- Windows 发布：推送 `v*` tag 触发 GitHub Actions，自动产出安装包与免安装版 zip
- 应用图标源文件为 `src-tauri/icons/icon.svg`（小尺寸简化变体 `icon-small.svg`）；`pnpm tauri icon` 重生成全套后，需按源文件内注释把 ico 的 16/24/32 帧换回小尺寸变体

技术栈：Tauri 2 (Rust) + Vanilla TypeScript + Vite 多入口

## License

[MIT](LICENSE)
