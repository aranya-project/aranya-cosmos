//! Integration test for the COSMOS TaskCamera roundtrip.
//!
//! Tests that ephemeral bytes produced by `task_camera` on the owner
//! can be verified with `receive_cosmos_ctrl` on a member.

#![allow(
    clippy::disallowed_macros,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    rust_2018_idioms
)]

mod common;

use std::str::FromStr;

use anyhow::Result;
use aranya_daemon_api::Text;
use test_log::test;
use tracing::info;

use crate::common::DevicesCtx;

/// Tests the full task_camera -> receive_cosmos_ctrl roundtrip.
///
/// Owner issues a camera task command targeting membera.
/// Membera receives and verifies the ephemeral command bytes.
#[test(tokio::test(flavor = "multi_thread"))]
async fn test_cosmos_task_camera_roundtrip() -> Result<()> {
    let mut devices = DevicesCtx::new("test_cosmos_roundtrip").await?;

    let team_id = devices.create_and_add_team().await?;
    devices.add_all_device_roles(team_id).await?;

    let owner_team = devices.owner.client.team(team_id);
    let membera_team = devices.membera.client.team(team_id);

    // Owner issues a camera task targeting membera.
    info!("owner issuing task_camera");
    let ctrl = owner_team
        .task_camera(Text::from_str("photo_earth")?, devices.membera.id)
        .await
        .expect("task_camera should succeed for owner targeting member");

    assert!(!ctrl.is_empty(), "ctrl bytes should not be empty");

    // Membera receives and verifies the ephemeral command.
    info!("membera receiving cosmos ctrl");
    membera_team
        .receive_cosmos_ctrl(Text::from_str("photo_earth")?, ctrl)
        .await
        .expect("receive_cosmos_ctrl should succeed for the intended recipient");

    Ok(())
}

/// Tests that receive_cosmos_ctrl rejects a mismatched task name.
#[test(tokio::test(flavor = "multi_thread"))]
async fn test_cosmos_task_camera_wrong_name() -> Result<()> {
    let mut devices = DevicesCtx::new("test_cosmos_wrong_name").await?;

    let team_id = devices.create_and_add_team().await?;
    devices.add_all_device_roles(team_id).await?;

    let owner_team = devices.owner.client.team(team_id);
    let membera_team = devices.membera.client.team(team_id);

    let task_name = Text::from_str("photo_earth")?;
    let wrong_name = Text::from_str("photo_mars")?;

    // Owner issues a camera task.
    let ctrl = owner_team
        .task_camera(task_name, devices.membera.id)
        .await
        .expect("task_camera should succeed");

    // Membera tries to receive with wrong task name — should fail.
    let result = membera_team.receive_cosmos_ctrl(wrong_name, ctrl).await;
    assert!(
        result.is_err(),
        "receive_cosmos_ctrl should fail with wrong task name"
    );

    Ok(())
}
