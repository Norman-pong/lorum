# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.1] - 2026-05-13

### Added

- Phase 1: MCP 配置管理核心 — 支持 Claude Code、Codex、Proma、kimi、trae 的 MCP 适配器
- Phase 2: Rules 管理 — 支持 Cursor、Windsurf、Codex 的 AI 规则同步
- Phase 3: Hooks 管理 — 支持 Claude Code、kimi 的 hooks 同步（PreToolUse/PostToolUse 等事件）
- Phase 4: Skills 管理 — 支持 Claude Code、Proma 的 skills 目录同步
- Phase 5: 诊断增强 — `status` 四维表格、`check` 六项验证、dry-run 报告
- Phase 6: 配置生命周期 — `init` 交互式引导、`import` 全维度导入、`backup` 备份恢复、`config` 配置查看
- Phase 7: IDE & CLI 补齐 — 新增 Cursor、Windsurf、OpenCode 的 MCP 适配器，覆盖 8 个工具
- 统一的 `config.yaml` 配置格式
- 多格式支持：JSON、TOML、YAML
- 环境变量插值（`${VAR}` 语法）
- 跨平台路径处理
- 全面的单元测试和集成测试（367+ 测试）

### Changed

- 备份系统重构：采用新的时间戳格式和 CLI 交互设计
- `config` 命令增强：支持格式选择、来源标注和 hooks 环境变量插值
- `import` 命令扩展：覆盖 hooks 和 rules 维度，支持 dry-run 预览
- 全局配置路径统一使用 XDG 目录规范
- 同步引擎扩展：支持 Rules 同步与 dry-run 预览

### Fixed

- 修复 Phase 6 代码审查中的 CRITICAL 和 HIGH 级别问题
- 修复 Phase 6 代码审查中的 MEDIUM 和 LOW 级别问题
- 修复 Phase 4 Skills 管理的代码审查问题
- 修复代码审查发现的 clippy 违规和重复代码（DRY）问题
- 修复全局配置路径在所有平台上使用 XDG 约定
- 清理过时的源文件并同步架构文档
