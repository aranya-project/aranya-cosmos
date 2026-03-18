//! Integration tests for the MAVLink TaskDrone and MapSysId roundtrip.
//!
//! Tests that:
//! - `map_sys_id` maps a MAVLINK system ID to a device ID
//! - `task_drone` produces ephemeral command bytes
//! - `receive_mavlink_ctrl` verifies command bytes on the receiving device

#![allow(
    clippy::disallowed_macros,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    rust_2018_idioms
)]

mod common;

use anyhow::Result;
use aranya_daemon_api::MavData;
use test_log::test;
use tracing::info;

use crate::common::{sleep, DevicesCtx, SLEEP_INTERVAL};

/// Tests the full task_drone -> receive_mavlink_ctrl roundtrip.
///
/// Owner maps system IDs, issues a drone task command, and membera
/// receives and verifies the ephemeral command bytes.
#[test(tokio::test(flavor = "multi_thread"))]
async fn test_mavlink_task_drone_roundtrip() -> Result<()> {
    let mut devices = DevicesCtx::new("test_mavlink_roundtrip").await?;

    let team_id = devices.create_and_add_team().await?;
    let roles = devices.setup_default_roles(team_id).await?;
    devices.add_all_device_roles(team_id, &roles).await?;

    let owner_team = devices.owner.client.team(team_id);
    let membera_team = devices.membera.client.team(team_id);

    // Owner maps its own system ID (255 = ground station).
    info!("owner mapping system ID 255 to itself");
    owner_team
        .map_sys_id(255, devices.owner.id)
        .await
        .expect("map_sys_id should succeed for owner mapping itself");

    // Owner maps membera to system ID 1.
    info!("owner mapping system ID 1 to membera");
    owner_team
        .map_sys_id(1, devices.membera.id)
        .await
        .expect("map_sys_id should succeed for owner mapping member");

    // Sync to propagate the persistent MapSysId commands to all devices.
    let owner_addr = devices.owner.aranya_local_addr().await?;
    membera_team.sync_now(owner_addr, None).await?;
    sleep(SLEEP_INTERVAL).await;

    // Owner issues a drone task command with MavData identifying sender/target.
    let mavdata = MavData {
        sender_sys_id: 255, // owner's system ID
        target_sys_id: 1,   // membera's system ID
        task_id: 42,
    };

    info!("owner issuing task_drone");
    let ctrl = owner_team
        .task_drone(mavdata)
        .await
        .expect("task_drone should succeed for owner");

    assert!(!ctrl.is_empty(), "ctrl bytes should not be empty");

    // Membera receives and verifies the ephemeral command with the same MavData.
    info!("membera receiving mavlink ctrl");
    let mavdata = MavData {
        sender_sys_id: 255,
        target_sys_id: 1,
        task_id: 42,
    };
    membera_team
        .receive_mavlink_ctrl(mavdata, ctrl)
        .await
        .expect("receive_mavlink_ctrl should succeed for the intended recipient");

    Ok(())
}

/// Tests that map_sys_id rejects duplicate system ID mappings.
#[test(tokio::test(flavor = "multi_thread"))]
async fn test_mavlink_map_sys_id_duplicate_rejected() -> Result<()> {
    let mut devices = DevicesCtx::new("test_mavlink_dup_sysid").await?;

    let team_id = devices.create_and_add_team().await?;
    let roles = devices.setup_default_roles(team_id).await?;
    devices.add_all_device_roles(team_id, &roles).await?;

    let owner_team = devices.owner.client.team(team_id);

    // First mapping should succeed.
    info!("owner mapping system ID 10 to membera");
    owner_team
        .map_sys_id(10, devices.membera.id)
        .await
        .expect("first map_sys_id should succeed");

    // Same system ID again should fail (policy checks `!exists SystemId[sys_id: ...]`).
    info!("owner attempting duplicate system ID mapping");
    let result = owner_team.map_sys_id(10, devices.memberb.id).await;
    assert!(
        result.is_err(),
        "map_sys_id should reject duplicate system ID"
    );

    Ok(())
}
