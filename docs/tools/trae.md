# trae 配置

## 状态

- 基于 VS Code 的 AI IDE（字节跳动）
- 支持 VS Code 插件生态和设置迁移

## MCP 配置

- **UI 方式**: Settings → MCP → Add Manually
- **项目级文件**: `.trae/mcp.json`（需开启 "Enable Project MCP"）
- **Schema**:
  ```json
  {
    "mcpServers": {
      "your-mcp-name": {
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-github"],
        "env": { "GITHUB_PERSONAL_ACCESS_TOKEN": "xxx" }
      },
      "remote-service": {
        "url": "https://example.com/mcp",
        "headers": { "Authorization": "Bearer xxxx" }
      }
    }
  }
  ```
- **特性**: 支持 stdio（local）和 http（remote）两种传输；内置 MCP Marketplace 一键安装

## Settings

- **位置**: `~/Library/Application Support/Trae/settings.json` (macOS)
- **内容**: 标准 VS Code settings + Trae 专属设置
