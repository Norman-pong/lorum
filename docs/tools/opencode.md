# opencode 配置

## 状态

- 开源 AI CLI 工具
- 官方文档: https://opencode.ai

## MCP 配置

- **位置**: `opencode.json`（项目级）或 `~/.config/opencode/opencode.json`（全局）
- **Schema**:
  ```json
  {
    "$schema": "https://opencode.ai/config.json",
    "mcp": {
      "my-local-mcp-server": {
        "type": "local",
        "command": ["npx", "-y", "my-mcp-command"],
        "enabled": true,
        "environment": { "MY_ENV_VAR": "value" },
        "timeout": 5000
      },
      "remote-server": {
        "type": "remote",
        "url": "https://my-mcp-server.example.com"
      }
    }
  }
  ```
- **CLI 命令**:
  - `opencode mcp add` — 交互式添加 MCP 服务器
  - `opencode mcp list` — 列出已配置服务器
  - `opencode mcp auth <name>` — OAuth 认证
  - `opencode mcp debug <name>` — 调试连通性

## 其他配置

- 支持 GitHub Agent (`opencode github install/run`)
- 支持 skills/agents 管理
