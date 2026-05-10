# Continue.dev 配置

## 状态

- 开源 AI 代码助手
- 支持 VS Code 和 JetBrains

## MCP 配置

- **支持状态**: 已实现 MCP 支持（具体配置语法未完全公开）

## Config

- **位置**: `~/.continue/config.json`（全局）或 `.continue/config.json`（项目级）
- **Schema**:
  ```json
  {
    "models": [
      {
        "title": "My Model",
        "provider": "openai",
        "model": "gpt-4",
        "apiKey": "..."
      }
    ],
    "tabAutocompleteModel": {
      "provider": "ollama",
      "model": "starcoder2:3b"
    },
    "customCommands": [
      { "name": "test", "prompt": "Write a test for the selected code" }
    ],
    "contextProviders": [
      { "name": "code", "params": {} },
      { "name": "docs", "params": {} }
    ],
    "slashCommands": [
      { "name": "edit", "description": "Edit selected code" }
    ]
  }
  ```
- **注意**: Continue.dev 以 models、customCommands、contextProviders、slashCommands 为核心，与其他工具的 MCP-centric 设计不同
