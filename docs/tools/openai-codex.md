# OpenAI Codex 配置

> 注意：以下信息基于社区资料推断，OpenAI Codex CLI 仍在快速发展中。

## MCP 配置

- **位置**: `~/.codex/config.json`（全局）或 `.codex/config.json`（项目级）
- **Schema**: 类似 Claude Code 的 `mcpServers` 结构
- **特性**: 支持 stdio 和 http 传输

## Rules / Instructions

- **用户级**: `~/.codex/instructions.md`
- **项目级**: `.codex/rules.md` 或 `.codex/instructions.md`
- **特性**: 纯文本 instructions，无结构化 hooks

## Config

- **位置**: `~/.codex/config.yaml`（或 .json）
- **内容**: API key、默认模型、自动提交设置等
