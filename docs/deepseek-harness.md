# DeepSeek Harness 接入说明

Atoll 通过 **Cordis 进程内插件** 接管 DeepSeek Harness（DSH）的审批请求。DSH 没有 Claude Code 那种外部命令 hook 管线，因此 Atoll 不写命令 hook，而是把桥接插件部署到 DSH 的共享 profile 模块目录，并在 DSH 的 home patch 中登记一行受管理配置。

> 当前为 M1：审批与会话在线。Token 统计、聊天记录、子 Agent 与问答/Plan 接管在后续阶段。

## 工作原理

1. Atoll 把 `@atoll/dsh-bridge` 部署到 `~/.dsh/profiles/node_modules/@atoll/dsh-bridge`（共享给所有 DSH profile）。
2. Atoll 在 `~/.dsh/cordis.patch.yml` 写入一条 marker 包起来的 `insert`：

   ```yaml
   # >>> Atoll DSH bridge (managed) >>>
   - insert:
       - id: atoll-bridge
         name: '@atoll/dsh-bridge'
   # <<< Atoll DSH bridge (managed) <<<
   ```

3. 重启 DSH 后，DSH 在带 `webServer` 的 Web/Desktop surface 加载插件；headless 与无 Web 的 TUI 不激活桥接逻辑。
4. DSH 触发 `approval/request` 时，插件把请求 POST 到 Atoll 的 `127.0.0.1:<port>/dsh/hook`，等待浮岛上的 Approve / Deny / Always。
5. Atoll 返回允许时插件回 `allowed-once`，拒绝时回 `rejected`。Atoll 不可达、返回异常或用户未处理时，插件调用 `next()`，DSH 自带审批继续工作。

插件只按需读取 `~/Library/Application Support/Atoll/bridge.json`（Windows 为 `%LOCALAPPDATA%\Atoll\bridge.json`）中的 `dshUrl` 与 token，因此 Atoll 重启或换端口后不需要重启 DSH。

## 安装

1. Settings → Agent hooks → **Install DeepSeek Harness**。
2. 完全退出并重新打开 DeepSeek Harness。home patch 只在启动时读取。
3. 在 DSH 中触发一次会要求审批的操作（最容易的是让 DSH 发起一次需要 sandbox escalation 的命令）。
4. 在 Atoll 中分别执行一次批准与拒绝，并回到 Settings → Agent hooks 点击「验证连接」确认真实事件。

安装会创建 `~/.dsh`、`profiles/node_modules` 等缺失目录；如果 DSH 尚未运行过，插件会在 DSH 首次启动时加载。

## Atoll 不在线时

- 插件读取不到 `bridge.json` 或连接失败时立即 `next()`，DSH 自带审批出现，不阻塞会话。
- Atoll 崩溃/退出后，正在等待的请求会以连接失败结束，同样回落到 DSH 原生审批。
- 插件不把离线请求写入 `~/.atoll/hook-spool`，因为 DSH 原生审批已经接管，Atoll 无法得知最终决定。

## 已知限制（M1）

- 不统计 DSH token，也不显示 DSH 会话聊天记录；会话列表只显示审批与在线状态。
- DSH 的 `rejected` outcome 不携带原因，Atoll 里填写的拒绝备注只进入审批历史，不会传给模型。
- 插件只在带 `webServer` 的 Web/Desktop surface 工作；headless、CI 与无 Web 的 TUI 保持原行为。
- 修改 home patch 后必须重启 DSH；安装/卸载 Atoll 不会自动重启 DSH。
- Atoll 更新桥接插件后，状态会提示重新安装并重启 DSH。

## 卸载

Settings → Agent hooks → **Uninstall**。Atoll 只删除自己 marker 包住的块和 `@atoll/dsh-bridge` 包；如果 patch 中存在手写的 `atoll-bridge` 条目，卸载会拒绝并提示手动处理，避免破坏 DSH 启动。卸载后同样需要重启 DSH。

## 故障排查

- **DSH 没有加载插件**：确认 `~/.dsh/cordis.patch.yml` 含有受管理块，且 `~/.dsh/profiles/node_modules/@atoll/dsh-bridge/index.mjs` 存在；完全退出 DSH 再启动。
- **状态显示冲突**：patch 中已有手写的 `atoll-bridge` 条目。删除或改名后重新安装。
- **Atoll 收不到审批**：确认 Atoll 正在运行、`bridge.json` 有 `dshUrl`；查看 `~/.atoll/dsh-bridge-invoke.log`（设置 `ATOLL_DSH_HOOK_DEBUG=1` 后记录详细错误）。
- **自定义 DSH_HOME**：Atoll 进程可见 `DSH_HOME` 时跟随；从 Finder 启动 Atoll 时通常看不到 shell 环境，仍按 `~/.dsh` 处理。
- **卸载后仍被拦截**：插件已加载在内存中。重启 DSH 后才会完全移除。

## 手工验收

1. 安装 → 重启 DSH → 触发 sandbox escalation；Atoll 卡显示命令、描述与 justification。
2. Approve：命令执行；Deny：DSH 模型收到拒绝。
3. 对同一会话选择 Always：下一次请求自动放行。
4. 退出 Atoll，再触发审批：DSH 原生审批出现，不阻塞。
5. 重新启动 Atoll（不重启 DSH），再次触发：Atoll 恢复接管。
6. 卸载并重启 DSH：patch 恢复为 `[]`，插件目录移除，不再拦截。
