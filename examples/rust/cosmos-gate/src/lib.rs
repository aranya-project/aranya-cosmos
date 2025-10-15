use std::{
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context as _, Result};
use aranya_client::{
    client::{Client, DeviceId, KeyBundle},
    AddTeamConfig, AddTeamQuicSyncConfig, CreateTeamConfig, CreateTeamQuicSyncConfig,
    SyncPeerConfig, TeamId,
};
use aranya_policy_text::Text;
use aranya_util::Addr;
use axum::http::header::CONTENT_TYPE;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use backon::{ExponentialBuilder, Retryable};
use rustix::shm;
use serde::Deserialize;
use tokio::{fs, process::Child, process::Command, time::sleep};
use tracing::{debug, info};

#[derive(Clone, Debug)]
pub struct DaemonPath(pub PathBuf);

#[derive(Debug)]
#[clippy::has_significant_drop]
pub struct Daemon {
    // NB: This has important drop side effects.
    _proc: Child,
    _work_dir: PathBuf,
}

impl Daemon {
    pub async fn spawn(path: &DaemonPath, user_name: &str, work_dir: &Path) -> Result<Self> {
        fs::create_dir_all(&work_dir).await?;

        // Prepare daemon dirs and config.
        let shm = format!("/shm_{}", user_name);
        // Ensure no stale POSIX SHM exists from previous runs (matches aranya example).
        let _ = shm::unlink(&shm);

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

pub struct ClientCtx {
    pub client: Arc<Client>,
    pub pk: KeyBundle,
    pub id: DeviceId,
    // keep daemon alive
    _work_dir: PathBuf,
    _daemon: Daemon,
}

impl ClientCtx {
    pub async fn new(user_name: &str, daemon_path: &DaemonPath, work_dir: PathBuf) -> Result<Self> {
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

        // Fetch client identity info.
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

    pub async fn aranya_local_addr(&self) -> Result<SocketAddr> {
        Ok(self.client.local_addr().await?)
    }
}

// Convenience helpers for state files.
pub fn init_marker_path(owner_dir: &Path) -> PathBuf {
    owner_dir.join(".aranya_initialized")
}
pub fn team_id_path(owner_dir: &Path) -> PathBuf {
    owner_dir.join(".aranya_team_id")
}
// NEW: member_id file path and reader
pub fn member_id_path(owner_dir: &Path) -> PathBuf {
    owner_dir.join(".aranya_member_id")
}
pub async fn read_member_id(path: &Path) -> Result<DeviceId> {
    let s = fs::read_to_string(path)
        .await
        .context("unable to read member_id file")?;
    s.trim()
        .parse::<DeviceId>()
        .context("invalid member_id in file")
}
pub async fn read_team_id(path: &Path) -> Result<TeamId> {
    let s = fs::read_to_string(path)
        .await
        .context("unable to read team_id file")?;
    s.trim()
        .parse::<TeamId>()
        .context("invalid team_id in file")
}

#[derive(Clone)]
pub struct AppState {
    pub owner: Arc<Client>,
    pub owner_team_id: TeamId,
    // REPLACED: was `target_member: Arc<Client>`
    pub target_member_id: DeviceId,
}

// Map summary object of dispatcher POST requests.
#[derive(Deserialize)]
pub struct CMDSummary {
    pub keycloak_id: String,
    pub target: String,
    pub packet_name: String,
    #[serde(deserialize_with = "deserialize_hex_u16")]
    pub stream_id: u16,
    pub function_code: u16,
}

pub fn deserialize_hex_u16<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    struct HexVisitor;
    impl<'de> serde::de::Visitor<'de> for HexVisitor {
        type Value = u16;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(
                f,
                "a hex string (e.g., \"0x1A2B\" or \"1A2B\") or a number 0-65535"
            )
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
            let s = s
                .strip_prefix("0x")
                .or_else(|| s.strip_prefix("0X"))
                .unwrap_or(s);
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

pub async fn handle_post(State(state): State<AppState>, Json(body): Json<CMDSummary>) -> Response {
    // Minimal echo; extend to use `ClientCtx` if needed.
    info!(
        "received POST /authorize: keycloak_id={} target={} packet_name={} stream_id=0x{:04X} function_code={}",
        &body.keycloak_id,
        &body.target,
        &body.packet_name,
        body.stream_id,
        body.function_code
    );

    let owner_team = state.owner.team(state.owner_team_id);
    // TODO: make task lowercase
    let task_name = Text::try_from(body.packet_name.clone())
        .unwrap_or_else(|_| Text::from_str("unknown").unwrap());

    // Simplify: use persisted member id instead of a live client
    info!(
        "owner_id: {}, owner_team_id: {}",
        state.owner.get_device_id().await.unwrap(),
        state.owner_team_id
    );
    info!(
        "issuing task_camera to target client id: {}",
        state.target_member_id
    );

    match owner_team
        .task_camera(task_name, state.target_member_id)
        .await
    {
        Ok(serialized_cmd) => {
            info!("serialized_cmd produced: {} bytes", serialized_cmd.len());
            (
                StatusCode::OK,
                [(CONTENT_TYPE, "application/octet-stream")],
                serialized_cmd,
            )
                .into_response()
        }
        Err(e) => {
            info!("task_camera failed: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to produce command bytes".to_string(),
            )
                .into_response()
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/authorize", post(handle_post))
        .with_state(state)
}

pub async fn initialize_or_return(
    owner: &ClientCtx,
    _member: &ClientCtx,
    init_marker: &Path,
    team_id_path: &Path,
    // NEW ARG: path to persist member_id
    member_id_path: &Path,
    already_initialized: bool,
) -> Result<TeamId> {
    if already_initialized {
        info!("already initialized; skipping onboarding");
        let team_id = read_team_id(team_id_path).await?;
        let member_id = read_member_id(member_id_path).await?;
        info!(%team_id, "read team_id from file");
        info!("member id: {}", member_id);
        info!("owner id: {}", owner.id);
        return Ok(team_id);
    }

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

    // One way to make sure member receives the team info is to trigger a sync from member to owner.
    member_team.sync_now(member_addr.into(), None).await?;

    info!("onboarding complete");

    // Mark initialization complete.
    fs::write(init_marker, b"initialized").await?;
    fs::write(team_id_path, team_id.to_string()).await?;
    // NEW: persist member id
    fs::write(member_id_path, _member.id.to_string()).await?;
    info!("wrote init marker, team_id, and member_id files");

    Ok(team_id)
}
