use anyhow::{Context as _, Result};
use std::{env, path::PathBuf};
use tracing_subscriber::{layer::SubscriberExt, prelude::*, util::SubscriberInitExt, EnvFilter};

// Import from the local lib crate.
use cosmos_gate::{
    init_marker_path, initialize_or_return, member_id_path, team_id_path, ClientCtx, DaemonPath,
};

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

    // Args: <daemon_path> <owner_work_dir> <member_work_dir>
    let mut args = env::args();
    let _exe = args.next();
    let daemon_exe = args.next().context("missing <daemon_path>")?;
    let owner_dir = args.next().context("missing <owner_work_dir>")?;
    let member_dir = args.next().context("missing <member_work_dir>")?;

    let daemon_path = DaemonPath(daemon_exe.into());
    let owner_dir_pb = PathBuf::from(&owner_dir);
    let member_dir_pb = PathBuf::from(&member_dir);

    let init_marker = init_marker_path(&owner_dir_pb);
    let team_id_path = team_id_path(&owner_dir_pb);
    let member_id_path = member_id_path(&owner_dir_pb);
    let already_initialized = tokio::fs::metadata(&init_marker).await.is_ok();

    // Spawn daemons and clients
    let owner = ClientCtx::new("owner", &daemon_path, owner_dir_pb.clone()).await?;
    let member = ClientCtx::new("member", &daemon_path, member_dir_pb.clone()).await?;

    // Onboard (or print info if already initialized) and exit.
    let _ = initialize_or_return(
        &owner,
        &member,
        &init_marker,
        &team_id_path,
        &member_id_path,
        already_initialized,
    )
    .await?;
    Ok(())
}
