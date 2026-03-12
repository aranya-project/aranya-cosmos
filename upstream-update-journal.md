# Upstream Update Journal

## Overview
Update of the aranya-cosmos fork (aranya-project/aranya-cosmos) to match upstream aranya v5.0.0, preserving all COSMOS ground-to-flight command integration functionality.

- **Fork**: aranya-project/aranya-cosmos
- **Upstream**: aranya-project/aranya (v3.0.0 → v5.0.0)
- **Branch**: `update/upstream-sync`
- **PR**: https://github.com/aranya-project/aranya-cosmos/pull/8

## Status: PR OPEN (not merged)

## Upstream Breaking Changes (v5.0.0)
- **AQC removed**: Aranya Quick Channels feature dropped entirely
- **Rank-based roles**: Enum roles (`Role::Owner`, `Role::Member`) replaced with rank-based custom roles (`Rank`, `RoleId`, `Perm`)
- **Client modularized**: `client.rs` split into `client/{device.rs, label.rs, role.rs, team.rs, object.rs}`
- **API trait refactored**: `DaemonApi` trait methods renamed/restructured, `create_ctx()` replaces `context::current()`, `SessionData` replaces `(Vec<Box<[u8]>>, Vec<Effect>)` tuples
- **Action pattern changed**: `call_session_action(policy::action_name(...))` replaces old `session_action` + `VmAction`/`ident!`/`Value` pattern

## Steps Taken

### Phase 0: Pre-Rebase Preparation
1. **Explored fork custom changes** — identified all COSMOS integration touchpoints: policy (TaskCamera ephemeral command), daemon API (`task_camera`/`receive_cosmos_ctrl`), client methods, C API binding, cosmos-gate example app
2. **Analyzed upstream divergence** — 54 commits behind, 46 ahead; identified 17 conflicted files categorized by risk tier
3. **Wrote detailed rebase plan** — conflict resolution guide with exact code blocks to preserve, saved at `.claude/plans/bright-launching-dragonfly.md`
4. **Wrote COSMOS regression tests** — `crates/aranya-client/tests/cosmos.rs` with two tests:
   - `test_cosmos_task_camera_roundtrip`: owner issues task_camera targeting membera, membera receives and verifies
   - `test_cosmos_task_camera_wrong_name`: verifies rejection when task name doesn't match

### Phase 1: Rebase
5. **Created backup branch** (`backup/main-pre-upstream-update`) and update branch (`update/upstream-sync`)
6. **Rebased 40 fork commits onto upstream/main** — resolved 17+ conflict files across all tiers

### Phase 2: Post-Rebase Fixes
7. **Fixed compile errors in aranya-daemon**:
   - `actions.rs`: Updated `task_camera` from old `VmAction`/`ident!`/`session_action` pattern to `call_session_action(policy::task_camera(...))`; return type `SessionData`
   - `api.rs`: `DeviceId::transmute()` instead of `into_id()`, `session_new(graph)` without borrow, made `SessionData` import unconditional (removed `#[cfg(feature = "afc")]` gate)
   - `policy.rs`: Added `CameraTaskReceived` to Effect enum, `task_camera` to EphemeralAction enum

8. **Fixed compile errors in cosmos-gate**:
   - `KeyBundle` → `PublicKeyBundle`
   - `daemon_uds_path()` → `with_daemon_uds_path()`
   - `get_key_bundle()` → `get_public_key_bundle()`
   - `add_device_to_team(pk)` → `add_device(pk, None, Rank::new(0))`
   - Removed `aqc_server_addr()` call
   - `SocketAddr` → `Addr` for sync addresses

9. **Fixed workspace feature unification issue**:
   - **Root cause**: Root `Cargo.toml` had `features = ["preview", "afc", "experimental"]` on `aranya-client` and `aranya-daemon` workspace deps. This caused `aranya-daemon-api` to receive `preview` via feature unification (expanding `DaemonApi` trait to include `sync_hello_*` methods), while `aranya-daemon` itself didn't get `preview`, excluding its `#[cfg(feature = "preview")]` impl blocks → trait mismatch error
   - **Fix**: Removed hardcoded features from workspace deps to match upstream. Restructured `cosmos-gate/Cargo.toml` to use workspace deps with feature flags (same pattern as `aranya-example`)

10. **Fixed TaskCamera policy**:
    - **Removed `is_device_on_team()`** — function doesn't exist in upstream policy; `get_device()` already validates device existence via `check_unwrap`
    - **Removed `is_member(peer.role)`** — function doesn't exist upstream; `peer` variable was also undefined
    - **Added proper authorization**: `get_assigned_role(author.device_id)` + `is_owner(author_role)` for owner check
    - **Added explicit `finish` blocks for all code paths**: `finish {}` for author, `finish { emit CameraTaskReceived{...} }` for recipient, `check false` for others. Previous version had no `finish` for the author branch, causing a policy VM panic

11. **Updated COSMOS tests**: Added `setup_default_roles(team_id)` call before `add_all_device_roles(team_id, &roles)` to match upstream's new API

12. **Added `cargo-all-features` metadata** to cosmos-gate: `always_include_features = ["default"]` to prevent `compile_error!` when testing without default features

### Phase 3: Verification
13. **All tests passing**:
    - `cargo check --workspace` — clean
    - `cargo make test` — all feature combinations pass
    - 48 upstream integration tests pass (`cargo test -p aranya-client --test tests`)
    - 2 COSMOS roundtrip tests pass (`cargo test -p aranya-client --test cosmos`)

### Phase 4: PR
14. **Committed post-rebase fix** as `57434bc9` (signed with SSH key)
15. **Pushed to public remote** and opened PR #8

## Files Modified (Post-Rebase Fixes)
| File | Changes |
|------|---------|
| `Cargo.toml` (root) | Removed `features = ["preview", "afc", "experimental"]` from workspace deps |
| `Cargo.lock` | Regenerated |
| `crates/aranya-daemon/src/actions.rs` | Updated `task_camera` to new action pattern |
| `crates/aranya-daemon/src/api.rs` | Fixed `task_camera` + `receive_cosmos_ctrl` impls for new types |
| `crates/aranya-daemon/src/policy.md` | Fixed TaskCamera policy section |
| `crates/aranya-daemon/src/policy.rs` | Added CameraTaskReceived effect + task_camera action structs |
| `crates/aranya-client/src/client/team.rs` | Added COSMOS methods to modular Team struct |
| `crates/aranya-client/tests/cosmos.rs` | Updated for `setup_default_roles` API |
| `examples/rust/cosmos-gate/Cargo.toml` | Workspace deps + feature flags + cargo-all-features metadata |
| `examples/rust/cosmos-gate/src/lib.rs` | Updated for upstream API changes |
| `examples/rust/example/README.md` | Removed (orphaned directory) |

### Phase 5: Cosmos-Gate Fixes & Integration Tests

16. **Removed stale AQC config from cosmos-gate daemon config** (`lib.rs`):
    - Removed `aqc.enable = true` from the generated TOML config passed to the daemon — AQC was removed upstream in v5.0.0
    - Updated `README.md` build instructions: removed `aqc` from `--features` flag

17. **Fixed `sync_now` target address bug** (`lib.rs:364`):
    - `member_team.sync_now(member_addr, None)` was syncing the member with *itself* instead of the owner
    - Fixed to `member_team.sync_now(owner_addr, None)`

18. **Fixed sync ordering issue** (`lib.rs`):
    - `add_sync_peer` (background sync, 400ms interval) was called *before* the initial `sync_now`, causing the background sync manager to race with the one-shot sync and produce QUIC connection conflicts (`application::Error(0)`)
    - Reordered: `sync_now` first, then `add_sync_peer` — eliminates the race

19. **Added cosmos-gate integration tests** (`tests/integration.rs`, 6 tests):
    - **Init tests**:
      - `test_init_creates_team_and_persists_state` — verifies `.aranya_initialized`, `.aranya_team_id`, `.aranya_member_id` files are written and parseable
      - `test_init_idempotent` — verifies re-init with `already_initialized=true` reads existing state, returns same team_id
    - **Server tests**:
      - `test_server_authorize_returns_bytes` — POST `/authorize` with valid `CMDSummary` JSON returns 200 + non-empty command bytes
      - `test_server_authorize_invalid_body` — POST with missing fields returns 422
      - `test_server_authorize_different_packet_name` — different packet names produce valid bytes
      - `test_server_authorize_roundtrip_with_member` — full roundtrip: server produces command bytes via `/authorize`, member verifies them with `receive_cosmos_ctrl`
    - Tests use unique SHM names per test to allow parallel execution
    - Tests spawn real daemon processes via `ClientCtx::new()` and test against the axum router via `tower::ServiceExt::oneshot`

20. **Updated cosmos-gate `Cargo.toml`**: added `tower` + `http-body-util` dev-dependencies, `[[test]]` target

## Files Modified (Phase 5)
| File | Changes |
|------|---------|
| `examples/rust/cosmos-gate/src/lib.rs` | Removed `aqc.enable`, fixed `sync_now` addr, reordered sync setup |
| `examples/rust/cosmos-gate/README.md` | Removed `aqc` from `--features` |
| `examples/rust/cosmos-gate/Cargo.toml` | Added dev-deps (`tower`, `http-body-util`), test target |
| `examples/rust/cosmos-gate/tests/integration.rs` | New: 6 integration tests for init + server |

## Key Decisions
- **Dropped `aqc` feature** — removed upstream, our code didn't use AQC directly (just had the feature flag)
- **TaskCamera authorization** — owner check via `get_assigned_role()` + `is_owner()` (upstream removed enum-based role checks)
- **cosmos-gate dependency pattern** — uses workspace deps with feature flags, matching `aranya-example` convention
- **Workspace deps** — no hardcoded features on workspace dependency definitions (matches upstream)
- **Policy finish blocks** — every ephemeral command code path must end with `finish` (learned from policy VM panic)
