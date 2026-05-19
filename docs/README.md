# lorum 文档

> 跨 AI CLI 统一 harness 项目文档
> 最后更新: 2026-05-19

---

## 目录

- [命令参考](#命令参考) — lorum CLI 完整命令说明
- [工具配置详情](tools/) — 各 AI CLI/IDE 的配置方式详解
- [架构决策](architecture/decisions.md) — 调研关键发现
- [路线图](architecture/roadmap.md) — 分阶段实现建议

---

## 项目概览

lorum 是一个跨 AI CLI 的统一配置管理工具（harness），目标是将多个 AI 编程助手（Claude Code、OpenAI Codex、kimi CLI、Cursor、Windsurf 等）的配置统一管理，实现 MCP 服务器、Hooks、Skills、Rules 的跨平台同步。

### 支持的工具（10 个）

| CLI/IDE | 类型 | MCP | Hooks | Skills | Rules |
|---------|------|-----|-------|--------|-------|
| Claude Code | CLI | ✅ | ✅（18+ 事件） | ✅ | ✅ |
| OpenAI Codex | CLI | ✅ | ✅ | ✅ | ✅ |
| Proma | CLI | ✅ | ❌ | ✅ | ❌ |
| kimi CLI | CLI | ✅ | ✅（Beta） | ✅ | ✅ |
| opencode | CLI | ✅ | ✅ | ✅ | ✅ |
| trae | IDE | ✅ | ❌ | ✅ | ✅ |
| Cursor | IDE | ✅ | ✅ | ✅ | ✅ |
| Windsurf | IDE | ✅ | ✅ | ✅ | ✅ |
| Continue.dev | IDE 插件 | ⚠️ 未确认 | ❌ | ❌ | ❌ |
| Aider | CLI | ❓ 存疑 | ❌ | ❌ | ❌ |

### 配置映射速查表

| CLI | MCP 位置 | MCP Schema | Hooks | Skills | Rules | 配置文件 |
|-----|---------|-----------|-------|--------|-------|---------|
| Claude Code | `~/.claude/settings.json` | `mcpServers` | `settings.json` hooks（对象，18+ 事件） | `~/.claude/skills/` | `CLAUDE.md` | `settings.json` |
| Codex | `~/.codex/config.json` | `mcpServers` | `.codex/hooks.json` + `~/.codex/hooks.json` | `~/.codex/skills/` | `.codex/rules.md` | `config.json` |
| Proma | `~/.proma/mcp.json` | `servers` | 无 | `~/.proma/agent-workspaces/lorum/skills/` | 无 | `config.json` |
| kimi | `~/.kimi/config.toml` | `mcp.client` + `kimi mcp add` | TOML `[[hooks]]`（Beta） | `~/.kimi/skills/` | `AGENTS.md` | `config.toml` |
| opencode | `~/.config/opencode/opencode.json` | `mcp`（local/remote） | `~/.config/opencode/hooks.json`（实验性） | `~/.config/opencode/skills/` | `AGENTS.md` | `opencode.json` |
| trae | `.trae/mcp.json` + UI | `mcpServers` | 无 | `~/.trae/skills/` | `.trae/rules/project_rules.md` | `settings.json` |
| Cursor | `.cursor/mcp.json` | `mcpServers` | `.cursor/hooks.json` + `~/.cursor/hooks.json` | `~/.cursor/skills/` | `.cursorrules` | — |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` | `mcpServers` + SSE | `.windsurf/hooks.json` + `~/.codeium/windsurf/hooks.json` | `~/.codeium/windsurf/skills/` | `.windsurfrules` | `mcp_config.json` |
| Continue.dev | 未确认 | 未确认 | 无 | 无 | 无 | `config.json` |
| Aider | 存疑 | 存疑 | 无 | 无 | 无 | `.aider.conf.yml` |

---

## 命令参考

### 全局选项

| 选项 | 说明 |
|------|------|
| `--config <path>` | 指定自定义配置文件路径 |
| `--verbose` | 启用详细输出 |
| `--no-welcome` | 未创建配置时跳过欢迎提示 |

### 核心命令

#### `lorum init`

初始化 lorum 配置。

| 选项 | 说明 |
|------|------|
| `--local` | 创建项目级配置（`.lorum/config.yaml`）而非全局配置 |
| `--yes` | 跳过交互式提示，自动导入已检测到的工具 |

#### `lorum import --from <tool>`

从已有 AI 工具导入配置到 lorum 统一配置。支持导入 MCP 服务器、Hooks 和 Rules。

| 选项 | 说明 |
|------|------|
| `--from <tool>` | 源工具名称，如 `claude-code`、`cursor`、`windsurf`；`all` 表示全部 |
| `--dry-run` | 预览导入结果，不写入任何文件 |

#### `lorum sync`

将 lorum 配置同步到各工具的配置文件。默认仅同步 MCP 维度。

| 选项 | 说明 |
|------|------|
| `--dry-run` | 预览变更，不写入文件 |
| `--tools <tool1> <tool2>...` | 仅同步指定工具（默认全部） |
| `--expand-env` | 同步前展开环境变量引用 |
| `--mcp` | 同步 MCP 服务器配置 |
| `--hooks` | 同步生命周期钩子 |
| `--skills` | 同步 Skills 目录 |
| `--rules` | 同步 Rules 文件 |
| `--all` | 同步全部四个维度（MCP + Hooks + Skills + Rules） |

#### `lorum check`

验证当前配置的有效性，检查 MCP 命令可用性、未设置的环境变量引用、Hooks 事件名规范等。

#### `lorum status`

显示每个注册工具在各维度的配置数量概览。输出格式：

```
TOOL               MCP  RULES  HOOKS  SKILLS
claude-code          3      ·     18      5
cursor               2      4      ·      ·
```

`·` 表示该维度已适配但当前无内容，`-` 表示该工具不支持此维度。

#### `lorum doctor [--tools ...]`

运行综合健康检查，检测各工具配置文件的一致性、格式错误、路径问题等。

### 配置查看

#### `lorum config`

输出解析后的配置内容。

| 选项 | 说明 |
|------|------|
| `--resolve-env` | 输出中解析环境变量为实际值 |
| `--local` | 仅显示项目级配置 |
| `--global` | 仅显示全局配置 |
| `--format yaml\|json` | 输出格式（默认 yaml） |

### 备份管理

#### `lorum backup <action>`

| 子命令 | 说明 |
|--------|------|
| `list` | 列出可用备份 |
| `create [tools...] [--all]` | 为指定工具或全部工具创建备份 |
| `restore <tool> [--backup <file>]` | 从备份恢复指定工具配置 |

### 维度子命令

#### `lorum mcp <action>`

管理 MCP 服务器条目。

| 子命令 | 说明 |
|--------|------|
| `add <name> --command <cmd> [--args ...] [--env ...]` | 添加服务器 |
| `remove <name>` | 删除服务器 |
| `list` | 列出所有服务器 |
| `edit <name> [--command <cmd>] [--args ...] [--env ...]` | 编辑服务器 |

#### `lorum rule <action>`

管理项目级 Rules（`.lorum/RULES.md`）。

| 子命令 | 说明 |
|--------|------|
| `init` | 创建空的 `.lorum/RULES.md` 模板 |
| `add <name> --content <text>` | 添加规则段落 |
| `remove <name>` | 删除规则段落 |
| `edit <name> --content <text>` | 编辑规则段落 |
| `list` | 列出所有段落名 |
| `show [name]` | 显示规则内容（省略 name 显示全部） |
| `sync [--dry-run] [--tools ...]` | 同步规则到各工具的 rules 文件 |
| `import --from <tool>` | 从工具导入规则到 `.lorum/RULES.md` |

#### `lorum hook <action>`

管理生命周期钩子。

| 子命令 | 说明 |
|--------|------|
| `add <event> --matcher <pattern> --command <cmd> [--timeout <s>] [--handler-type <type>]` | 添加钩子处理器 |
| `remove <event> [--matcher <pattern>]` | 删除钩子（省略 matcher 删除整个事件） |
| `list` | 列出所有配置钩子 |
| `sync [--dry-run] [--tools ...]` | 同步钩子到支持的工具 |

#### `lorum skill <action>`

管理 Skills（AI 技能目录）。

| 子命令 | 说明 |
|--------|------|
| `list` | 列出统一目录中的所有技能 |
| `show <name>` | 查看技能内容 |
| `add <name> --from <dir>` | 导入技能目录到统一存储 |
| `remove <name>` | 删除技能 |
| `sync [--dry-run] [--tools ...]` | 同步技能到支持的工具 |

---

## 审核说明

本文档原始内容经过 WebSearch 交叉验证，修正了以下问题：

- Claude Code Hooks 的 schema 结构（对象而非数组，支持 18+ 事件和多种 handler 类型）
- kimi CLI 状态从 experimental 改为正式发布，配置格式为 TOML
- opencode 配置文件名和位置修正（`~/.config/opencode/opencode.json`）
- trae MCP 配置位置修正为 `.trae/mcp.json`
- deepwiki 上 kimi 信息严重过时（YAML 格式 vs 实际 TOML，旧模型名 vs 实际 kimi-for-coding）

**近期更新（2026-05-19）**：

- 扩展 Rules 适配器覆盖到 7 个工具（新增 claude-code、codex、kimi、opencode、trae）
- 扩展 Hooks 适配器覆盖到 6 个工具（新增 codex、cursor、windsurf、opencode）
- 扩展 Skills 适配器覆盖到 8 个工具（新增 codex、cursor、kimi、opencode、trae、windsurf）
- 实现跨维度统一同步（`lorum sync --all` 同时同步 MCP + Hooks + Skills + Rules）

## 数据来源

- 官方文档: Claude Code Docs、OpenAI Codex GitHub、kimi-cli 官方文档 (moonshotai.github.io)、opencode.ai、trae docs、Windsurf docs
- 社区资源: GitHub 仓库、技术博客、Lobehub Skills Marketplace
- 交叉验证: WebSearch 多源对比
