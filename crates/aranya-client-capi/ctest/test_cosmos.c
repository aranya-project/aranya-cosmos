#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "aranya-client.h"
#include "utils.h"

/*
 * Helper: initialize a client connected to a daemon at the given UDS path.
 * On success, populates `client`, `device_id`, and optionally `pk`/`pk_len`.
 */
static AranyaError init_client(const char *uds_path,
                               AranyaClient *client,
                               AranyaDeviceId *device_id,
                               uint8_t **pk_out,
                               size_t *pk_len_out) {
    AranyaError err;

    AranyaClientConfigBuilder builder;
    err = aranya_client_config_builder_init(&builder);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    err = aranya_client_config_builder_set_daemon_uds_path(&builder, uds_path);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    AranyaClientConfig config;
    err = aranya_client_config_build(&builder, &config);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    err = aranya_client_init(client, &config);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    err = aranya_get_device_id(client, device_id);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    if (pk_out && pk_len_out) {
        size_t len = 1;
        uint8_t *pk = calloc(1, 1);
        if (!pk) return ARANYA_ERROR_OTHER;

        err = aranya_get_public_key_bundle(client, pk, &len);
        if (err == ARANYA_ERROR_BUFFER_TOO_SMALL) {
            pk = realloc(pk, len);
            if (!pk) return ARANYA_ERROR_OTHER;
            err = aranya_get_public_key_bundle(client, pk, &len);
        }
        if (err != ARANYA_ERROR_SUCCESS) {
            free(pk);
            return err;
        }
        *pk_out = pk;
        *pk_len_out = len;
    }

    return ARANYA_ERROR_SUCCESS;
}

/*
 * Helper: create team on owner, generate seed IKM, and have membera join.
 * Populates team_id and seed_ikm for the caller.
 */
static AranyaError create_team_and_add_member(
    AranyaClient *owner, AranyaClient *membera,
    const uint8_t *membera_pk, size_t membera_pk_len,
    AranyaTeamId *team_id, AranyaSeedIkm *seed_ikm) {
    AranyaError err;

    /* Generate seed IKM */
    err = aranya_rand(owner, seed_ikm->bytes, ARANYA_SEED_IKM_LEN);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    /* Owner creates team with QUIC sync */
    AranyaCreateTeamQuicSyncConfigBuilder qs_builder;
    err = aranya_create_team_quic_sync_config_builder_init(&qs_builder);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    err = aranya_create_team_quic_sync_config_raw_seed_ikm(&qs_builder, seed_ikm);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    AranyaCreateTeamQuicSyncConfig qs_config;
    err = aranya_create_team_quic_sync_config_build(&qs_builder, &qs_config);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    AranyaCreateTeamConfigBuilder team_builder;
    err = aranya_create_team_config_builder_init(&team_builder);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    err = aranya_create_team_config_builder_set_quic_syncer(&team_builder, &qs_config);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    AranyaCreateTeamConfig team_config;
    err = aranya_create_team_config_build(&team_builder, &team_config);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    err = aranya_create_team(owner, &team_config, team_id);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    /* Setup default roles */
    AranyaRole roles[8];
    size_t roles_len = 8;
    err = aranya_setup_default_roles(owner, team_id, roles, &roles_len);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    /* Find the member role */
    AranyaRoleId member_role_id;
    err = get_role_id_by_name(roles, roles_len, "member", &member_role_id);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    /* Query member role rank to derive device rank */
    AranyaObjectId role_object_id = {.id = member_role_id.id};
    int64_t role_rank = 0;
    err = aranya_query_rank(owner, team_id, &role_object_id, &role_rank);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    /* Add membera to team with member role */
    err = aranya_add_device_to_team(owner, team_id, membera_pk, membera_pk_len,
                                    &member_role_id, role_rank - 1);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    /* Membera joins the team */
    AranyaAddTeamQuicSyncConfigBuilder add_qs_builder;
    err = aranya_add_team_quic_sync_config_builder_init(&add_qs_builder);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    err = aranya_add_team_quic_sync_config_raw_seed_ikm(&add_qs_builder, seed_ikm);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    AranyaAddTeamQuicSyncConfig add_qs_config;
    err = aranya_add_team_quic_sync_config_build(&add_qs_builder, &add_qs_config);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    AranyaAddTeamConfigBuilder add_team_builder;
    err = aranya_add_team_config_builder_init(&add_team_builder);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    err = aranya_add_team_config_builder_set_id(&add_team_builder, team_id);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    err = aranya_add_team_config_builder_set_quic_syncer(&add_team_builder, &add_qs_config);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    AranyaAddTeamConfig add_team_config;
    err = aranya_add_team_config_build(&add_team_builder, &add_team_config);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    err = aranya_add_team(membera, &add_team_config);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    /* Sync membera with owner so it sees the team state */
    err = aranya_sync_now(membera, team_id, "127.0.0.1:40001", NULL);
    if (err != ARANYA_ERROR_SUCCESS) return err;

    return ARANYA_ERROR_SUCCESS;
}

/* Test: task_camera -> receive_cosmos_ctrl roundtrip */
static int test_cosmos_task_camera_roundtrip(const char *tmpdir) {
    printf("\n=== TEST: COSMOS task_camera roundtrip ===\n");

    AranyaError err;
    AranyaClient owner_client = {0};
    AranyaClient membera_client = {0};
    AranyaDeviceId owner_id, membera_id;
    uint8_t *membera_pk = NULL;
    size_t membera_pk_len = 0;
    uint8_t *ctrl = NULL;
    int result = EXIT_FAILURE;

    char owner_uds[512];
    char membera_uds[512];
    snprintf(owner_uds, sizeof(owner_uds), "%s/owner/uds.sock", tmpdir);
    snprintf(membera_uds, sizeof(membera_uds), "%s/membera/uds.sock", tmpdir);

    /* Initialize clients */
    CLIENT_EXPECT("Failed to init owner", "",
                  init_client(owner_uds, &owner_client, &owner_id, NULL, NULL));
    CLIENT_EXPECT("Failed to init membera", "",
                  init_client(membera_uds, &membera_client, &membera_id,
                              &membera_pk, &membera_pk_len));

    printf("Owner and membera clients initialized\n");

    /* Create team and add membera */
    AranyaTeamId team_id;
    AranyaSeedIkm seed_ikm;
    CLIENT_EXPECT("Failed to create team and add member", "",
                  create_team_and_add_member(&owner_client, &membera_client,
                                             membera_pk, membera_pk_len,
                                             &team_id, &seed_ikm));

    printf("Team created, membera added and synced\n");

    /* Owner issues task_camera targeting membera */
    printf("Owner issuing task_camera...\n");
    size_t ctrl_len = 1;
    ctrl = calloc(1, 1);
    if (!ctrl) {
        fprintf(stderr, "Failed to allocate ctrl buffer\n");
        goto exit;
    }

    err = aranya_task_camera(&owner_client, &team_id, "photo_earth",
                             &membera_id, ctrl, &ctrl_len);
    if (err == ARANYA_ERROR_BUFFER_TOO_SMALL) {
        ctrl = realloc(ctrl, ctrl_len);
        if (!ctrl) {
            fprintf(stderr, "Failed to realloc ctrl buffer\n");
            goto exit;
        }
        CLIENT_EXPECT("task_camera failed", "",
                      aranya_task_camera(&owner_client, &team_id, "photo_earth",
                                         &membera_id, ctrl, &ctrl_len));
    } else {
        CLIENT_EXPECT("task_camera failed", "", err);
    }

    if (ctrl_len == 0) {
        fprintf(stderr, "FAIL: ctrl bytes should not be empty\n");
        goto exit;
    }
    printf("task_camera returned %zu bytes\n", ctrl_len);

    /* Membera receives and verifies the control message */
    printf("Membera receiving cosmos ctrl...\n");
    CLIENT_EXPECT("receive_cosmos_ctrl failed", "",
                  aranya_receive_cosmos_ctrl(&membera_client, &team_id,
                                             "photo_earth", ctrl, ctrl_len));

    printf("PASS: COSMOS task_camera roundtrip succeeded\n");
    result = EXIT_SUCCESS;

exit:
    free(ctrl);
    free(membera_pk);
    aranya_client_cleanup(&membera_client);
    aranya_client_cleanup(&owner_client);
    return result;
}

/* Test: receive_cosmos_ctrl rejects a mismatched task name */
static int test_cosmos_task_camera_wrong_name(const char *tmpdir) {
    printf("\n=== TEST: COSMOS task_camera wrong name ===\n");

    AranyaError err;
    AranyaClient owner_client = {0};
    AranyaClient membera_client = {0};
    AranyaDeviceId owner_id, membera_id;
    uint8_t *membera_pk = NULL;
    size_t membera_pk_len = 0;
    uint8_t *ctrl = NULL;
    int result = EXIT_FAILURE;

    char owner_uds[512];
    char membera_uds[512];
    snprintf(owner_uds, sizeof(owner_uds), "%s/owner/uds.sock", tmpdir);
    snprintf(membera_uds, sizeof(membera_uds), "%s/membera/uds.sock", tmpdir);

    /* Initialize clients */
    CLIENT_EXPECT("Failed to init owner", "",
                  init_client(owner_uds, &owner_client, &owner_id, NULL, NULL));
    CLIENT_EXPECT("Failed to init membera", "",
                  init_client(membera_uds, &membera_client, &membera_id,
                              &membera_pk, &membera_pk_len));

    /* Create team and add membera */
    AranyaTeamId team_id;
    AranyaSeedIkm seed_ikm;
    CLIENT_EXPECT("Failed to create team and add member", "",
                  create_team_and_add_member(&owner_client, &membera_client,
                                             membera_pk, membera_pk_len,
                                             &team_id, &seed_ikm));

    /* Owner issues task_camera */
    size_t ctrl_len = 1;
    ctrl = calloc(1, 1);
    if (!ctrl) {
        fprintf(stderr, "Failed to allocate ctrl buffer\n");
        goto exit;
    }

    err = aranya_task_camera(&owner_client, &team_id, "photo_earth",
                             &membera_id, ctrl, &ctrl_len);
    if (err == ARANYA_ERROR_BUFFER_TOO_SMALL) {
        ctrl = realloc(ctrl, ctrl_len);
        if (!ctrl) {
            fprintf(stderr, "Failed to realloc ctrl buffer\n");
            goto exit;
        }
        CLIENT_EXPECT("task_camera failed", "",
                      aranya_task_camera(&owner_client, &team_id, "photo_earth",
                                         &membera_id, ctrl, &ctrl_len));
    } else {
        CLIENT_EXPECT("task_camera failed", "", err);
    }

    /* Membera tries to receive with wrong task name -- should fail */
    printf("Membera receiving with wrong name...\n");
    err = aranya_receive_cosmos_ctrl(&membera_client, &team_id,
                                     "photo_mars", ctrl, ctrl_len);
    if (err == ARANYA_ERROR_SUCCESS) {
        fprintf(stderr,
                "FAIL: receive_cosmos_ctrl should fail with wrong task name\n");
        goto exit;
    }

    printf("PASS: receive_cosmos_ctrl correctly rejected wrong task name "
           "(error: %s)\n",
           aranya_error_to_str(err));
    result = EXIT_SUCCESS;

exit:
    free(ctrl);
    free(membera_pk);
    aranya_client_cleanup(&membera_client);
    aranya_client_cleanup(&owner_client);
    return result;
}

int main(int argc, const char *argv[]) {
    if (argc != 2) {
        fprintf(stderr, "usage: %s <tmpdir>\n", argv[0]);
        return EXIT_FAILURE;
    }

    const char *tmpdir = argv[1];
    int failures = 0;

    printf("Running COSMOS tests\n");

    if (test_cosmos_task_camera_roundtrip(tmpdir) != EXIT_SUCCESS) {
        fprintf(stderr, "FAILED: test_cosmos_task_camera_roundtrip\n");
        failures++;
    }

    if (test_cosmos_task_camera_wrong_name(tmpdir) != EXIT_SUCCESS) {
        fprintf(stderr, "FAILED: test_cosmos_task_camera_wrong_name\n");
        failures++;
    }

    if (failures > 0) {
        fprintf(stderr, "\n%d test(s) FAILED\n", failures);
        return EXIT_FAILURE;
    }

    printf("\nAll COSMOS tests PASSED\n");
    return EXIT_SUCCESS;
}
