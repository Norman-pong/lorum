# Claude Code 配置

## MCP 配置

- **位置**: `~/.claude/settings.json`（全局）或 `.claude/settings.json`（项目级）
- **Schema**:
  ```json
  {
    "mcpServers": {
      "server-name": {
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-fetch"],
        "env": { "KEY": "value" }
      }
    }
  }
  ```
- **特性**: 支持 stdio 传输；可通过 `claude mcp add` 命令管理

## Hooks

- **位置**: `~/.claude/settings.json`（或项目级）中的 `hooks` 字段
- **Schema**（对象结构，以事件名为 key）：
  ```json
  {
    "hooks": {
      "PreToolUse": [
        {
          "match": "Bash",
          "command": "scripts/security_scanner.py '{{command}}'",
          "timeout": 60
        }
      ],
      "PostToolUse": [
        {
          "match": "Write|Edit",
          "command": "npx prettier --write {{file}}"
        }
      ]
    }
  }
  ```
- **支持的事件**（18+）:
  - `SessionStart`, `SessionEnd`
  - `Stop`, `SubagentStop`
  - `PreToolUse`, `PostToolUse`, `PostToolUseFailure`
  - `PermissionRequest`
  - `WorktreeCreate`, `WorktreeRemove`
  - `TeammateIdle`, `TaskCompleted`, `ConfigChange`
- **Handler 类型**: `command`（默认 600s 超时）、`http`、`prompt`（30s 超时）、`agent`（60s 超时）
- **关键特性**: `PreToolUse` 可通过 exit code 2 阻止工具执行；`match` 字段支持正则匹配工具名

## Skills

- **位置**: `~/.claude/skills/<skill-name>/SKILL.md`（全局）或 `.claude/skills/<skill-name>/SKILL.md`（项目级）
- **结构**: 每个 skill 是一个目录，内含 SKILL.md，格式为 Markdown，含 Instructions 和 Examples 章节
- **调用**: 通过 `/skill-name` 命令触发
