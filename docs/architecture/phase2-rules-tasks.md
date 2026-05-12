# Phase 2 Tasks: Rules 通用语法 + 各平台翻译器

> 创建日期: 2026-05-12
> 状态: 待实施
> 前置: Phase 1 (MCP 统一管理) 已完成

---

## 设计决策

| 决策项 | 方案 | 理由 |
|--------|------|------|
| **源文件格式** | `.lorum/RULES.md`（Markdown，以 `##` 标题为 section 分隔） | Markdown 天然适合规则文本，与各目标工具格式一致 |
| **CRUD 粒度** | 以 `##` 二级标题为分隔符，每个 section 是一个可增删改的 rule | 结构化管理优于纯文本编辑，支持 `add/remove/edit/list` 命令 |
| **覆盖范围** | Cursor、Windsurf、Codex（仅项目级） | 严格遵循路线图；Claude Code CLAUDE.md 留待后续迭代 |
| **同步策略** | 完全覆盖（与 MCP 一致） | lorum 作为唯一来源，目标文件完整替换 |
| **适配器模式** | 新增 `RulesAdapter` trait，独立于 `ToolAdapter` | Rules 的读写语义与 MCP 不同（纯文本 vs 结构化数据），职责分离 |
| **Rules 文件层级** | 仅项目级（`.lorum/RULES.md`） | Rules 本质上是项目编码规范，全局级意义不大 |

### Rules 文件格式

```markdown
# Project Rules

This file defines AI coding rules for the project.

## Code Style
- Use 4 spaces for indentation
- Prefer immutability

## Testing
- Every public function needs a doc test
- Integration tests for cross-module workflows

## Architecture
- Follow the adapter pattern for new tools
```

- `#` 标题行及以上内容为 **preamble**（前言），不属于任何 section
- 每个 `##` 标题定义一个 **section**，名称为标题文本
- Section 内容为标题下方到下一个 `##` 标题之前的所有文本

### 目标工具映射

| 工具 | 目标文件 | 格式 |
|------|---------|------|
| Cursor | `.cursorrules`（项目根目录） | 纯文本 |
| Windsurf | `.windsurfrules`（项目根目录） | 纯文本 |
| Codex | `.codex/rules.md`（项目目录） | Markdown |

同步时将 RULES.md 的完整内容写入目标文件（Cursor/Windsurf 不要求特定格式，纯 Markdown 可直接使用）。

---

## Milestone 1: Rules 数据模型与解析器

- [ ] **T1.1** 创建 `src/rules.rs`，定义 `RulesSection` 和 `RulesFile` 结构体：
  ```rust
  struct RulesSection { name: String, content: String }
  struct RulesFile { preamble: String, sections: Vec<RulesSection> }
  ```
- [ ] **T1.2** 实现 `parse_rules(content: &str) -> RulesFile` 解析器：按 `## ` 行分割，提取 section 名称和内容
- [ ] **T1.3** 实现 `render_rules(rules: &RulesFile) -> String` 序列化器：将结构体还原为 Markdown 文本
- [ ] **T1.4** 实现文件 I/O：`load_rules(project_root)` / `save_rules(project_root, rules)` 读写 `.lorum/RULES.md`
- [ ] **T1.5** 实现项目根目录查找：复用 `config::find_project_config` 的目录搜索逻辑，返回包含 `.lorum/` 的目录路径
- [ ] **T1.6** 编写解析器单元测试：
  - 空文件 → 空 RulesFile
  - 仅 preamble（无 `##` 标题）→ preamble 有内容，sections 为空
  - 多个 sections → 正确分割
  - 重复 section 名称 → 后者覆盖前者（或报错，待确认）
  - 内容中包含 `###` 三级标题 → 不作为 section 分割符
  - 内容中包含 `##` 但不在行首 → 不作为分割符
- [ ] **T1.7** 验证: `cargo test` 通过

---

## Milestone 2: Rules 适配器

- [ ] **T2.1** 在 `src/adapters/mod.rs` 中定义 `RulesAdapter` trait：
  ```rust
  trait RulesAdapter {
      fn name(&self) -> &str;
      fn rules_path(&self, project_root: &Path) -> PathBuf;
      fn read_rules(&self, project_root: &Path) -> Result<Option<String>, LorumError>;
      fn write_rules(&self, project_root: &Path, content: &str) -> Result<(), LorumError>;
  }
  ```
  新增 `all_rules_adapters()` 和 `find_rules_adapter()` 工厂函数。
- [ ] **T2.2** 创建 `src/adapters/cursor.rs`，实现 Cursor RulesAdapter：
  - `rules_path` → `<project_root>/.cursorrules`
  - `read_rules` → 读取文件，不存在返回 `None`
  - `write_rules` → 写入文件（覆盖）
- [ ] **T2.3** 创建 `src/adapters/windsurf.rs`，实现 Windsurf RulesAdapter：
  - `rules_path` → `<project_root>/.windsurfrules`
  - 其余同 Cursor
- [ ] **T2.4** 在 `src/adapters/codex.rs` 中新增 RulesAdapter 实现：
  - `rules_path` → `<project_root>/.codex/rules.md`
  - 复用现有 codex.rs 的 TOML 工具函数和模式
- [ ] **T2.5** 编写各适配器单元测试（tempdir 模拟项目目录，验证读写正确性、文件不存在时的行为）
- [ ] **T2.6** 验证: `cargo test` 通过

---

## Milestone 3: Rules 同步引擎

- [ ] **T3.1** 在 `src/sync/mod.rs` 中新增 rules 同步函数：
  - `sync_rules(project_root, content, tools)` → 写入指定工具
  - `sync_rules_all(project_root, content)` → 写入所有 rules 适配器
  - 每个 adapter 独立执行，单工具失败不影响其他
- [ ] **T3.2** 实现 rules dry-run：对比当前各工具文件内容与目标内容，输出 diff 摘要
- [ ] **T3.3** 实现 rules 备份：sync 前备份目标文件到 `~/.config/lorum/backups/<tool>-rules-<timestamp>.<ext>`
- [ ] **T3.4** 编写同步引擎单元测试（tempdir 模拟多工具，验证完整同步流程）
- [ ] **T3.5** 验证: `cargo test` 通过

---

## Milestone 4: Rules CLI 命令

- [ ] **T4.1** 在 `src/main.rs` 的 clap 定义中新增 `Rule` 子命令和 `RuleAction` 枚举：
  ```
  lorum rule
  ├── init                              # 创建 .lorum/RULES.md 模板
  ├── add <name> --content <content>    # 新增 section
  ├── remove <name>                     # 删除 section
  ├── edit <name> --content <content>   # 替换 section 内容
  ├── list                              # 列出所有 section 名称
  ├── show [name]                       # 显示内容（全部或指定 section）
  ├── sync [--dry-run] [--tools ...]    # 同步到目标工具
  └── import --from <tool>              # 从目标工具导入到 RULES.md
  ```
- [ ] **T4.2** 创建 `src/commands/rule.rs`，实现各 handler：
  - `run_rule_init` → 创建带 preamble 的模板 RULES.md
  - `run_rule_add` → 解析 → 追加 section → 写回
  - `run_rule_remove` → 解析 → 删除 section → 写回
  - `run_rule_edit` → 解析 → 替换 section 内容 → 写回
  - `run_rule_list` → 解析 → 表格输出 section 名称和内容行数
  - `run_rule_show` → 解析 → 输出指定 section 或全部内容
  - `run_rule_sync` → 读取 RULES.md → 调用同步引擎
  - `run_rule_import` → 读取目标工具 rules 文件 → 写入 RULES.md
- [ ] **T4.3** 更新 `src/commands/mod.rs` 中的 `run_hook` 和 `run_skill` 保持 stub 不变；新增 `run_rule` dispatcher
- [ ] **T4.4** 编写 CLI handler 单元测试（tempdir 模拟项目目录，验证 add/remove/edit/list 的正确行为）
- [ ] **T4.5** 验证: `cargo test` 通过

---

## Milestone 5: 集成测试与验证

- [ ] **T5.1** 编写端到端集成测试（`tests/integration_rules.rs`）：
  rule init → rule add (多个) → rule sync → 验证 `.cursorrules`、`.windsurfrules`、`.codex/rules.md` 内容正确
- [ ] **T5.2** 编写 import 集成测试：创建 `.cursorrules` → `rule import --from cursor` → 验证 RULES.md 内容正确
- [ ] **T5.3** 编写备份恢复测试：rule sync → backup list → backup restore → 验证恢复后文件一致
- [ ] **T5.4** 编写 CRUD 序列测试：add → edit → remove → list 验证每步状态正确
- [ ] **T5.5** 验证: `cargo fmt --check` + `cargo clippy -- -D warnings` + `cargo test` 全部通过

---

## 依赖关系

```
M1 (数据模型) → M2 (适配器) → M3 (同步引擎)
                                   ↓
                              M4 (CLI 命令)
                                   ↓
                              M5 (集成测试)
```

M2 依赖 M1 的 `RulesFile` 结构体。M3 依赖 M1 的 I/O 和 M2 的适配器。M4 依赖 M1（解析器）和 M3（同步）。M5 依赖所有前置。

---

## 项目结构变更

```
src/
├── rules.rs                   # NEW: Rules 数据模型 + 解析器
├── adapters/
│   ├── mod.rs                 # MODIFY: 新增 RulesAdapter trait + 工厂函数
│   ├── cursor.rs              # NEW: Cursor rules 适配器
│   ├── windsurf.rs            # NEW: Windsurf rules 适配器
│   └── codex.rs               # MODIFY: 新增 RulesAdapter impl
├── sync/
│   └── mod.rs                 # MODIFY: 新增 rules 同步函数
├── commands/
│   ├── mod.rs                 # MODIFY: 新增 rule dispatcher
│   ├── rule.rs                # NEW: Rule CLI handlers
│   └── rule_tests.rs          # NEW: Rule CLI tests
└── main.rs                    # MODIFY: 新增 Rule clap 子命令
```

---

## 预估规模

| Milestone | 文件 | 预估行数 |
|-----------|------|---------|
| M1 数据模型 | 1 个 | ~200 行 + ~150 行测试 |
| M2 适配器 | 2 新 + 1 改 | ~250 行 + ~150 行测试 |
| M3 同步引擎 | 1 改 | ~120 行 + ~80 行测试 |
| M4 CLI 命令 | 1 新 + 2 改 | ~250 行 + ~120 行测试 |
| M5 集成测试 | 1 个 | ~200 行 |
| **合计** | ~4 新 + 4 改 | ~1,520 行 |

---

## 与后续 Phase 的衔接

- Phase 3 (Hooks) 可复用 M2 的 RulesAdapter 模式，新增 HooksAdapter trait
- Phase 5 (IDE 深度集成) 将为 Cursor 和 Windsurf 补充 ToolAdapter (MCP) 实现
- Codex 的 `codex.rs` 将同时实现 ToolAdapter + RulesAdapter，为后续扩展提供模板
