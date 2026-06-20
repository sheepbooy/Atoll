<p align="center">
  <img src="docs/assets/logo.svg" alt="Atoll" width="64" height="72" />
</p>

<h1 align="center">Atoll</h1>

<p align="center">
  <strong>macOS 菜单栏里的 AI 权限审批浮岛</strong><br/>
  <sub>Claude Code / Codex 发起权限请求时，不用切窗口，一眼批准或拒绝</sub>
</p>

<p align="center">
  <a href="https://github.com/sheepbooy/Atoll/releases"><img src="https://img.shields.io/github/v/release/sheepbooy/Atoll?style=flat-square&color=38BDD8&label=release" alt="Release" /></a>
  <img src="https://img.shields.io/badge/license-MIT-9ee9b8?style=flat-square" alt="MIT" />
  <img src="https://img.shields.io/badge/platform-macOS%20(Apple%20Silicon)-111317?style=flat-square&logo=apple&logoColor=white" alt="macOS" />
  <img src="https://img.shields.io/badge/privacy-100%25%20local-b2ea7e?style=flat-square" alt="100% local" />
</p>

<p align="center">
  <a href="#安装">安装</a> ·
  <a href="#接入-agent">接入 Agent</a> ·
  <a href="#视觉">视觉</a> ·
  <a href="#开发">开发</a> ·
  <a href="#路线图">路线图</a>
</p>

<br/>

<p align="center">
  <img src="docs/assets/demo.gif" alt="Atoll demo: 权限审批、置顶、归档" width="560" />
</p>

<p align="center">
  <em>权限请求 → 展开审批 → 置顶 / 归档 → 自动收回</em>
</p>

---

## 是什么

**Atoll** 是一个轻量 macOS 桌面应用，住在菜单栏 / 刘海区域：

- **平时** — 紧凑胶囊，显示在线状态、活跃会话、待审批数量
- **有请求时** — 自动展开，展示命令详情，一键 **Approve / Deny / Always**
- **全程本地** — Hook 桥接 `127.0.0.1:47777`，数据不出本机

目前支持 **Claude Code** 和 **Codex CLI**（Apple Silicon）。

---

## 安装

**推荐 — 一行命令：**

```bash
curl -fsSL https://raw.githubusercontent.com/sheepbooy/Atoll/main/scripts/install.sh | bash
```

指定版本：`ATOLL_VERSION=0.1.4 curl -fsSL .../install.sh | bash`

<details>
<summary>其他安装方式</summary>

**Homebrew**

```bash
brew tap sheepbooy/tap
brew install --cask --no-quarantine atoll
```

**手动下载** — 从 [Releases](https://github.com/sheepbooy/Atoll/releases) 下载 `Atoll-aarch64.dmg`，拖入 Applications 后执行：

```bash
sudo xattr -cr /Applications/Atoll.app
```

> 应用尚未公证。首次启动若被拦截，在 Applications 中右键 **Open** 一次即可。

</details>

---

## 接入 Agent

Atoll 通过应用内 **一键安装 Hook**，无需手动编辑配置文件。

<p align="center">
  <img src="docs/assets/setup.gif" alt="首次安装 Hook 全流程" width="560" />
</p>

| Agent | 安装路径 | 额外步骤 |
| --- | --- | --- |
| **Claude Code** | 菜单 → Settings → Agent hooks → Install | 无 |
| **Codex** | 同上 → Install Codex | 安装后在 Codex 运行 `/hooks` 并信任 Atoll hook |

Hook 注册 `PermissionRequest`、`PostToolUse`、`Stop` 等事件，写入 `~/.claude/settings.json` 或 `~/.codex/hooks.json`。

卸载：Settings → Agent hooks → Uninstall。

> 自定义端口：设置环境变量 `ATOLL_HOOK_URL`。

### 快捷键

| 操作 | 按键 |
| --- | --- |
| Approve | `Enter` |
| Deny | `Delete` |
| Always approve | `Shift` + `Enter` |

---

## 视觉

### Atoll Logo 状态

菜单栏 Logo 反映 App 全局状态（与 Agent 无关）：

<p align="center">
  <img src="docs/assets/atoll-states.png" alt="Atoll Logo: 空闲 / 待审批 / 工作中 / 离线" width="560" />
</p>

| 状态 | 含义 |
| --- | --- |
| 空闲 | 在线监听，无 session、无 pending |
| 待审批 | 有权限请求等待处理 |
| 工作中 | 有活跃 Agent 会话 |
| 离线 | Hook 未就绪或 bridge 不可达 |

### Agent 形象

每个 Agent 有独立的像素风 mascot；Gemini 复用 Clawd 造型并着色为绿色：

<p align="center">
  <img src="docs/assets/agent-mascots.png" alt="Claude / Codex / Gemini mascots" width="420" />
</p>

### 多 Session 与终端

<p align="center">
  <img src="docs/assets/session-overview.png" alt="多 session 菜单栏 + Claude Code 终端" width="92%" />
</p>

---

## 开发

```bash
npm install          # 安装依赖
npm run tauri dev    # 启动桌面应用（需 Rust）
npm test             # 运行测试
npm run tauri build  # 打包
```

<details>
<summary>项目结构 & 文档素材</summary>

```
src/                          React 浮岛 UI
src-tauri/src/hook_bridge.rs  本地 HTTP 桥接（Claude + Codex）
src-tauri/src/transcript.rs   JSONL 会话 & Token 解析
scripts/atoll-*-hook.mjs      Hook shim（随应用分发）
```

重新生成 README 截图（macOS）：

```bash
npm run capture:media    # 浮岛 UI 截图 + demo GIF
npm run export:brand     # Logo 状态 + Agent 形象
```

发布新版本：

```bash
./scripts/release.sh 0.2.0
```

</details>

---

## 路线图

- [ ] Apple 签名 & 公证、Intel 构建
- [ ] Gemini / Cursor 等更多 Agent 适配
- [ ] 新请求自动展开、通知中心提醒
- [ ] 审批历史导出、会话搜索
- [x] Codex hook 适配

---

## License

MIT

<p align="center">
  <sub>Built for developers who live in the terminal but refuse to context-switch for every <code>y/n</code>.</sub>
</p>
