# 架构决策记录 (ADR)

> 基于 2026-05-09 的跨工具配置调研

---

## ADR-001: MCP 字段名差异

**上下文**: 10 个工具中有 9 个支持 MCP，但字段名不统一。

**决策**: 在 lorum 统一配置中使用 `mcp.servers` 作为标准字段名，输出时根据目标工具做字段映射。

**映射表**:

| 工具 | 目标字段名 |
|------|-----------|
| Claude Code | `mcpServers` |
| OpenAI Codex | `mcp_servers` |
| Proma | `servers` |
| kimi | `[mcp.client]`（TOML） |
| opencode | `mcp` |
| trae | `mcpServers` |
| Cursor | `mcpServers` |
| Windsurf | `mcpServers` |

---

## ADR-002: Hooks 统一策略

**上下文**: Claude Code 支持 18+ 事件、5 种 handler 类型（`command`、`http`、`mcp_tool`、`prompt`、`agent`），使用 PascalCase 事件名（如 `PreToolUse`）和 `matcher` 字段；kimi 仅支持 Beta 版 `[[hooks]]` 数组（TOML）。

**决策**: 定义 lorum 统一 hooks 语法，以 Claude Code 的丰富语义为基准。输出到 kimi 时降级为支持的子集。

**统一语法草案**:
```yaml
hooks:
  pre-tool-use:
    - matcher: "Bash"
      command: "scripts/safety-check.sh"
      timeout: 60
  post-tool-use:
    - matcher: "Write|Edit"
      command: "cargo fmt"
```

> **注意**: 统一 schema 使用 kebab-case 事件名和 `matcher` 字段，输出时由适配器翻译为目标工具的实际格式（Claude Code 使用 PascalCase 事件名）。

---

## ADR-003: Rules 统一格式

**上下文**: Cursor (`.cursorrules`)、Windsurf (`.windsurfrules`)、Codex (`.codex/rules.md`) 都有项目级规则文件，但格式均为纯文本 Markdown，无统一 schema。

**决策**: lorum 使用单一 `RULES.md` 作为源文件，输出时根据目标工具生成对应的文件名（`.cursorrules`、`.windsurfrules` 等）。

---

## ADR-004: IDE 类工具特殊处理

**上下文**: Cursor、Windsurf、trae 都是基于 VS Code 的 IDE，配置路径和继承机制各不相同。

- Cursor: `.cursor/mcp.json`（项目级）
- Windsurf: `~/.codeium/windsurf/mcp_config.json`（全局）
- trae: `.trae/mcp.json`（项目级）+ VS Code settings

**决策**: IDE 类工具单独处理配置路径逻辑，不与其他 CLI 工具混用同一路由。

---

## ADR-005: Continue.dev 独立适配

**上下文**: Continue.dev 以 models/commands/providers 为核心，与其他工具的 MCP-centric 设计完全不同。

**决策**: Continue.dev 作为独立适配器，不纳入通用 MCP 同步流程。

---

## ADR-006: Aider 暂不支持

**上下文**: Aider 截至 2026-05 未确认支持 MCP。

**决策**: Aider 暂列为不支持，后续跟进其 MCP 集成进展。

---

## ADR-007: MCP 同步策略

**上下文**: 在场景推演中发现，若采用"智能合并"策略，从 lorum 删除的 MCP 服务器仍会残留在目标工具中；若采用"完全覆盖"策略，用户在各工具中手动添加的 MCP 服务器会被删除。

**决策**: 采用 **完全覆盖** 策略。lorum 作为配置的唯一来源，同步时目标工具的 MCP 配置完全替换为 lorum 的内容。配套自动备份机制保护数据安全。

**理由**:
- lorum 的核心价值就是"单一数据源"，如果保留目标工具中的非 lorum 配置，lorum 就退化为"增量添加工具"
- 自动备份机制（ADR-010）消除了数据丢失风险
- 与 Terraform 等声明式管理工具的哲学一致

---

## ADR-008: 环境变量处理

**上下文**: 统一配置中支持 `${GITHUB_TOKEN}` 语法，但 Claude Code JSON 配置和 Codex TOML 配置均不原生支持变量展开。

**决策**: 采用 **混合策略** — 默认保留 `${VAR}` 原样写入；提供 `--expand-env` 显式选项展开为实际值。

**理由**:
- 保留原样写入更安全，敏感信息不会明文分散在多个配置文件中
- 对不支持变量语法的工具给出 WARN，让用户自行决定
- 显式展开是强意图表达，避免意外泄露敏感信息

---

## ADR-009: 项目级配置合并

**上下文**: 用户在不同项目需要不同的 MCP 服务器组合（如公司项目需要内部 MCP，个人项目不需要）。

**决策**: 采用 **合并叠加 (Merge Overlay)** 策略。全局配置 + 项目级 `.lorum/config.yaml` 合并，项目级同名服务器优先；支持 `exclude` 列表禁用全局中的特定服务器。

**合并规则**:
```
生效配置 = 全局配置 与 项目级配置 合并
          项目级同名服务器 覆盖 全局同名服务器
          服务器属性 深合并（env、args 等合并而非替换）
          项目级可显式禁用全局服务器（通过 exclude 机制）
```

**理由**:
- 用户天然期望"项目特有配置叠加到个人通用配置之上"
- `exclude` 机制提供了禁用全局服务器的明确手段
- `--config` 采用完全替代语义（不读全局/项目级），满足显式控制需求

---

## ADR-010: 配置备份机制

**上下文**: 完全覆盖策略下，同步失败或误操作可能导致配置丢失。

**决策**: sync 前 **自动备份** 目标工具的现有配置。备份路径: `~/.config/lorum/backups/<tool>-<timestamp>.<ext>`，保留最近 10 个。

**配套命令**:
- `lorum backup list` — 查看备份历史
- `lorum backup restore <tool>` — 恢复到指定备份
- `lorum sync --dry-run` — 预览变更，不实际写入

---

## ADR-011: CLI CRUD 命令

**上下文**: 场景推演发现，纯手动编辑 YAML 体验差（缩进敏感、无自动补全、编辑-验证-同步循环繁琐）。

**决策**: Phase 1 实现 MCP 的 CLI CRUD 命令：`lorum mcp add/remove/edit/list`。

**分工**:
- Phase 1: `lorum mcp add/remove/edit/list`
- Phase 3: `lorum hook add/remove/list`
- Phase 4: `lorum skill add/remove/list/show`

**理由**:
- CLI 命令比手动编辑 YAML 更可靠（参数校验、格式保证）
- 与 Claude Code 的原生命令 `claude mcp add` 体验对齐
- 降低 onboarding 门槛

---

## ADR-012: 配置查找与生效规则

**上下文**: 用户可能在子目录运行 `lorum sync`，也可能显式指定配置文件。

**决策**: 配置查找遵循以下优先级：

1. 若指定 `--config <path>`：仅使用该文件（完全替代语义）
2. 否则：读取全局配置 `~/.config/lorum/config.yaml`
3. 从当前目录向上查找 `.lorum/config.yaml`（类似 git 的目录查找）
4. 合并全局 + 项目级（项目级优先）

**配套命令**:
- `lorum config` — 查看当前目录下生效的完整配置（合并后）
- `lorum config --resolve-env` — 显示生效配置并解析环境变量
- `lorum config --local` — 仅显示项目级配置
- `lorum config --global` — 仅显示全局配置
