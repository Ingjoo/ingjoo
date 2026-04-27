---
session: ses_integration_boost
date: 2026-04-26
---

# 会话总结 — 集成测试扩充 + Flaky 修复 + 文档同步

## 目标
继续推进项目：修复 flaky test、扩充集成测试覆盖、确保所有文档和进度记录同步更新。

## 限制与偏好
- 测试必须同时通过 `cargo test --workspace` 和 `cargo test --workspace --features multi-db`
- 集成测试遵循 `integration_test.rs` 中已有 helper 模式（`create_state`, `setup_app`, `make_request` 等）
- Search/notification/audit 扩展在测试中使用 noop 实现
- 所有文档（AGENTS.md, PROGRESS-PLAN, ROADMAP en/zh, README.md, PROGRESS-REPORT）必须同步更新测试基线

## 进展

### 已完成
- [x] **Flaky test 修复**: `retry::tests::delay_increases_exponentially` — jitter 阈值从 1.5 降至 1.25（数学：最坏比率 = 2×0.8/(1×1.2) ≈ 1.333 > 1.25）
- [x] **19 个新集成测试** (61→80)：覆盖 healthz, readyz, auth/me, logout, change-password, preferences, settings/definitions, dashboard/stats, notifications (list/unread-count/mark-all-read), audit-log, global search, search rebuild, users/search
- [x] **验证**: `cargo test --workspace` = 411 tests, 0 failures
- [x] **验证**: `cargo test --workspace --features multi-db` = 0 failures
- [x] **Lint**: `cargo clippy` 0 warnings, `cargo fmt` 已格式化
- [x] **PROGRESS-REPORT 更新**: 387→411, per-crate stats 修正（ingjoo-bin 2→80, ingjoo-infra 170→197, ingjoo-security —→22, ingjoo-queue 22→25, ingjoo-macros —→4）
- [x] **AGENTS.md 更新**: 测试基线 411
- [x] **README.md 更新**: 测试数量 411
- [x] **ROADMAP (en/zh) 更新**: per-crate 统计修正, T4.4 🔄→✅, 总计 411
- [x] **PROGRESS-PLAN 更新**: 基线 411
- [x] **技术债务更新**: retry flaky test 已解决

### 测试分布（当前基线）

| 来源 | 测试数 |
|------|--------|
| ingjoo-bin (integration_test) | 80 |
| ingjoo-core (lib) | 82 |
| ingjoo-infra (lib + tests) | 197 |
| ingjoo-security (lib) | 22 |
| ingjoo-queue (lib + tests) | 25 |
| ingjoo-cache (lib) | 5 |
| ingjoo-macros (derive_test) | 4 |
| **Total** | **411** |

## 关键决策
- Search/rebuild 测试接受 200 或 503（noop 搜索引擎返回"搜索服务未启用" → handler 映射为 503）
- Jitter 阈值 1.25 有数学安全边际（1.333 > 1.25）

## 文件变更清单
- `source/ingjoo/crates/ingjoo-queue/src/retry.rs` — jitter 阈值修复
- `source/ingjoo/crates/ingjoo-bin/tests/integration_test.rs` — 19 新测试
- `docs/PROGRESS-REPORT-2026-04-26.md` — 387→411, per-crate stats
- `AGENTS.md` — 测试基线
- `source/ingjoo/README.md` — 测试数量
- `source/ingjoo/dev-docs/en/ROADMAP.md` — per-crate stats + T4.4
- `source/ingjoo/dev-docs/zh/ROADMAP.md` — per-crate stats + T4.4
- `docs/PROGRESS-PLAN-2026-04-25.md` — 基线更新

## 下一步建议
1. Header 通知铃铛接线（对接 `/api/notifications/unread-count`）
2. E2E 测试框架（Playwright）
3. 剩余未测试端点：attachments, events SSE, modules, WebSocket
4. Phase 12 (multi-db) ROADMAP 条目
