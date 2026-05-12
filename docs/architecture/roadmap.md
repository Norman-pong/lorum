# 路线图

> 分阶段实现跨 AI CLI 统一配置管理

---

## Phase 1: MCP 统一管理（优先级最高）

**目标**: 实现 5 个工具的 MCP 服务器配置统一管理。

**任务**:
1. 定义 lorum 统一 MCP 配置 schema（`mcp.servers`）
2. 实现各平台的配置读写适配器
3. 实现字段名映射（`mcpServers` / `mcp_servers` / `servers` / `[mcp.client]`）
4. 实现 MCP 服务器的增删改查 CLI 命令
5. 实现配置同步（单向导出到各工具）

**覆盖工具**: Claude Code、OpenAI Codex、Proma、kimi、trae

**排除**: Continue.dev（独立适配）、Aider（暂不支持）

---

## Phase 2: Rules 通用语法 + 各平台翻译器

**目标**: 统一项目级 AI 规则管理。

**任务**:
1. 定义 lorum 统一 Rules 格式（Markdown）
2. 实现输出翻译器：
   - → `.cursorrules`
   - → `.windsurfrules`
   - → `.codex/rules.md`
3. 实现 Rules 的增删改查 CLI 命令

**覆盖工具**: Cursor、Windsurf、OpenAI Codex

---

## Phase 3: Hooks 通用语法 + 各平台翻译器

**目标**: 统一生命周期钩子管理。

**任务**:
1. 定义 lorum 统一 Hooks 语法（YAML，kebab-case 事件名 + `matcher` 字段，以 Claude Code 语义为基准）
2. 实现输出翻译器：
   - → Claude Code `settings.json` hooks（PascalCase 事件名，支持 5 种 handler 类型）
   - → kimi `config.toml` `[[hooks]]`（数组结构，降级子集）
3. 实现 Hooks 的增删改查 CLI 命令

**覆盖工具**: Claude Code、kimi

---

## Phase 4: Skills 管理

**目标**: 统一 Skill 管理（仅部分工具受益）。

**任务**:
1. 定义 lorum 统一 Skill 格式（兼容 Claude Code / Proma 的 SKILL.md 结构）
2. 实现 Skill 的增删改查 CLI 命令
3. 实现 Skill 的跨平台同步

**覆盖工具**: Claude Code、Proma

**注意**: opencode 也有 skill 概念但机制不同，需单独处理。

---

## Phase 5: IDE 类工具深度集成

**目标**: 完善 IDE 类工具的配置管理。

**任务**:
1. 处理 VS Code 设置继承机制（trae）
2. 处理全局 vs 项目级配置优先级（Cursor、Windsurf、trae）
3. 处理 Windsurf 特有的 SSE / Streamable HTTP 传输
4. 处理 Windsurf MCP Marketplace 集成
5. Continue.dev 独立适配器（models/commands/providers 结构）

**覆盖工具**: Cursor、Windsurf、trae、Continue.dev

---

## 风险与依赖

| 风险 | 缓解措施 |
|------|---------|
| 工具配置 schema 变更 | 建立版本化 schema 检测机制 |
| 字段名映射遗漏 | 建立映射表自动化测试 |
| IDE 配置路径差异 | 为每个 IDE 维护独立的适配器 |
| 配置冲突（全局 vs 项目级） | 实现优先级合并策略 |
