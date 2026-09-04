# MarkBox

Windows 托盘常驻小工具：从主界面或托盘一键唤起微信截图式圈选，在屏幕上留下一个不挡任何操作的边框标记——用来标记"我要点的位置"。

- 主窗口 / 托盘菜单：开始圈选（松开鼠标后可拉宽/拉高/移动，✓ 或回车确认）、清除标记
- 无全局快捷键、无开机自启，所有操作都在软件界面和托盘完成

状态：已实现（版本以 [Releases](../../releases) 页为准），设计见 [docs/superpowers/specs/2026-09-03-markbox-design.md](docs/superpowers/specs/2026-09-03-markbox-design.md)

## 开发与构建

前置要求：Node ≥ 20、pnpm 11、Rust stable。

- 本地调试（macOS/Windows）：`pnpm install` 安装依赖后运行 `pnpm tauri dev`
- Windows 发布：推送 `v*` tag 触发 GitHub Actions，自动产出 NSIS 安装包与免安装版 zip（内含 markbox.exe，解压即用）

技术栈：Tauri 2 (Rust) + Vanilla TypeScript + Vite 多入口

## License

[MIT](LICENSE)
