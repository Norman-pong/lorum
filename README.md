# Lorum

[![crates.io](https://img.shields.io/crates/v/lorum)](https://crates.io/crates/lorum)
[![docs.rs](https://docs.rs/lorum/badge.svg)](https://docs.rs/lorum)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

> 跨 AI 编码工具的统一 MCP 配置管理器

## 功能特性

- 一处配置，同步到所有支持的 AI 工具
- 支持 MCP Servers、Rules、Hooks、Skills 多维度管理
- 8+ AI 工具适配（Claude Code、Cursor、Windsurf、Codex、kimi、trae、OpenCode、Proma）
- 交互式初始化、配置导入、备份与恢复
- 配置健康检查与诊断

## 安装

```bash
cargo install lorum
```

## 快速开始

```bash
lorum init       # 交互式初始化，检测已安装的 AI 工具
lorum sync       # 同步配置到所有工具
lorum status     # 查看各工具配置状态
```

## 支持的工具

| 工具 | MCP | Rules | Hooks | Skills |
|------|-----|-------|-------|--------|
| Claude Code | ✅ | - | ✅ | ✅ |
| Cursor | ✅ | ✅ | - | - |
| Windsurf | ✅ | ✅ | - | - |
| Codex | ✅ | ✅ | - | - |
| kimi | ✅ | - | ✅ | - |
| trae | ✅ | - | - | - |
| OpenCode | ✅ | - | - | - |
| Proma | ✅ | - | - | ✅ |

## 命令速查

| 命令 | 说明 |
|------|------|
| `init` | 交互式初始化，自动检测已安装的 AI 工具 |
| `sync` | 同步配置到所有已适配工具 |
| `status` | 查看各工具配置状态 |
| `check` | 配置健康检查与诊断 |
| `import` | 从现有工具导入配置 |
| `backup` | 备份当前配置 |
| `config` | 管理全局配置项 |
| `skill` | 管理 Skills 技能集 |

## License

MIT © 2026 Norman-pong

---

## 发音指南

| 体系 | 近似标注 | 中文谐音 |
|------|---------|---------|
| 拉丁语原音 | LO-rum（长音）| 洛-伦 |
| 英语化读音 | LORE-um | 洛尔-姆 |
| 简化读法 | LO-rum | 罗-伦 |
