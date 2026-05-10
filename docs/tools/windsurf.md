# Windsurf 配置

## 状态

- AI 原生 IDE，由 Codeium（现 Cognition）开发
- Cascade 为内置 AI agent
- 2025-07 被 Cognition（Devin 开发商）收购

## MCP 配置

- **位置**: `~/.codeium/windsurf/mcp_config.json`（全局）
- **Schema**:
  ```json
  {
    "mcpServers": {
      "github": {
        "command": "docker",
        "args": ["run", "-i", "--rm", "-e", "GITHUB_PERSONAL_ACCESS_TOKEN", "ghcr.io/github/github-mcp-server"],
        "env": { "GITHUB_PERSONAL_ACCESS_TOKEN": "your_token" }
      },
      "zapier": {
        "serverUrl": "https://actions.zapier.com/mcp/YOUR_MCP_KEY/sse"
      }
    }
  }
  ```
- **传输方式**: stdio、SSE（`serverUrl`）、Streamable HTTP
- **特性**:
  - 内置 MCP Marketplace 一键安装
  - 企业版支持白名单/黑名单（regex 模式匹配）
  - 可配置自定义 Enterprise Registry

## Rules

- **位置**: `.windsurfrules`（项目根目录）
- **特性**: 类似 `.cursorrules`，定义 AI 行为、编码标准、项目上下文
