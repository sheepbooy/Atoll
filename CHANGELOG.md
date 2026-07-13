# Changelog

本项目的所有重要变更均记录于此。格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [0.1.48] - 2026-07-13

### 新增
- **中英文切换**：设置页 Display 分区新增 English/中文 语言切换，主应用壳层 UI 支持全量中英双语，偏好持久化到 localStorage

### 修复
- **Windows Small folded island**：修复小折叠岛收起与 micro header 布局异常
- **设置/Token 子页**：首次打开设置或 Token 子页时同步原生窗口尺寸，避免展开尺寸不正确

## [0.1.47] - 2026-07-12

### 修复
- **Windows Small folded island**：修复有活跃 session 时 Atoll logo、绿色呼吸灯与 session 图标重叠；micro 宽度与 compact 宽度解耦，有 session 时自动加宽 micro 条
- **Windows 展开岛空闲文案**：居中显示 idle 文本

## [0.1.46] - 2026-07-10

### 改进
- **浮岛动效**：统一 motion 令牌，优化展开/收起、视图切换与审批反馈动画；展开面板增加环境光与轻量材质
- **Token 图表展开**：设置尺寸放大改用更稳的缓动，并延迟挂载热力图/图表，减少窗口放大卡顿

### 修复
- **审批 Tab 同步**：新审批到达时自动切换到对应 agent 标签
- **Cursor Subagent**：避免已完成/活跃 subagent 在会话列表中重复出现
- **Codex 审批阻塞**：即使 observer hook 超时，仍保持 PermissionRequest 阻塞，避免误放行
- **Hook Bridge**：加固 bridge 与 snapshot 快路径，降低卡顿与超时风险

## [0.1.45] - 2026-07-09

### 新增
- **定价目录**：远程定价目录 24h 缓存、合并 override 与 usage 发现模型；设置页可刷新目录、编辑单价、从列表隐藏/删除模型
- **用量与费用展示**：设置页接入 Display & pricing；热力图与展开计数器显示费用；Codex token 按当前模型归因
- **Plan 模式**：展开窗口尺寸增大，适配 plan mode 请求

### 修复
- **定价刷新**：目录刷新移至后台线程，避免阻塞 UI；刷新失败时展示错误状态
- **费用格式**：普通金额保留两位小数；热力图费用与展开计数器对齐
- **热力图布局**：agent donut 在卡片内居中，图例保持下方

## [0.1.44] - 2026-07-09

### 修复
- **Windows 岛启动可见性**：修复 Windows 上应用启动时岛窗口不可见或未能正确置顶的问题
- **Codex Stop 超时**：Codex/Claude observer hook 改为后台异步处理，并缩短 runner 请求超时，避免 `Stop` 事件阻塞 agent 30 秒

## [0.1.43] - 2026-07-09

### 修复
- **Cursor Hook 超时**：缩短 hook runner 请求超时（默认 1.2s）与 hooks.json 中 legacy 1800s 配置，启动时自动修复为 30s
- **Cursor Hook 阻塞**：lifecycle 事件改为后台异步处理，避免 observer hook 长时间占用 bridge 连接

### 改进
- **Cursor Hook 日志**：默认关闭成功路径日志，仅错误与 `ATOLL_CURSOR_HOOK_DEBUG=1` 时写入

## [0.1.42] - 2026-07-08

### 修复
- **Windows Codex/Claude 反复弹窗**：检测宿主进程时不再附带抢焦点副作用，避免 snapshot 刷新与 hook 请求周期性把 Codex/Claude 拉到前台
- **Windows 审批后重复启动**：审批完成后仅激活已有窗口，不再在找不到窗口时重新启动 Codex/Claude
- **Windows Claude 宿主识别**：补齐 Claude Desktop 检测，避免 Desktop 运行时被误判为 CLI

## [0.1.41] - 2026-07-08

### 修复
- **Windows micro 折叠岛宽度**：修复小巧模式下折叠岛宽度计算不正确的问题
- **Windows micro 折叠岛尺寸**：进一步缩小折叠岛默认尺寸与样式，减少屏幕占用

## [0.1.40] - 2026-07-08

### 新增
- **Windows 折叠岛尺寸**：设置中新增折叠岛大小选项（常规 / 小巧），偏好持久化到 localStorage

### 改进
- **官网**：更新首页文案与样式，展示 agent workflow 相关能力

## [0.1.39] - 2026-07-08

### 修复
- **Agent 使用中点击卡死**：修复运行中的 agent 产生 subagent 事件时，点击活跃会话、subagent 列表或归档 subagent 可能卡死的问题；后端缩短 `active_subagents` 锁持有时间，避免持锁读取 transcript、解析路径或更新 conversation map
- **Subagent 归档稳定性**：归档已完成 subagent 时仅隐藏已完成项，保留运行中的 sibling，并防止重复点击归档按钮触发并发请求

## [0.1.38] - 2026-07-06

### 修复
- **Windows Cursor Hook**：自动修复 legacy hook 命令与缺失事件，保留用户自定义 hook 配置
- **Cursor Token 计数**：lifecycle hook 已上报用量的会话不再从 `stop` 重复采集
- **Codex 会话详情**：修复 transcript 加载卡死；增量读取 transcript，避免每次全量扫描历史

## [0.1.37] - 2026-07-06

### 修复
- **Subagent 列表卡死**：进一步修复 subagent 列表展开/刷新时的 UI 冻结；后端 snapshot 增量合并 subagent 数据，前端减少全量重渲染
- **Hook 稳定性**：加固 hook bridge 与 runner 部署校验，限制 transcript 路径信任范围，启动时自动刷新过期的 hook 脚本与 bridge 模块
- **Cursor 会话详情**：已知 transcript 路径时直接加载 transcript，避免误走 session 解析；并发加载去重，减少重复请求
- **Cursor Token 计数**：扩展 token 字段别名解析；未安装 `afterAgentResponse` hook 时从 `stop` 回退采集用量
- **Cursor Hook 升级**：启动时自动补齐缺失的 lifecycle hooks（如 `afterAgentResponse`）
- **Windows micro 岛**：最后一个活跃会话消失后隐藏 listener dot，避免无会话时仍显示在线指示

### 改进
- **Release CI**：Release workflow 增加测试步骤，修复 `Fix-Atoll.command` shebang 与 release notes 截断问题

## [0.1.36] - 2026-07-06

### 修复
- **Subagent 列表卡死**：修复 subagent 列表展开/刷新时 UI 冻结的问题；后端 snapshot 构建改为增量合并 subagent 数据，前端避免全量重渲染

## [0.1.35] - 2026-07-03

### 修复
- **Token 计数膨胀**：修复重启或 transcript 重扫后今日 token 被重复累加（可膨胀至数十亿）的问题；Cursor `stop` 与 `afterAgentResponse` 双 hook 重复计数；启动时自动修复历史文件中已膨胀的 `usage` 记录
- **悬停崩溃/卡死**：修复 token 聚合逻辑引入的 `absolute_token_sessions` 死锁，鼠标移入岛时主线程冻结的问题
- **Token 计数偶发清零**：transcript 全量扫描改用 max 合并，token 历史文件原子写入并保留 `.bak` 备份，运行时 baseline 同步更新，热力图今日数据取 max，避免持久化总量被意外覆盖
- **Windows Hook 安装**：Codex / Cursor hook 改用 PowerShell 启动器与稳定部署目录，修复含空格或非 ASCII 用户路径下 cmd 解析失败；同步 Codex hook 缓存与 trust 状态，安装时 tolerate runner 文件锁定，并在 UI 中展示安装错误
- **Cursor agent tab 背景**：修复 expanded 顶栏中 Cursor agent 选中 tab 背景不可见的问题

## [0.1.34] - 2026-06-28

### 修复
- **Header dead agent logo**：修复某个 agent 的 hook 未正确安装、已卸载或 drift 后，顶栏 logo 不切换为对应 agent 死掉 mascot 的问题；通过 localStorage 记录用户曾配置的 agent，避免从未安装的其他 agent 误报断开

## [0.1.33] - 2026-06-26

### 修复
- **Windows 岛展开置顶**：修复 Windows 上岛从 micro/compact 展开为全面板时未保持在所有窗口最上方的问题。展开前重新应用 `WS_EX_TOPMOST`，避免无焦点窗口在大尺寸 resize/重定位过程中被其他窗口覆盖
- **Cursor 活跃会话显示回归**：修复 v0.1.32 导致 Win / Mac 上 Cursor 活跃会话无法正常显示的问题。`has_atoll_cursor_hooks` 改为只要求 v0.1.31 的 5 个核心事件（`preToolUse`/`postToolUse`/`stop`/`subagentStart`/`subagentStop`）即视为已安装，新增的 Composer lifecycle hooks 作为可选增强，不再让旧版 hooks 配置被误判为未安装而触发离线/断连状态
- **Windows micro 岛 hover 闪烁**：修复 hover 展开前岛先缩至 micro 再展开的闪烁问题；指针重新进入窗口时立即上报 cursor-over 并取消 pending shrink

## [0.1.32] - 2026-06-26

### 改进
- **Cursor Composer hooks**：注册 `beforeSubmitPrompt`、`afterAgentThought` 等 lifecycle 事件，覆盖 Agent / Ask 等模式下的会话活跃度与 thinking 阶段

### 修复
- **Cursor Ask 模式**：修复只读问答模式下 session 列表为空、token 计数器不可用的问题（依赖 `sessionStart` / `afterAgentResponse` 等 observer hooks，无需工具调用）
- **Windows WSL**：Hook bridge 增加备用 fallback 端口，改善 WSL 环境下连接失败

## [0.1.31] - 2026-06-26

### 新增
- **Cursor IDE 支持**：Hook 集成、像素风 Cursor mascot、会话 transcript 解析、Token 计数、subagent 追踪，以及顶栏 **Open Cursor** 一键跳回 IDE
- Cursor `preToolUse` 事件自动放行（Cursor 自带权限 UI，Atoll 负责会话追踪与用量统计）
- 解析 `workspace_roots`，准确识别 Cursor 会话工作区路径

### 改进
- 优化会话切换时的导航竞态
- Cursor mascot 在 compact / micro / expanded 各布局下与 Claude、Codex 尺寸对齐

### 修复
- **Windows 折叠态**：修复从 session 子视图点击 Open Cursor（及 Open Claude / Open Codex）后错误进入超级折叠态（micro）的问题
- **Cursor Token 计数**：修复应用重启后累积 token 不再增长的问题
- **Subagent 聊天记录**：修正 transcript 路径解析，加载失败时显示明确提示
- **Cursor 性能**：Hook 快照防抖与缓存，减少 IDE 卡顿

## [0.1.30] - 2026-06-25

### 修复
- **Token 持久化**：修复版本更新后累积 token 数被清零的问题；历史文件写入改用 component-wise max 合并策略，防止重启后空状态覆盖已有数据；同时增强反序列化容错，缺失字段不再导致整个历史文件丢失

## [0.1.29] - 2026-06-25

### 修复
- **开机自启动**：macOS 改用 SMAppService 注册登录项，修复重启后未自动启动的问题；自动清理旧 LaunchAgent 中指向 dev 二进制的不正确配置

## [0.1.28] - 2026-06-25

### 新增
- **开机自启动**：Settings → General 新增 Launch at login 开关，支持 macOS 与 Windows 登录后自动启动 Atoll

## [0.1.27] - 2026-06-25

### 新增
- **Subagent 列表视图**：会话内可查看全部 subagent，支持状态、时间与 last message 展示，点击进入详情
- **Subagent 显示上限与批量归档**：Settings 可调显示数量，会话行一键归档已完成 subagent

### 改进
- 优化会话列表中 subagent chips 与归档按钮排版，新增「查看全部」入口

### 修复
- 修复重启后每日 token 计数器未持久化的问题
- 修复大量 subagent 并发时界面卡顿冻结

## [0.1.26] - 2026-06-24

### 新增
- **Plan 模式**：支持 plan 审批流程、子 agent 生命周期与紧凑布局稳定性
- **Build 审批卡片**：展示 plan Markdown 预览

### 改进
- 优化 subagent UI 导航与身份样式

### 修复
- 修复从 session 子视图折叠（如点击 Open Claude、跳转浏览器）后折叠态宽度异常变窄
- 修复 plan 中 Reply freely 输入框点击后无法输入
- 修复折叠完成后 session 图标与 token 计数器被错误隐藏的问题

## [0.1.25] - 2026-06-24

### 新增
- **Token 热力图**：持久化每日 token 用量，支持从展开态计数器进入查看
- **Agent 占比饼图**与 **30 天趋势图**，展示各 agent 用量分布与近期走势

### 改进
- 无活跃会话时展开态 token 计数器常驻显示，便于随时查看热力图
- 会话归档后仍保留当日 agent 归类，避免饼图误显示为 Other

### 修复
- 修复 token 历史同步时的死锁导致岛无法显示
- 修复热力图今日格子缺失、返回导航错误、布局被挤压等问题

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

[0.1.29]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.29
[0.1.28]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.28
[0.1.27]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.27
[0.1.26]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.26
[0.1.25]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.25
[0.1.24]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.24
[0.1.23]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.23
[0.1.22]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.22
[0.1.21]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.21
[0.1.20]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.20
[0.1.19]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.19
[0.1.18]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.18
[0.1.17]: https://github.com/sheepbooy/Atoll/releases/tag/v0.1.17
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
