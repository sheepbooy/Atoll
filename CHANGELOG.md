# Changelog

本项目的所有重要变更均记录于此。格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [0.1.24] - 2026-06-24

### 改进
- 移除 Logo 右上角更新黄点，保留三点菜单上的更新提示与安装入口

## [0.1.23] - 2026-06-24

### 新增
- **Codex Desktop** 支持：会话来源识别（Desktop / CLI）、审批后焦点恢复、岛中「Open Codex / Terminal」跳转
- Desktop 工具名 `exec_command` 在审批卡片中显示为可读 Bash 命令

### 改进
- Codex hook 安装优先使用 Codex.app 自带 Node 与 Atoll.app 内置脚本路径，避免 dev 构建路径在 Desktop 中失效
- Settings 检测 hook 指向 dev 构建路径并提示重新安装

## [0.1.22] - 2026-06-24

### 改进
- **检查更新提示**：手动检查且已是最新版时，显示与浮岛风格一致的自定义卡片，替代系统原生弹窗

## [0.1.21] - 2026-06-24

### 新增
- **应用内自动更新**：启动时检测 GitHub Release 新版本，Logo 角标提示，三点菜单支持一键下载安装并重启
- CI 发版流程生成 `latest.json` 与 updater 签名产物（`.tar.gz.sig` / `.msi.sig`）

## [0.1.16] - 2026-06-23

### 新增
- **Claude Desktop** 支持：自动识别 Desktop 与 CLI 会话来源，审批后跳回正确的应用窗口
- 会话感知的焦点恢复（macOS / Windows）

### 修复
- **Windows**：修复 Windows API 导入错误导致 dev 构建失败
- **Windows**：修复系统托盘无图标问题，并放大托盘 Logo 显示
- **Windows**：修复后台 hook 健康检查与 agent hook 触发时偶发弹出终端窗口的问题
- **macOS**：启用 `objc2-foundation/NSString` feature，修复 Release 构建失败
- **macOS**：修复托盘图标 lifetime 编译错误

### 文档
- 新增 CHANGELOG，Release 说明自动从 CHANGELOG 同步

## [0.1.15] - 2026-06-22

### 新增
- **Windows**：micro island 模式放大 Logo，并显示 token 计数器

### 修复
- **Windows**：调整 micro island token 计数器指示器布局

## [0.1.14] - 2026-06-22

### 修复
- 统一 Windows 与 macOS 安装程序图标为 Atoll Logo

## [0.1.13] - 2026-06-22

### 新增
- **Windows**：micro island 模式，支持点击穿透与 session Logo 展示

### 文档
- 补充 cmd 兼容的 Windows 一行安装说明（修复在 cmd 中运行 `irm` 报错）

## [0.1.12] - 2026-06-22

### 修复
- 修复展开态 Agent 标签遮挡 Logo 与刘海区域
- **Windows**：隐藏发布版控制台窗口，优化 compact 顶栏间距

## [0.1.11] - 2026-06-22

### 修复
- **Windows**：加固 Hook bridge——端口 fallback 与单实例保护，提升连接稳定性

## [0.1.10] - 2026-06-22

### 修复
- **Windows**：Hook 命令路径使用正斜杠，兼容 Claude Code bash 执行环境
- **Windows**：剥离 Hook 脚本路径的 UNC 前缀

## [0.1.9] - 2026-06-22

### 新增
- **Windows** 平台支持：跨平台 `platform` 模块、MSI 打包与 PowerShell 一键安装

### 修复
- 补充 `icon.ico`，修复 Windows 构建 Rust 编译错误

## [0.1.8] - 2026-06-22

### 修复
- 无刘海屏上 session 列表与 token 计数器间距不足

## [0.1.7] - 2026-06-20

### 修复
- 首次安装 Hook 后 Logo 仍显示 dead 状态
- 陈旧 snapshot 加载导致在线状态误判

## [0.1.6] - 2026-06-20

### 修复
- Hook 健康检测逻辑：区分「未安装」与「已安装但 drift」
- dead Logo 与 mascot 眼睛状态显示错误

## [0.1.5] - 2026-06-20

### 新增
- Hook 漂移检测；Agent 断开时显示 dead Logo 状态
- compact / expanded 视图按 session 拆分 token 计数器

### 修复
- 修复阻塞生产构建的 TypeScript 错误

### 文档
- 用真实应用截图刷新 README 与品牌素材导出

## [0.1.4] - 2026-06-20

### 新增
- **Codex CLI** Hook 集成与专属 island mascot

### 修复
- 浮岛展开 / 收起动画与刘海屏 compact 布局
- 离线 nap 姿态显示睡帽与 zzz 动画
- `SubagentStop` Hook 不再误刷新 session 活跃状态
- 自动归档过期 session 时 UI 卡顿

## [0.1.3] - 2026-06-18

### 文档
- README 大改版：Demo GIF、视觉素材区、路线图
- 修复 README 媒体素材（PNG 格式、真实 Tauri WebView 截图）

## [0.1.2] - 2026-06-18

### 新增
- Atoll Logo 状态模型与 enter / loop / exit 动画系统
- Header Logo 指示器整合进顶栏

### 修复
- 后端 snapshot 构建、在线检测与 session 归档
- 浮岛 hover 呈现异常
- `release.sh` 完善为一键发布流程

## [0.1.1] - 2026-06-18

### 新增
- macOS 一行 `curl` 安装脚本
- 自动发布流水线与版本号同步（`sync-version.sh`）

### 修复
- 安装脚本 GitHub API 限流导致下载失败
- 全新安装时设置默认值不生效

## [0.1.0] - 2026-06-18

首个公开发布（macOS Apple Silicon）。

### 新增
- Claude Code 权限审批浮岛：多 session 聚合、会话聊天视图、Markdown 渲染
- 一键安装 Claude Hook；Always Approve 与键盘快捷键（Enter / Delete / Shift+Enter）
- Dynamic Island 风格 UI：休眠态、刘海屏适配、菜单栏 Logo 状态指示
- Agent 像素风 mascot（Clawd）、风险感知审批、请求归档与快速拒绝回复
- Session 置顶 / 归档、终端跳转、动态 token 计数器
- 设置 UI（滑块）、NSPanel 浮于菜单栏之上
- GitHub Actions 发布流水线、Homebrew cask 模板

### 修复
- 刘海 MacBook 浮岛位置、窗口控件点击、展开 / 收起过渡动画
- 非激活 panel 上 session 列表 hover、compact / expanded 布局对齐

### 变更
- 仅保留 Apple Silicon 构建（移除 Intel Mac 产物）

[0.1.16]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.16
[0.1.15]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.15
[0.1.14]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.14
[0.1.13]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.13
[0.1.12]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.12
[0.1.11]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.11
[0.1.10]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.10
[0.1.9]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.9
[0.1.8]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.8
[0.1.7]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.7
[0.1.6]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.6
[0.1.5]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.5
[0.1.4]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.4
[0.1.3]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.3
[0.1.2]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.2
[0.1.1]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.1
[0.1.0]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.0
