# lorum 文档

> 跨 AI CLI 统一 harness 项目文档
> 最后更新: 2026-05-10

---

## 目录

- [工具配置详情](tools/) — 各 AI CLI/IDE 的配置方式详解
- [架构决策](architecture/decisions.md) — 调研关键发现
- [路线图](architecture/roadmap.md) — 分阶段实现建议

---

## 项目概览

lorum 是一个跨 AI CLI 的统一配置管理工具（harness），目标是将多个 AI 编程助手（Claude Code、OpenAI Codex、kimi CLI、Cursor、Windsurf 等）的配置统一管理，实现 MCP 服务器、Hooks、Skills、Rules 的跨平台同步。

### 支持的工具（10 个）

| CLI/IDE | 类型 | MCP | Hooks | Skills | Rules |
|---------|------|-----|-------|--------|-------|
| Claude Code | CLI | ✅ | ✅（18+ 事件） | ✅ | ❌ |
| OpenAI Codex | CLI | ✅ | ❌ | ❌ | ✅ |
| Proma | CLI | ✅ | ❌ | ✅ | ❌ |
| kimi CLI | CLI | ✅ | ✅（Beta） | ❌ | ❌ |
| opencode | CLI | ✅ | ❌ | ✅ | ❌ |
| trae | IDE | ✅ | ❌ | ❌ | ❌ |
| Cursor | IDE | ✅ | ❌ | ❌ | ✅ |
| Windsurf | IDE | ✅ | ❌ | ❌ | ✅ |
| Continue.dev | IDE 插件 | ⚠️ 未确认 | ❌ | ❌ | ❌ |
| Aider | CLI | ❓ 存疑 | ❌ | ❌ | ❌ |

### 配置映射速查表

| CLI | MCP 位置 | MCP Schema | Hooks | Skills | Rules | 配置文件 |
|-----|---------|-----------|-------|--------|-------|---------|
| Claude Code | `~/.claude/settings.json` | `mcpServers` | `settings.json` hooks（对象，18+ 事件） | `~/.claude/skills/` | 无 | `settings.json` |
| Codex | `~/.codex/config.json` | `mcpServers` | 无 | 无 | `.codex/rules.md` | `config.yaml` |
| Proma | `~/.proma/mcp.json` | `servers` | 无 | `skills/` | 无 | `config.json` |
| kimi | `~/.config/kimi/config.toml` | `mcp.client` + `kimi mcp add` | TOML `[[hooks]]`（Beta） | 无 | 无 | `config.toml` |
| opencode | `~/.config/opencode/opencode.json` | `mcp`（local/remote） | 无 | 有 | 无 | `opencode.json` |
| trae | `.trae/mcp.json` + UI | `mcpServers` | 无 | 无 | 无 | `settings.json` |
| Cursor | `.cursor/mcp.json` | `mcpServers` | 无 | 无 | `.cursorrules` | — |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` | `mcpServers` + SSE | 无 | 无 | `.windsurfrules` | `mcp_config.json` |
| Continue.dev | 未确认 | 未确认 | 无 | 无 | 无 | `config.json` |
| Aider | 存疑 | 存疑 | 无 | 无 | 无 | `.aider.conf.yml` |

---

## 审核说明

本文档原始内容经过 WebSearch 交叉验证，修正了以下问题：

- Claude Code Hooks 的 schema 结构（对象而非数组，支持 18+ 事件和多种 handler 类型）
- kimi CLI 状态从 experimental 改为正式发布，配置格式为 TOML
- opencode 配置文件名和位置修正（`~/.config/opencode/opencode.json`）
- trae MCP 配置位置修正为 `.trae/mcp.json`
- deepwiki 上 kimi 信息严重过时（YAML 格式 vs 实际 TOML，旧模型名 vs 实际 kimi-for-coding）

## 数据来源

- 官方文档: Claude Code Docs、OpenAI Codex GitHub、kimi-cli 官方文档 (moonshotai.github.io)、opencode.ai、trae docs、Windsurf docs
- 社区资源: GitHub 仓库、技术博客、Lobehub Skills Marketplace
- 交叉验证: WebSearch 多源对比
