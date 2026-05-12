# Phase 1 Tasks: MCP 统一管理

> 生成日期: 2026-05-11
> 完成日期: 2026-05-12
> 状态: 已完成

---

## Milestone 1: 项目骨架

- [x] **T1.1** 初始化 Rust 项目：`cargo init`，配置 `Cargo.toml` 依赖（clap、serde、serde_yaml、toml、dirs、thiserror、tempfile）
- [x] **T1.2** 定义统一错误类型 `src/error.rs`（LorumError enum，实现 `std::error::Error + Send + Sync`）
- [x] **T1.3** 搭建 CLI 入口 `src/main.rs` + `src/commands/mod.rs`，用 clap 定义顶层命令结构（init / import / sync / check / status / config / backup / mcp / hook / skill），所有子命令先占位 stub
- [x] **T1.4** 验证: `cargo fmt` + `cargo clippy` + `cargo test` 通过

---

## Milestone 2: 配置 Schema 与解析

- [x] **T2.1** 定义统一配置 Schema structs（`src/config.rs`）：`LorumConfig`、`McpConfig`、`McpServer`（command / args / env），支持 serde YAML 序列化/反序列化
- [x] **T2.2** 实现全局配置读取：解析 `~/.config/lorum/config.yaml`（使用 dirs crate 定位 XDG 路径）
- [x] **T2.3** 实现项目级配置查找：从 cwd 向上递归查找 `.lorum/config.yaml`（类似 `.git` 查找逻辑）
- [x] **T2.4** 实现配置合并：全局 + 项目级深度合并，项目级同名服务器覆盖全局，处理 `exclude` 列表
- [x] **T2.5** 实现 `--config <path>` 完全替代语义
- [x] **T2.6** 编写配置解析与合并的单元测试（全局 only、项目级 only、合并、exclude、--config 覆盖）
- [x] **T2.7** 验证: `cargo test` 通过

---

## Milestone 3: 适配器框架

- [x] **T3.1** 定义 adapter trait 层次（`src/adapters/mod.rs`）：`ToolAdapter`（声明支持哪些配置类型 + 配置路径）+ `McpAdapter`（read_mcp / write_mcp），设计为 per-tool 实现
- [x] **T3.2** 实现 Claude Code 适配器（`src/adapters/claude.rs`）：读写 `~/.claude/settings.json`，映射 `mcpServers` 字段，处理全局/项目级/本地三个路径，保留非 MCP 字段
- [x] **T3.3** 实现 Codex 适配器（`src/adapters/codex.rs`）：读写 `~/.codex/config.toml`，映射 `mcp_servers` 字段（snake_case），TOML 格式，处理全局 + 项目级路径
- [x] **T3.4** 实现 Proma 适配器（`src/adapters/proma.rs`）：读写 `~/.proma/mcp.json`，映射 `servers` 字段，JSON 独立文件
- [x] **T3.5** 实现 kimi 适配器（`src/adapters/kimi.rs`）：读写 `~/.kimi/config.toml`，映射 `[mcp.client]` 段，TOML 格式，仅全局路径（项目级通过 CLI flag 不支持文件发现）
- [x] **T3.6** 实现 trae 适配器（`src/adapters/trae.rs`）：读写 `.trae/mcp.json`（项目级），映射 `mcpServers` 字段，JSON 格式
- [x] **T3.7** 编写各适配器的单元测试（mock 文件系统，验证字段映射、保留非 MCP 字段、TOML/JSON 格式正确性）
- [x] **T3.8** 验证: `cargo test` 通过

---

## Milestone 4: 同步引擎

- [x] **T4.1** 实现同步引擎（`src/sync.rs`）：遍历所有已注册适配器，对每个工具执行 read → replace MCP fields → write，单工具失败不影响其他工具
- [x] **T4.2** 实现备份管理（`src/backup.rs`）：sync 前自动备份目标文件到 `~/.config/lorum/backups/<tool>-<timestamp>.<ext>`，保留最近 10 个，超出自动清理
- [x] **T4.3** 实现环境变量插值（`src/env_interpolate.rs`）：扫描 `${VAR}` 模式，默认保留原样；`--expand-env` 模式从 env 读取替换，变量不存在时保留原字符串
- [x] **T4.4** 实现 dry-run 模式：预览变更但不写入，输出 diff 或变更摘要
- [x] **T4.5** 实现 `--tools` 过滤：仅同步到指定工具子集
- [x] **T4.6** 编写同步引擎集成测试（tempdir 模拟多工具配置，验证完整同步流程、备份创建、dry-run 不写入）
- [x] **T4.7** 验证: `cargo test` 通过

---

## Milestone 5: MCP CRUD 命令

- [x] **T5.1** 实现 `lorum mcp add <name> --command <cmd> --args <args...> [--env KEY=VALUE]`：读取配置 → 插入/更新服务器 → 写回
- [x] **T5.2** 实现 `lorum mcp remove <name>`：读取配置 → 删除服务器 → 写回
- [x] **T5.3** 实现 `lorum mcp list`：读取生效配置 → 表格输出所有 MCP 服务器（name / command / args）
- [x] **T5.4** 实现 `lorum mcp edit <name>`：交互式编辑（或 `--set` 参数批量修改）
- [x] **T5.5** 编写 MCP CRUD 单元测试（add/remove/list/edit 在全局/项目级配置上的正确行为）
- [x] **T5.6** 验证: `cargo test` 通过

---

## Milestone 6: 其他 CLI 命令

- [x] **T6.1** 实现 `lorum init`：检测已安装工具 → 交互询问是否导入 → 创建全局 `~/.config/lorum/config.yaml`
- [x] **T6.2** 实现 `lorum init --local`：在当前目录创建 `.lorum/config.yaml`
- [x] **T6.3** 实现 `lorum import --from <tool|all>`：从指定工具的现有配置读取 MCP 服务器 → 写入 lorum 配置
- [x] **T6.4** 实现 `lorum sync` 命令入口：调用同步引擎 + 表格输出结果
- [x] **T6.5** 实现 `lorum check`：校验配置有效性（schema 合法、必填字段、引用的工具是否已安装）
- [x] **T6.6** 实现 `lorum status`：检测各工具安装状态 + 配置文件存在性 + MCP 字段内容，表格输出
- [x] **T6.7** 实现 `lorum config`：输出合并后的生效配置；支持 `--resolve-env` / `--local` / `--global` 子选项
- [x] **T6.8** 实现 `lorum backup list / restore <tool>`：备份管理与恢复
- [x] **T6.9** 实现表格输出工具（`src/output.rs`）：统一的列对齐格式化
- [x] **T6.10** 验证: `cargo test` 通过

---

## Milestone 7: 集成测试与文档

- [x] **T7.1** 编写端到端集成测试（`tests/integration_sync.rs`）：init → mcp add → sync → 验证各工具配置文件内容正确
- [x] **T7.2** 编写 import 集成测试：从真实 Claude Code / Codex 配置导入 → 验证 lorum 配置正确
- [x] **T7.3** 编写备份恢复集成测试：sync → backup list → backup restore → 验证恢复后配置一致
- [x] **T7.4** 编写配置合并集成测试：全局 + 项目级 + exclude → 验证生效配置正确
- [x] **T7.5** 创建 `examples/config.yaml` 示例配置文件
- [x] **T7.6** 验证: `cargo fmt --check` + `cargo clippy -- -D warnings` + `cargo test` 全部通过
- [x] **T7.7** 验证: `cargo doc` 无 warnings

---

## 依赖关系

```
M1 (骨架) → M2 (Schema) → M3 (适配器) → M4 (同步引擎)
                                            ↓
                                      M5 (MCP CRUD) + M6 (其他 CLI)
                                            ↓
                                      M7 (集成测试)
```

M2 和 M3 可部分并行（M3.1 trait 定义依赖 M2.1 schema structs，但各适配器实现可与 M2.2~2.5 并行）。
M5 和 M6 可完全并行（互不依赖）。

---

## 预估规模

| Milestone | 文件 | 预估行数 |
|-----------|------|---------|
| M1 骨架 | 3-4 个 | ~150 行 |
| M2 Schema | 1-2 个 | ~300 行 + ~200 行测试 |
| M3 适配器 | 6-7 个 | ~600 行 + ~400 行测试 |
| M4 同步引擎 | 3 个 | ~250 行 + ~150 行测试 |
| M5 MCP CRUD | 1 个 | ~200 行 + ~100 行测试 |
| M6 其他 CLI | 7-8 个 | ~400 行 + ~200 行测试 |
| M7 集成测试 | 2 个 | ~300 行 |
| **合计** | ~30 个 | ~3,250 行 |
