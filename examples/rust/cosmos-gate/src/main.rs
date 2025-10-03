use std::{
    env,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    str::FromStr,
    thread,
    time::Duration,
};

use anyhow::{bail, Context as _, Result};
use aranya_client::{
    client::{Client, DeviceId, KeyBundle, NetIdentifier},
    AddTeamConfig, AddTeamQuicSyncConfig, CreateTeamConfig, CreateTeamQuicSyncConfig,
    SyncPeerConfig,
};
use aranya_util::Addr;
use axum::{http::StatusCode, routing::post, Json, Router};
use backon::{ExponentialBuilder, Retryable};
use bytes::Bytes;
use serde::Deserialize;
use tokio::{fs, process::Child, process::Command, time::sleep};
use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, prelude::*};

#[derive(Clone, Debug)]
struct DaemonPath(PathBuf);

#[derive(Debug)]
#[clippy::has_significant_drop]
struct Daemon {
    // NB: This has important drop side effects.
    _proc: Child,
    _work_dir: PathBuf,
}

impl Daemon {
    async fn spawn(path: &DaemonPath, work_dir: &Path) -> Result<Self> {
        fs::create_dir_all(&work_dir).await?;

        // Prepare daemon dirs and config.
        // TODO: fix user name variable
        let user_name = work_dir
            .file_name()
            .and_then(|n| n.to_str())
            .context("work_dir name UTF-8")?;
        let shm = format!("/shm_{}", user_name);
        let runtime_dir = work_dir.join("run");
        let state_dir = work_dir.join("state");
        let cache_dir = work_dir.join("cache");
        let logs_dir = work_dir.join("logs");
        let config_dir = work_dir.join("config");
        for dir in &[&runtime_dir, &state_dir, &cache_dir, &logs_dir, &config_dir] {
            fs::create_dir_all(dir)
                .await
                .with_context(|| format!("unable to create directory: {}", dir.display()))?;
        }

        let cfg_path = work_dir.join("config.toml");
        let cfg_buf = format!(
            r#"
            name = {user_name:?}
            runtime_dir = {runtime_dir:?}
            state_dir = {state_dir:?}
            cache_dir = {cache_dir:?}
            logs_dir = {logs_dir:?}
            config_dir = {config_dir:?}

            [afc]
            enable = true
            shm_path = {shm:?}
            max_chans = 100

            [sync.quic]
            enable = true
            addr = "127.0.0.1:0"
            "#
        );
        fs::write(&cfg_path, cfg_buf).await?;

        // Spawn daemon.
        let cfg_path = cfg_path.as_os_str().to_str().context("cfg_path UTF-8")?;
        let mut cmd = Command::new(&path.0);
        cmd.kill_on_drop(true)
            .current_dir(work_dir)
            .args(["--config", cfg_path]);
        debug!(?cmd, "spawning daemon");
        let proc = cmd.spawn().context("unable to spawn daemon")?;
        Ok(Daemon {
            _proc: proc,
            _work_dir: work_dir.into(),
        })
    }
}

struct ClientCtx {
    client: Client,
    pk: KeyBundle,
    id: DeviceId,
    // keep daemon alive
    _work_dir: PathBuf,
    _daemon: Daemon,
}

impl ClientCtx {
    async fn new(user_name: &str, daemon_path: &DaemonPath, work_dir: PathBuf) -> Result<Self> {
        info!(user_name, "creating `ClientCtx`");

        // Spawn daemon in given work_dir.
        let daemon = Daemon::spawn(daemon_path, &work_dir).await?;

        // UDS path the daemon listens on.
        let uds_sock = work_dir.join("run").join("uds.sock");

        // Give the daemon a moment to start and bind its UDS.
        sleep(Duration::from_millis(100)).await;

        // Connect client.
        let any_addr = Addr::from((Ipv4Addr::LOCALHOST, 0));
        let client = (|| {
            Client::builder()
                .daemon_uds_path(&uds_sock)
                .aqc_server_addr(&any_addr)
                .connect()
        })
        .retry(ExponentialBuilder::default())
        .await
        .context("unable to initialize client")?;

        let pk = client
            .get_key_bundle()
            .await
            .context("expected key bundle")?;
        let id = client.get_device_id().await.context("expected device id")?;

        Ok(Self {
            client,
            pk,
            id,
            _work_dir: work_dir,
            _daemon: daemon,
        })
    }

    async fn aranya_local_addr(&self) -> Result<SocketAddr> {
        Ok(self.client.local_addr().await?)
    }

    fn aqc_net_id_from(&self, addr: SocketAddr) -> NetIdentifier {
        NetIdentifier::from_str(addr.to_string().as_str()).expect("net identifier")
    }
}

#[derive(Deserialize)]
struct PostData {
    message: String,
}

async fn handle_post(Json(body): Json<PostData>) -> (StatusCode, String) {
    // Minimal echo; extend to use `ClientCtx` if needed.
    info!("received POST /data: {}", body.message);
    (StatusCode::ACCEPTED, format!("ok: {}", body.message))
}

fn spawn_rest(bind: SocketAddr) -> thread::JoinHandle<Result<()>> {
    thread::spawn(move || -> Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();

        rt?.block_on(async move {
            let app = Router::new()
                .route("/authorize", post(handle_post));

            info!("REST listening on http://{}", bind);

            let listener = tokio::net::TcpListener::bind(bind)
                            .await
                            .unwrap();

            axum::serve(listener, app)
                .await
                .unwrap();

            Ok::<_, anyhow::Error>(())
        })?;
        Ok(())
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_file(false)
                .with_target(false)
                .compact()
                .with_filter(EnvFilter::try_from_env("COSMOS_GATE_LOG").unwrap_or_else(|_| EnvFilter::new("info"))),
        )
        .init();

    // Args: <daemon_path> <owner_work_dir> <member_work_dir> [rest_bind_addr]
    let mut args = env::args();
    let _exe = args.next();
    let daemon_exe = args.next().context("missing <daemon_path>")?;
    let owner_dir = args.next().context("missing <owner_work_dir>")?;
    let member_dir = args.next().context("missing <member_work_dir>")?;
    let bind = args
        .next()
        .unwrap_or_else(|| "127.0.0.1:8080".to_string())
        .parse::<SocketAddr>()
        .context("invalid [rest_bind_addr]")?;

    let daemon_path = DaemonPath(daemon_exe.into());

    // Initialize owner and member contexts.
    let owner = ClientCtx::new("owner", &daemon_path, PathBuf::from(owner_dir)).await?;
    let member = ClientCtx::new("member", &daemon_path, PathBuf::from(member_dir)).await?;

    // Create team on owner.
    info!("creating team");
    let seed_ikm = {
        let mut buf = [0u8; 32];
        owner.client.rand(&mut buf).await;
        buf
    };
    let owner_cfg = {
        let qs_cfg = CreateTeamQuicSyncConfig::builder()
            .seed_ikm(seed_ikm)
            .build()?;
        CreateTeamConfig::builder().quic_sync(qs_cfg).build()?
    };
    let owner_team = owner.client.create_team(owner_cfg).await.context("create team")?;
    let team_id = owner_team.team_id();
    info!(%team_id, "team created");

    // Onboard member.
    let add_team_cfg = {
        let qs_cfg = AddTeamQuicSyncConfig::builder().seed_ikm(seed_ikm).build()?;
        AddTeamConfig::builder().quic_sync(qs_cfg).team_id(team_id).build()?
    };
    let member_team = member.client.add_team(add_team_cfg).await?;
    owner_team.add_device_to_team(member.pk.clone()).await?;
    info!("member added to team");

    // Setup sync peers.
    let sync_interval = Duration::from_millis(200);
    let sync_cfg = SyncPeerConfig::builder().interval(sync_interval).build()?;
    let owner_addr = owner.aranya_local_addr().await?;
    let member_addr = member.aranya_local_addr().await?;
    owner_team.add_sync_peer(member.aqc_net_id_from(member_addr).into(), sync_cfg.clone()).await?;
    member_team.add_sync_peer(member.aqc_net_id_from(owner_addr).into(), sync_cfg.clone()).await?;

    // Wait a moment for sync.
    sleep(sync_interval * 4).await;

    // Start REST API (dedicated thread).
    let rest = spawn_rest(bind);

    // Keep running (join the REST thread).
    if let Err(e) = rest.join().unwrap_or_else(|_| bail!("REST thread panicked")) {
        return Err(e);
    }
    Ok(())
}
