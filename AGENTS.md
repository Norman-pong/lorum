# SETUP

Always reply to users in Simplified Chinese

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

**Project docs:** See [`docs/`](docs/) for architecture decisions, roadmap, and tool configuration references.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

## 5. Rust Code Standard

This project is written in Rust. Follow these conventions, derived from the Rust API Guidelines and Clippy lints.

### 5.1 Quality Gates (Pre-Commit)

Run these two commands before every commit. Either failure blocks the commit until fixed.

```bash
# 1. Format check
cargo fmt --check
# Fix with: cargo fmt

# 2. Static analysis
cargo clippy --workspace --all-targets --all-features --tests -- -D warnings
```

### 5.2 Naming (RFC 430)

| Item | Convention | Example |
|---|---|---|
| Crates | `snake_case` (prefer single word) | `lorum`, `config_sync` |
| Types / Enums / Traits | `UpperCamelCase` | `ConfigParser`, `SyncError` |
| Functions / Methods / Modules | `snake_case` | `parse_config()`, `sync_engine` |
| Constants / Statics | `SCREAMING_SNAKE_CASE` | `DEFAULT_TIMEOUT` |
| Acronyms | Treated as one word | `Uuid`, `is_xid_start` |

- **Getters** — Never use `get_` prefix. Use `first()` not `get_first()`. Mutable reference: `first_mut()`.
- **Conversion methods** — Follow the C-CONV convention:
  - `as_` — zero-cost, borrowed → borrowed
  - `to_` — expensive, borrowed → owned (or owned → owned for Copy types)
  - `into_` — consuming, owned → owned (non-Copy)

### 5.3 Error Handling (C-GOOD-ERR)

- Every fallible public API returns `Result<T, E>` where `E` implements `std::error::Error + Send + Sync`.
- **Never use `()` as an error type.**
- `Display` output for errors is lowercase with no trailing punctuation.
- In library code, prefer `?` over `unwrap()`. `unwrap()` is acceptable only in tests or for truly impossible states.

### 5.4 Common Traits (C-COMMON-TRAITS)

New types should implement these traits unless there is a reason not to:

`Copy`, `Clone`, `Debug`, `Display`, `Default`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`

### 5.5 Documentation (C-EXAMPLE / C-FAILURE)

- Every public item has a `///` rustdoc block with a runnable example.
- Examples use `?` instead of `unwrap()`.
- Public functions include `# Errors`, `# Panics`, and `# Safety` sections when applicable.
- Run `cargo doc` to verify documentation builds without warnings.

### 5.6 Dependencies

- Prefer the standard library. New dependencies require justification in the PR description.
- Avoid dependencies that pull in heavy transitive trees for minor utility.

### 5.7 Safety

- Unsafe code is **prohibited** unless explicitly approved in a design doc.
- If unsafe is unavoidable, wrap it in a safe abstraction and document the invariants with `// SAFETY:` comments.

### 5.8 Common Clippy Fixes

| Clippy Warning | Typical Fix | Guideline |
|---|---|---|
| `unnecessary_wraps` | Remove `Result` if the function never fails | C-GOOD-ERR |
| `too_many_arguments` | Introduce a config struct or Builder pattern | C-GENERIC |
| `cast_possible_truncation` | Use `try_into()` or `saturating_*` | C-CONV-TRAITS |
| `missing_errors_doc` | Add `# Errors` section | C-FAILURE |
| `missing_panics_doc` | Add `# Panics` section | C-FAILURE |
| `unwrap_used` | Replace with `?` or explicit error handling in library code | C-GOOD-ERR |

## 6. Testing Standard

Tests are first-class code. They must be as maintainable and readable as production code.

### 6.1 Test Organization

| Test Type | Location | Scope | API Access |
|---|---|---|---|
| **Unit tests** | `src/` file (inline `#[cfg(test)] mod tests`) when file ≤ 300 lines; separate `src/tests/` file when > 300 lines | Single module / function | Private and public |
| **Integration tests** | `tests/*.rs` | Cross-module workflows | Public API only |
| **Doc tests** | `/// ``` ` blocks in public function docs | API usage examples | Public API only |

### 6.2 Unit Test Rules

- One test per behavior, not per function.
- Test names describe the behavior: `rejects_invalid_config_path` not `test_parse_config`.
- Use `assert_eq!`, `assert!`, `assert_matches!` — not manual `if` + `panic!`.
- `unwrap()` is acceptable in test code, but prefer `?` in doc test examples.
- Group related tests in a module with `mod tests`.

### 6.3 Integration Test Rules

- Each `tests/*.rs` file is a separate binary. Keep shared helpers in `tests/common/mod.rs`.
- Test real workflows end-to-end: create temp files, run the sync, assert output.
- Do not mock the filesystem unless the test would be unreasonably slow. Real I/O is preferred.
- Clean up temp files and directories after tests (use `tempfile` crate).

### 6.4 Doc Test Rules

- Every public function must have a runnable `/// ``` ` example.
- Examples use `?` for error handling, not `unwrap()`.
- If a function has multiple common use cases, include multiple examples.

### 6.5 Test Quality Checklist

- [ ] Tests fail before the fix and pass after the fix.
- [ ] Edge cases are covered: empty input, maximum size, malformed data, concurrent access.
- [ ] Tests are deterministic — no reliance on timing, randomness, or external state.
- [ ] Tests run fast — slow tests go in integration tests or behind a feature flag.
- [ ] No test-to-test coupling — each test sets up its own state.

---

## 7. Code Review

Every code change must pass review before merging. Reviews use the following severity-rated checklist.

### 6.1 Severity Levels

| Level | Meaning | Action |
|---|---|---|
| **CRITICAL** | Security vulnerability or data-loss bug | Must fix before merge |
| **HIGH** | Bug or major code smell | Should fix before merge |
| **MEDIUM** | Minor issue or missed best practice | Fix when possible |
| **LOW** | Style suggestion or nit | Consider fixing |

### 6.2 Review Checklist

#### Security
- [ ] No hardcoded secrets (API keys, passwords, tokens)
- [ ] All user inputs are sanitized or validated
- [ ] No SQL/NoSQL injection vectors
- [ ] No XSS through unescaped outputs
- [ ] Authentication and authorization properly enforced

#### Code Quality
- [ ] Functions are focused and cohesive (guideline: < 50 lines)
- [ ] Cyclomatic complexity is reasonable (guideline: < 10)
- [ ] No deeply nested control flow (> 4 levels is a smell)
- [ ] No duplicated logic (DRY)
- [ ] Naming is clear and descriptive

#### Performance
- [ ] No N+1 query patterns
- [ ] Appropriate caching where applicable
- [ ] Efficient algorithms (avoid O(n²) when O(n) is possible)

#### Best Practices
- [ ] Error handling is present and appropriate
- [ ] Logging at correct levels
- [ ] Public APIs are documented with rustdoc
- [ ] Tests cover critical paths and edge cases
- [ ] No commented-out code or debug prints left behind

#### Maintainability
- [ ] Coupling is minimized
- [ ] Code is testable without heavy mocking
- [ ] Types communicate intent (avoid `String` soup)

### 6.3 Review Output Format

The reviewer produces a report in this structure:

```
Files Reviewed: N
Total Issues: N

CRITICAL (N)
-----------
1. src/file.rs:line
   Issue: description
   Risk: explanation
   Fix: concrete suggestion

HIGH (N)
--------
...

MEDIUM (N)
----------
...

LOW (N)
-------
...

RECOMMENDATION: APPROVE | REQUEST CHANGES | COMMENT
```

### 6.4 Approval Criteria

- **APPROVE** — No CRITICAL or HIGH issues; only minor improvements.
- **REQUEST CHANGES** — One or more CRITICAL or HIGH issues present.
- **COMMENT** — Only LOW/MEDIUM issues; no blocking concerns.

## 8. Commit Protocol (Lore)

Commit messages capture *why* the change was made and what trade-offs shaped it — not *what* changed. The diff already shows the what. This is critical for future readers who have no context about the decision.

### 7.1 Format

```
<intent line: why the change was made, not what changed>

<body: narrative context — constraints, approach rationale>

Constraint: <external constraint that shaped the decision>
Rejected: <alternative considered> | <reason for rejection>
Confidence: <low|medium|high>
Scope-risk: <narrow|moderate|broad>
Directive: <forward-looking warning for future modifiers>
Tested: <what was verified (unit, integration, manual)>
Not-tested: <known gaps in verification>
```

### 7.2 Rules

1. **First line describes intent, not content.**
   - Good: `Prevent silent session drops during long-running operations`
   - Bad: `Add error handling to auth.rs`

2. **Body explains context and trade-offs.**
   - What constraints forced the decision.
   - Why this approach was chosen over alternatives.
   - Any non-obvious side effects or assumptions.

3. **Trailers capture structured metadata.**
   - Only include trailers that add value for this specific commit.
   - Never add boilerplate trailers.
   - Use Chinese or English for trailer values — match the project's existing convention.

### 7.3 Trailer Reference

| Trailer | When to Use |
|---|---|
| `Constraint:` | External force shaped the decision (API limit, deadline, policy, dependency version). |
| `Rejected:` | Alternative considered and discarded. Format: `Rejected: approach | reason` |
| `Confidence:` | `high` = well-tested, familiar pattern. `medium` = some uncertainty. `low` = experimental, needs scrutiny. |
| `Scope-risk:` | `narrow` = localized. `moderate` = multiple modules. `broad` = core infrastructure. |
| `Directive:` | Warning for future modifiers (e.g., "don't narrow error handling without verifying upstream"). |
| `Tested:` | Specific verification performed. |
| `Not-tested:` | Honest gaps in verification. |
| `Reversibility:` | `clean` / `messy` / `irreversible`. |
| `Related:` | Links to issues, tickets, or related commits. |

### 7.4 Example

```
Prevent silent session drops during long-running operations

The auth service returns inconsistent status codes on token
expiry, so the interceptor catches all 4xx responses and
triggers an inline refresh.

Constraint: Auth service does not support token introspection（认证服务不支持令牌内省）
Constraint: Must not add latency to non-expired-token paths（不得为未过期令牌引入额外延迟）
Rejected: Extend token TTL to 24h | security policy violation（违反安全策略）
Rejected: Background refresh on timer | race condition with concurrent requests（并发竞态条件）
Confidence: high
Scope-risk: narrow
Directive: Error handling is intentionally broad (all 4xx) — do not narrow without verifying upstream behavior（错误处理有意覆盖所有4xx响应，在验证上游行为前不要缩小范围）
Tested: Single expired token refresh (unit)
Not-tested: Auth service cold-start > 500ms behavior（认证服务冷启动大于500ms的行为）
```

### 7.5 Atomic Commits

One logical change per commit. Squash fixups before merging. If a PR contains multiple independent changes, split them into separate commits so each can be reverted independently.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.