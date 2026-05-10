# Aider 配置

## 状态

- 开源终端 AI 代码助手
- **仓库**: `Aider-AI/aider`

## MCP 配置

- **支持状态**: 截至 2026-05，MCP 原生支持情况**未确认**（可能为实验性或通过外部集成）

## Config

- **位置**: `.aider.conf.yml`（项目级）或 `~/.aider.conf.yml`（全局）
- **Schema**:
  ```yaml
  # 模型设置
  model: sonnet
  editor-model: sonnet
  weak-model: haiku

  # API keys
  anthropic-api-key: sk-ant-...
  openai-api-key: sk-...

  # Git 行为
  auto-commits: true
  dirty-commits: false

  # Lint/Test
  lint-cmd: "eslint"
  test-cmd: "pytest"
  ```
