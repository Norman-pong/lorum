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
| OpenAI Codex | `mcpServers` |
| Proma | `servers` |
| kimi | `mcp`（JSON）/ `[mcp.client]`（TOML） |
| opencode | `mcp` |
| trae | `mcpServers` |
| Cursor | `mcpServers` |
| Windsurf | `mcpServers` |

---

## ADR-002: Hooks 统一策略

**上下文**: Claude Code 支持 18+ 事件、4 种 handler 类型（对象结构）；kimi 仅支持 Beta 版 `[[hooks]]` 数组（TOML）。

**决策**: 定义 lorum 统一 hooks 语法，以 Claude Code 的丰富语义为基准。输出到 kimi 时降级为支持的子集。

**统一语法草案**:
```yaml
hooks:
  pre-tool-use:
    - match: "Bash"
      command: "scripts/safety-check.sh"
      timeout: 60
  post-tool-use:
    - match: "Write|Edit"
      command: "cargo fmt"
```

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
