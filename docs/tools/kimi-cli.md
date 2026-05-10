# kimi CLI 配置

## 状态

- **已正式发布**，不是 experimental
- **仓库**: `MoonshotAI/kimi-cli`
- **最新版本**: 1.40.0 (2026-04)
- **官方文档**: https://moonshotai.github.io/kimi-cli/

## 配置文件

- **格式**: TOML
- **位置**: `~/.config/kimi/config.toml`（推测）

## 配置示例

```toml
default_model = "kimi-for-coding"
default_thinking = false
default_yolo = false

[providers.kimi-for-coding]
type = "kimi"
base_url = "https://api.kimi.com/coding/v1"
api_key = "sk-xxx"

[models.kimi-for-coding]
provider = "kimi-for-coding"
model = "kimi-for-coding"
max_context_size = 262144

[mcp.client]
tool_call_timeout_ms = 60000

# Lifecycle hooks (Beta)
[[hooks]]
event = "PreToolUse"
matcher = "Shell"
command = ".kimi/hooks/safety-check.sh"
timeout = 10
```

## MCP 配置

- **方式**: `kimi mcp add <tool>`（命令行）或配置文件中的 `[mcp.client]` 节
- **项目级**: `kimi --mcp-config-file ./project-mcp.json`

## Hooks

- **方式**: TOML 中的 `[[hooks]]` 数组
- **支持事件**: `PreToolUse` 等（Beta 阶段）
- **字段**: `event`, `matcher`, `command`, `timeout`
