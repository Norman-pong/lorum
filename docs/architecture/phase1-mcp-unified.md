# lorum Phase 1: MCP 统一管理

> 状态: 规划中
> 决策日期: 2026-05-11

---

## 1. 核心决策记录

以下决策基于场景推演（4 个独立 Agent 模拟真实使用流程）后确认：

| 决策项 | 方案 | 理由 |
|--------|------|------|
| **同步策略** | 完全覆盖 (Full Overwrite) | lorum 作为配置唯一来源。同步时，目标工具的 MCP 配置完全替换为 lorum 的内容 |
| **环境变量** | 混合策略 | 默认不展开 `${VAR}`，保留原样写入；提供 `--expand-env` 显式展开 |
| **项目级配置** | 合并叠加 (Merge Overlay) | 全局 + 项目级合并，项目级同名服务器优先；支持 `exclude` 禁用全局服务器 |
| **MCP CRUD** | Phase 1 实现 | `lorum mcp add/remove/edit/list` CLI 命令，直接读写配置文件 |
| **Hooks CRUD** | Phase 3 实现 | `lorum hook add/remove/list` CLI 命令 |
| **Skills CRUD** | Phase 4 实现 | `lorum skill add/remove/list/show` CLI 命令 + 文件系统同步 |

---

## 2. 配置管理架构

### 2.1 配置文件位置

| 级别 | 路径 | 用途 |
|------|------|------|
| 全局 | `~/.config/lorum/config.yaml` (XDG) | 个人通用配置，所有项目共享 |
| 项目级 | `.lorum/config.yaml` | 项目特定配置，覆盖/扩展全局 |
| 显式指定 | `--config <path>` | 完全替代（不读取全局/项目级） |

### 2.2 合并规则

生效配置 = 全局配置 与 项目级配置 合并，项目级同名服务器覆盖全局同名服务器，服务器属性深合并。项目级可显式禁用全局服务器（通过 `exclude` 机制）。

全局配置示例：
```yaml
version: "1"
mcp:
  servers:
    fetch:  { command: npx, args: ["-y", "@modelcontextprotocol/server-fetch"] }
    github: { command: npx, args: ["-y", "@modelcontextprotocol/server-github"] }
```

项目级配置示例：
```yaml
version: "1"
mcp:
  servers:
    internal-docs:
      command: python
      args: ["/opt/company-a/mcp-docs.py"]
  exclude: [fetch]
```

生效配置：`github`（全局） + `internal-docs`（项目级），`fetch` 被排除。

### 2.3 配置查找逻辑

1. 若指定 `--config`：使用该文件（不读全局/项目级）
2. 否则：读取全局配置，从当前目录向上查找 `.lorum/config.yaml`，合并（项目级优先）

### 2.4 统一配置 Schema

```yaml
version: "1"

mcp:
  servers:
    fetch:
      command: npx
      args: ["-y", "@modelcontextprotocol/server-fetch"]
      env:
        KEY: "${VALUE}"
  exclude: []

hooks:
  pre-tool-use:
    - matcher: "Bash"
      command: "scripts/safety-check.sh"
      timeout: 60

skills:
  code-review:
    description: "Run comprehensive code review"
    instructions: |
      Review the code...
```

> **Phase 3 Hooks 注意事项**: Claude Code hooks 使用 PascalCase 事件名（如 `PreToolUse`、`PostToolUse`）、`matcher` 字段匹配工具名，支持 5 种 handler 类型（`command`、`http`、`mcp_tool`、`prompt`、`agent`）。统一 schema 中使用 kebab-case 事件名和 `matcher` 字段，输出时由适配器翻译为目标工具的实际格式。

---

## 3. 适配器设计

### 3.1 分层 Adapter

- **ToolAdapter**: 每个工具一个，声明支持哪些配置类型
- **ConfigAdapter**: 每种配置类型一个读写实现，按类型独立调度

### 3.2 各工具支持矩阵

| 工具 | MCP | Hooks | Skills |
|------|-----|-------|--------|
| Claude Code | yes | yes | yes |
| OpenAI Codex | yes | no | no |
| Proma | yes | no | yes |
| kimi | yes | yes (降级) | no |
| trae | yes | no | no |

### 3.3 各工具映射详情

| 工具 | 配置路径 | 字段名 | 格式 |
|------|---------|--------|------|
| Claude Code | `~/.claude/settings.json`（全局）、`.claude/settings.json`（项目级）、`.claude/settings.local.json`（本地，gitignored） | `mcpServers` | JSON (嵌套) |
| OpenAI Codex | `~/.codex/config.toml`（全局）、`.codex/config.toml`（项目级） | `mcp_servers` | TOML |
| Proma | `~/.proma/mcp.json` | `servers` | JSON (独立) |
| kimi | `~/.kimi/config.toml` | `[mcp.client]` | TOML |
| trae | `.trae/mcp.json` | `mcpServers` | JSON (项目级) |

### 3.4 同步策略（完全覆盖）

lorum 作为配置的唯一来源。同步时读取目标工具完整配置，替换 MCP/Hooks 相关字段为 lorum 的内容，保留目标工具中所有其他字段，写回文件。

---

## 4. 备份机制

- 完全覆盖模式前自动备份
- 备份路径: `~/.config/lorum/backups/<tool>-<timestamp>.<ext>`
- 保留最近 10 个备份，超出自动清理
- 命令: `lorum backup list/restore/create`

---

## 5. CLI 设计

### 5.1 命令结构

```
lorum
├── init              # 初始化配置
├── import            # 从已有工具导入
├── sync              # 同步配置到各工具
├── check             # 检查配置有效性
├── status            # 显示各工具状态
├── config            # 查看生效配置（支持 --resolve-env / --local / --global）
├── backup            # 备份管理
├── mcp               # MCP CRUD (Phase 1)
│   ├── add
│   ├── remove
│   ├── edit
│   └── list
├── hook              # Hooks CRUD (Phase 3)
│   ├── add
│   ├── remove
│   └── list
└── skill             # Skills CRUD (Phase 4)
    ├── add
    ├── remove
    ├── list
    └── show
```

### 5.2 核心命令示例

```bash
lorum init                              # 创建全局配置
lorum import --from claude              # 从 Claude Code 导入
lorum sync                              # 同步 MCP 到所有工具
lorum sync --mcp --tools claude,proma   # 仅同步 MCP 到指定工具
lorum sync --expand-env                 # 展开环境变量后同步
lorum sync --dry-run                    # 预览变更
lorum check                             # 检查配置有效性
lorum status                            # 显示各工具状态
lorum config                            # 查看生效配置（合并后）
lorum config --resolve-env              # 显示生效配置并解析环境变量
lorum config --local                    # 仅显示项目级配置
lorum config --global                   # 仅显示全局配置
lorum mcp add notion --command npx --args "-y" --args "@notion/mcp-server"
lorum mcp remove notion
lorum mcp list
```

---

## 6. 环境变量处理

默认行为：保留 `${VAR}` 原样写入目标工具配置。

显式展开：`lorum sync --expand-env`

- `${VAR}` 从当前 shell 环境读取
- 变量不存在时保留原字符串
- 对不支持变量语法的工具（如 Claude Code JSON）给出 WARN

---

## 7. 首次使用流程

当检测到没有配置文件时，运行 `lorum` 显示引导信息：

```
Welcome to lorum! You haven't created a configuration file yet.

Quick start:
  lorum init              Create initial config
  lorum init --local      Create project-level config
  lorum import --from all Import from existing tools
```

`lorum init` 交互流程：检测已安装工具 -> 询问是否导入 -> 创建配置 -> 提示运行 `lorum sync`

---

## 8. 输出设计

### `lorum sync` 输出

```
Config type: MCP
Strategy: overwrite (with backup)

Tool         Status    Details
──────────── ───────── ─────────────────────────────────────
claude       synced    backup: ~/.config/lorum/backups/claude-20260511-143022.json
codex        synced    backup: ~/.config/lorum/backups/codex-20260511-143022.toml
kimi         synced    backup: ~/.config/lorum/backups/kimi-20260511-143022.toml
proma        synced    backup: ~/.config/lorum/backups/proma-20260511-143022.json
trae         error     .trae/mcp.json not found (project-level config)
```

### `lorum status` 输出

```
Tool         MCP    Hooks   Skills   Config Path
──────────── ────── ─────── ──────── ──────────────────────────────
claude       yes    yes     yes      ~/.claude/settings.json
codex        yes    no      no       ~/.codex/config.toml
kimi         yes    yes     no       ~/.kimi/config.toml
proma        yes    no      yes      ~/.proma/mcp.json
trae         yes    no      no       .trae/mcp.json
```

---

## 9. 项目结构

```
lorum/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI 入口
│   ├── commands/            # CLI 子命令
│   │   ├── mod.rs
│   │   ├── init.rs
│   │   ├── import.rs
│   │   ├── sync.rs
│   │   ├── check.rs
│   │   ├── status.rs
│   │   ├── config.rs
│   │   ├── backup.rs
│   │   ├── mcp.rs
│   │   ├── hook.rs
│   │   └── skill.rs
│   ├── config.rs            # 配置 schema + 解析 + 合并
│   ├── sync.rs              # 同步引擎
│   ├── output.rs            # 表格输出
│   ├── backup.rs            # 备份管理
│   ├── env_interpolate.rs   # 环境变量插值
│   ├── adapters/
│   │   ├── mod.rs
│   │   ├── claude.rs
│   │   ├── codex.rs
│   │   ├── kimi.rs
│   │   ├── proma.rs
│   │   └── trae.rs
│   └── error.rs             # 统一错误类型
├── tests/
│   ├── common/
│   │   └── mod.rs
│   └── integration_sync.rs
└── examples/
    └── config.yaml
```

---

## 10. 依赖策略

| 用途 | Crate |
|------|-------|
| CLI | `clap` |
| 序列化 | `serde` + `serde_yaml` |
| TOML | `toml`（Codex、kimi 均使用 TOML 配置） |
| 路径 | `dirs` |
| 表格输出 | 手写字串格式化 |
| 错误处理 | `thiserror` |
| 测试 | `tempfile` |

---

## 11. 已知风险与缓解

| 风险 | 缓解措施 |
|------|---------|
| kimi TOML 结构不确定 | 已验证路径 `~/.kimi/config.toml`，项目级通过 `--mcp-config-file` CLI 选项指定（非文件系统约定） |
| Codex 配置为 TOML 格式 | 已验证路径 `~/.codex/config.toml`，字段名 `mcp_servers`（snake_case），项目级 `.codex/config.toml` |
| kimi 项目级 MCP 仅支持 CLI 选项 | kimi 不支持项目级配置文件发现，仅通过 `--mcp-config-file` 指定，lorum 同步仅覆盖全局配置 |
| 环境变量明文写入 | 默认保留 `${VAR}`，显式 `--expand-env` |
| 同步失败导致配置损坏 | 自动备份 + 独立工具执行 |
| 项目级配置合并歧义 | `exclude` 机制 + `lorum config` 可见性 |
| 目标工具配置 schema 变更 | 版本化 schema 检测（后续迭代） |
