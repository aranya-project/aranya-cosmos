use std::{
    env,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
    sync::Arc,
};

use anyhow::{Context as _, Result};
use aranya_client::{
    client::{Client, DeviceId, KeyBundle},
    AddTeamConfig, AddTeamQuicSyncConfig, CreateTeamConfig, CreateTeamQuicSyncConfig,
    SyncPeerConfig,
};
use aranya_util::Addr;
use axum::{http::StatusCode, routing::post, Json, Router, extract::State};
use backon::{ExponentialBuilder, Retryable};
use serde::Deserialize;
use tokio::{fs, process::Child, process::Command, time::sleep};
use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, prelude::*, util::SubscriberInitExt, EnvFilter};

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
    async fn spawn(path: &DaemonPath, user_name: &str, work_dir: &Path) -> Result<Self> {
        fs::create_dir_all(&work_dir).await?;

        // Prepare daemon dirs and config.
        let user_name = user_name;
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

            aqc.enable = true

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
    client: Arc<Client>,
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
        let daemon = Daemon::spawn(daemon_path, user_name, &work_dir).await?;

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
            client: Arc::new(client),
            pk,
            id,
            _work_dir: work_dir,
            _daemon: daemon,
        })
    }

    async fn aranya_local_addr(&self) -> Result<SocketAddr> {
        Ok(self.client.local_addr().await?)
    }
}

#[derive(Clone)]
struct AppState {
    owner: Arc<Client>,
}

// Map summary object of dispatcher POST requests.
#[derive(Deserialize)]
struct CMDSummary {
    pub keycloak_id: String,
    pub target: String,
    pub packet_name: String,
    #[serde(deserialize_with = "deserialize_hex_u16")]
    pub stream_id: u16,
    pub function_code: u16,
}

fn deserialize_hex_u16<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    struct HexVisitor;
    impl<'de> serde::de::Visitor<'de> for HexVisitor {
        type Value = u16;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "a hex string (e.g., \"0x1A2B\" or \"1A2B\") or a number 0-65535")
        }
        fn visit_u64<E>(self, v: u64) -> Result<u16, E>
        where
            E: serde::de::Error,
        {
            u16::try_from(v).map_err(|_| E::custom("number out of range for u16"))
        }
        fn visit_str<E>(self, v: &str) -> Result<u16, E>
        where
            E: serde::de::Error,
        {
            let s = v.trim();
            let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
            u16::from_str_radix(s, 16).map_err(|_| E::custom("invalid hex u16"))
        }
        fn visit_string<E>(self, v: String) -> Result<u16, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&v)
        }
    }
    deserializer.deserialize_any(HexVisitor)
}

async fn handle_post(State(state): State<AppState>, Json(body): Json<CMDSummary>) -> (StatusCode, String) {
    // Minimal echo; extend to use `ClientCtx` if needed.
    info!(
        "received POST /data: keycloak_id={} target={} packet_name={} stream_id=0x{:04X} function_code={}",
        &body.keycloak_id,
        &body.target,
        &body.packet_name,
        body.stream_id,
        body.function_code
    );

    info!("simulating command processing...");
    // Example: call a method on the owner client (fetch owner device id).
    match state.owner.get_device_id().await {
        Ok(owner_id) => {
            info!(?owner_id, "owner client device id");
            (StatusCode::ACCEPTED, format!("CMD ok: {} (owner_id={owner_id:?})", body.packet_name))
        }
        Err(e) => {
            // Keep response short; log the error detail.
            info!("owner client call failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "owner call failed".to_string())
        }
    }
}

// Args: <daemon_path> <owner_work_dir> <member_work_dir> [rest_bind_addr]
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
    let member_dir = args.next().context("missing <member_work_dir>")?;
    let bind = args
        .next()
        .unwrap_or_else(|| "127.0.0.1:8080".to_string())
        .parse::<SocketAddr>()
        .context("invalid [rest_bind_addr]")?;

    let daemon_path = DaemonPath(daemon_exe.into());

    // Resolve work dirs and init marker path (for onboarding once).
    let owner_dir_pb = PathBuf::from(&owner_dir);
    let member_dir_pb = PathBuf::from(&member_dir);
    let init_marker = owner_dir_pb.join(".aranya_initialized");
    let already_initialized = fs::metadata(&init_marker).await.is_ok();

    // Initialize owner and member contexts.
    let owner = ClientCtx::new("owner", &daemon_path, owner_dir_pb.clone()).await?;
    // Keep the member daemon alive even if we skip onboarding; underscore avoids unused warning.
    let _member = ClientCtx::new("member", &daemon_path, member_dir_pb.clone()).await?;

    if already_initialized {
        info!("existing initialization detected; skipping onboarding and using persisted daemon state");
    } else {
        // Create team on owner.
        info!("creating team (first-time onboarding)");
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
        let owner_team = owner
            .client
            .create_team(owner_cfg)
            .await
            .context("create team")?;
        let team_id = owner_team.team_id();
        info!(%team_id, "team created");

        // Onboard member.
        let add_team_cfg = {
            let qs_cfg = AddTeamQuicSyncConfig::builder()
                .seed_ikm(seed_ikm)
                .build()?;
            AddTeamConfig::builder()
                .quic_sync(qs_cfg)
                .team_id(team_id)
                .build()?
        };
        let member_team = _member.client.add_team(add_team_cfg).await?;
        owner_team.add_device_to_team(_member.pk.clone()).await?;
        info!("member added to team");

        // Setup sync peers.
        let sync_interval = Duration::from_millis(400);
        let sync_cfg = SyncPeerConfig::builder().interval(sync_interval).build()?;
        let owner_addr = owner.aranya_local_addr().await?;
        let member_addr = _member.aranya_local_addr().await?;
        owner_team
            .add_sync_peer((member_addr).into(), sync_cfg.clone())
            .await?;
        member_team
            .add_sync_peer((owner_addr).into(), sync_cfg.clone())
            .await?;

        // Wait a moment for sync.
        sleep(sync_interval * 4).await;

        // Create marker to skip onboarding next time.
        fs::write(&init_marker, b"ok").await?;
        info!("onboarding complete; marker written at {}", init_marker.display());
    }

    // Start REST API in this Tokio runtime and pass owner client as state.
    let state = AppState {
        owner: owner.client.clone(),
    };
    let app = Router::new()
        .route("/authorize", post(handle_post))
        .with_state(state);
    info!("REST listening on http://{}", bind);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
