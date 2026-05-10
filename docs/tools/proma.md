# Proma 配置

## MCP 配置

- **位置**: `~/.proma/mcp.json`（工作区级）
- **Schema**:
  ```json
  {
    "servers": {
      "fetch": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-fetch"] }
    }
  }
  ```

## Skills

- **位置**: `~/.proma/agent-workspaces/<workspace>/skills/<skill-name>/SKILL.md`
- **结构**: 与 Claude Code 类似，目录 + SKILL.md

## Config

- **位置**: `~/.proma/agent-workspaces/<workspace>/config.json`
- **内容**: attachedDirectories、技能列表等
