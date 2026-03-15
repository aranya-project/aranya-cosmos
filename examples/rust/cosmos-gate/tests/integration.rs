//! Integration tests for cosmos-gate init and server flows.
//!
//! Prerequisites: build the daemon binary first:
//!   cargo build --bin aranya-daemon
//!
//! Run:
//!   cargo test -p cosmos-gate

#![allow(
    clippy::disallowed_macros,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    rust_2018_idioms
)]

use std::path::{Path, PathBuf};

use anyhow::Result;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use cosmos_gate::{
    build_router, init_marker_path, initialize_or_return, member_id_path, read_member_id,
    read_team_id, team_id_path, AppState, ClientCtx, DaemonPath,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// Locate the aranya-daemon binary in the workspace target directory.
fn find_daemon_binary() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // cosmos-gate is at examples/rust/cosmos-gate, workspace root is 3 levels up.
    let workspace_root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    let release_path = workspace_root.join("target/release/aranya-daemon");
    let debug_path = workspace_root.join("target/debug/aranya-daemon");

    if release_path.exists() {
        return release_path;
    }
    if debug_path.exists() {
        return debug_path;
    }
    panic!(
        "aranya-daemon binary not found at {} or {}. \
         Run `cargo build --bin aranya-daemon` first.",
        debug_path.display(),
        release_path.display()
    );
}

/// Run the full init flow in a temp directory and return the contexts + paths.
///
/// `test_name` is used as a prefix for daemon user names to avoid SHM
/// path collisions when tests run in parallel.
async fn run_init(
    test_name: &str,
    daemon_path: &DaemonPath,
    work_dir: &Path,
) -> Result<(ClientCtx, ClientCtx, PathBuf, PathBuf, PathBuf)> {
    let owner_dir = work_dir.join("owner");
    let member_dir = work_dir.join("member");

    let init_marker = init_marker_path(&owner_dir);
    let tid_path = team_id_path(&owner_dir);
    let mid_path = member_id_path(&owner_dir);

    let owner_name = format!("{test_name}_owner");
    let member_name = format!("{test_name}_member");

    let owner = ClientCtx::new(&owner_name, daemon_path, owner_dir).await?;
    let member = ClientCtx::new(&member_name, daemon_path, member_dir).await?;

    initialize_or_return(&owner, &member, &init_marker, &tid_path, &mid_path, false).await?;

    Ok((owner, member, init_marker, tid_path, mid_path))
}

// ---------------------------------------------------------------------------
// Init tests
// ---------------------------------------------------------------------------

/// Fresh initialization creates a team, onboards the member, and persists
/// the init marker, team_id, and member_id files.
#[tokio::test(flavor = "multi_thread")]
async fn test_init_creates_team_and_persists_state() -> Result<()> {
    let daemon_path = DaemonPath(find_daemon_binary());
    let work_dir = tempfile::tempdir()?;

    let (_owner, _member, init_marker, tid_path, mid_path) =
        run_init("init_state", &daemon_path, work_dir.path()).await?;

    // Verify state files were written.
    assert!(
        tokio::fs::metadata(&init_marker).await.is_ok(),
        "init marker should exist"
    );
    assert!(
        tokio::fs::metadata(&tid_path).await.is_ok(),
        "team_id file should exist"
    );
    assert!(
        tokio::fs::metadata(&mid_path).await.is_ok(),
        "member_id file should exist"
    );

    // Verify the persisted IDs are parseable.
    let team_id = read_team_id(&tid_path).await?;
    let member_id = read_member_id(&mid_path).await?;
    assert_eq!(member_id, _member.id, "persisted member_id should match");

    // Verify owner got a valid team_id (non-zero check via string repr).
    let tid_str = team_id.to_string();
    assert!(!tid_str.is_empty(), "team_id should be non-empty");

    Ok(())
}

/// Re-initialization (already_initialized=true) reads IDs from files
/// without creating a new team.
#[tokio::test(flavor = "multi_thread")]
async fn test_init_idempotent() -> Result<()> {
    let daemon_path = DaemonPath(find_daemon_binary());
    let work_dir = tempfile::tempdir()?;

    // First init.
    let (owner, member, init_marker, tid_path, mid_path) =
        run_init("init_idem", &daemon_path, work_dir.path()).await?;

    let original_team_id = read_team_id(&tid_path).await?;

    // Second init with already_initialized=true.
    let team_id = initialize_or_return(&owner, &member, &init_marker, &tid_path, &mid_path, true)
        .await?;

    assert_eq!(
        team_id, original_team_id,
        "re-init should return the same team_id"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Server / router tests
// ---------------------------------------------------------------------------

/// Helper: set up an initialized team and return an AppState + both ClientCtx.
/// Both contexts must be kept alive for the daemon processes to continue running.
async fn setup_server_state(
    test_name: &str,
    daemon_path: &DaemonPath,
    work_dir: &Path,
) -> Result<(AppState, ClientCtx, ClientCtx)> {
    let (owner, member, _init_marker, tid_path, mid_path) =
        run_init(test_name, daemon_path, work_dir).await?;

    let team_id = read_team_id(&tid_path).await?;
    let member_id = read_member_id(&mid_path).await?;

    let state = AppState {
        owner: owner.client.clone(),
        owner_team_id: team_id,
        target_member_id: member_id,
    };

    // Return both contexts so their daemon processes stay alive.
    Ok((state, owner, member))
}

/// POST /authorize with a valid CMDSummary returns 200 and non-empty bytes.
#[tokio::test(flavor = "multi_thread")]
async fn test_server_authorize_returns_bytes() -> Result<()> {
    let daemon_path = DaemonPath(find_daemon_binary());
    let work_dir = tempfile::tempdir()?;
    let (state, _owner, _member) =
        setup_server_state("srv_bytes", &daemon_path, work_dir.path()).await?;

    let app = build_router(state);

    let body = serde_json::json!({
        "keycloak_id": "user-123",
        "target": "sat-1",
        "packet_name": "photo_earth",
        "stream_id": "0x00FF",
        "function_code": 42
    });

    let request = Request::builder()
        .method("POST")
        .uri("/authorize")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body)?))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "POST /authorize should return 200"
    );

    let bytes = response.into_body().collect().await?.to_bytes();
    assert!(
        !bytes.is_empty(),
        "response body should contain serialized command bytes"
    );

    Ok(())
}

/// POST /authorize with an invalid JSON body returns 422 (Unprocessable Entity).
#[tokio::test(flavor = "multi_thread")]
async fn test_server_authorize_invalid_body() -> Result<()> {
    let daemon_path = DaemonPath(find_daemon_binary());
    let work_dir = tempfile::tempdir()?;
    let (state, _owner, _member) =
        setup_server_state("srv_invalid", &daemon_path, work_dir.path()).await?;

    let app = build_router(state);

    // Missing required fields.
    let body = serde_json::json!({"keycloak_id": "user-123"});

    let request = Request::builder()
        .method("POST")
        .uri("/authorize")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body)?))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid body should return 422"
    );

    Ok(())
}

/// POST /authorize with a different packet_name still produces valid bytes.
#[tokio::test(flavor = "multi_thread")]
async fn test_server_authorize_different_packet_name() -> Result<()> {
    let daemon_path = DaemonPath(find_daemon_binary());
    let work_dir = tempfile::tempdir()?;
    let (state, _owner, _member) =
        setup_server_state("srv_diffpkt", &daemon_path, work_dir.path()).await?;

    let app = build_router(state);

    let body = serde_json::json!({
        "keycloak_id": "user-456",
        "target": "sat-2",
        "packet_name": "photo_mars",
        "stream_id": 255,
        "function_code": 7
    });

    let request = Request::builder()
        .method("POST")
        .uri("/authorize")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body)?))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await?.to_bytes();
    assert!(!bytes.is_empty());

    Ok(())
}

/// Verify the member can receive and validate the command bytes produced by
/// the server's /authorize endpoint (full roundtrip).
#[tokio::test(flavor = "multi_thread")]
async fn test_server_authorize_roundtrip_with_member() -> Result<()> {
    let daemon_path = DaemonPath(find_daemon_binary());
    let work_dir = tempfile::tempdir()?;
    let (owner, member, _init_marker, tid_path, _mid_path) =
        run_init("srv_roundtrip", &daemon_path, work_dir.path()).await?;

    let team_id = read_team_id(&tid_path).await?;

    let state = AppState {
        owner: owner.client.clone(),
        owner_team_id: team_id,
        target_member_id: member.id,
    };

    let app = build_router(state);

    let body = serde_json::json!({
        "keycloak_id": "user-789",
        "target": "sat-1",
        "packet_name": "photo_earth",
        "stream_id": "0x0001",
        "function_code": 1
    });

    let request = Request::builder()
        .method("POST")
        .uri("/authorize")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body)?))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let cmd_bytes = response.into_body().collect().await?.to_bytes();
    assert!(!cmd_bytes.is_empty());

    // Member verifies the command bytes.
    use aranya_policy_text::Text;
    use std::str::FromStr;

    let member_team = member.client.team(team_id);

    // Sync member with owner so it has the latest graph state.
    let owner_addr = owner.aranya_local_addr().await?;
    member_team.sync_now(owner_addr, None).await?;

    member_team
        .receive_cosmos_ctrl(
            Text::from_str("photo_earth")?,
            cmd_bytes.to_vec().into_boxed_slice(),
        )
        .await
        .expect("member should successfully verify the command bytes");

    Ok(())
}
