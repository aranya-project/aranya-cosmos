use anyhow::{bail, Context as _, Result};
use axum::Router;
use std::{env, net::SocketAddr, path::PathBuf};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, prelude::*, util::SubscriberInitExt, EnvFilter};

use cosmos_gate::{
    build_router, init_marker_path, member_id_path, read_member_id, read_team_id, team_id_path,
    AppState, ClientCtx, DaemonPath,
};

/// Args: <daemon_path> <owner_work_dir> [rest_bind_addr]
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_file(false)
                .with_target(false)
                .compact()
                .with_filter(
                    EnvFilter::try_from_env("COSMOS_GATE_LOG")
                        .unwrap_or_else(|_| EnvFilter::new("info")),
                ),
        )
        .init();

    let mut args = env::args();
    let _exe = args.next();
    let daemon_exe = args.next().context("missing <daemon_path>")?;
    let owner_dir = args.next().context("missing <owner_work_dir>")?;
    let bind = args
        .next()
        .unwrap_or_else(|| "127.0.0.1:8080".to_string())
        .parse::<SocketAddr>()
        .context("invalid [rest_bind_addr]")?;

    let daemon_path = DaemonPath(daemon_exe.into());
    let owner_dir_pb = PathBuf::from(&owner_dir);

    // Require prior initialization.
    let init_marker = init_marker_path(&owner_dir_pb);
    let team_id_file = team_id_path(&owner_dir_pb);
    let member_id_file = member_id_path(&owner_dir_pb);
    if !tokio::fs::metadata(&init_marker).await.is_ok() {
        bail!("not initialized; run the init binary first to onboard");
    }
    let owner_team_id = read_team_id(&team_id_file).await?;
    let target_member_id = read_member_id(&member_id_file).await?;

    // Spawn owner daemon/client only (member no longer needed here).
    let owner = ClientCtx::new("owner", &daemon_path, owner_dir_pb.clone()).await?;

    // Build REST state and router.
    let state = AppState {
        owner: owner.client.clone(),
        owner_team_id,
        target_member_id,
    };
    let app: Router = build_router(state);

    info!("REST listening on http://{}", bind);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
