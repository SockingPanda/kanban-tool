mod maintenance_adoption {
    use kanban_protocol::{
        BackupResponse, ExportResponse, ImportResponse, LegacyImportResponse,
        MaintenanceRebuildResponse, MaintenanceRunResponse, MaintenanceStatusResponse,
        VacuumResponse,
    };
    use serde::{Serialize, de::DeserializeOwned};
    use serde_json::Value;

    fn assert_fixture_roundtrip<T>(raw: &str)
    where
        T: DeserializeOwned + Serialize,
    {
        let expected: Value = serde_json::from_str(raw).expect("CLI maintenance fixture JSON");
        let value: T = serde_json::from_value(expected.clone()).expect("CLI maintenance DTO");
        assert_eq!(serde_json::to_value(value).expect("serialize CLI maintenance DTO"), expected);
    }

    macro_rules! adoption_pair {
        ($producer:ident, $consumer:ident, $ty:ty, $fixture:expr) => {
            #[test]
            fn $producer() {
                assert_fixture_roundtrip::<$ty>($fixture);
            }

            #[test]
            fn $consumer() {
                let value: $ty = serde_json::from_str($fixture).expect("CLI maintenance DTO");
                let encoded = serde_json::to_value(value).expect("serialize CLI maintenance DTO");
                assert!(encoded.is_object());
            }
        };
    }

    adoption_pair!(
        maintenance_backup_cli_producer,
        maintenance_backup_cli_consumer,
        BackupResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-backup-output.v1.valid.json")
    );
    adoption_pair!(
        maintenance_export_cli_producer,
        maintenance_export_cli_consumer,
        ExportResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-export-output.v1.valid.json")
    );
    adoption_pair!(
        maintenance_import_cli_producer,
        maintenance_import_cli_consumer,
        ImportResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-import-output.v1.valid.json")
    );
    adoption_pair!(
        maintenance_vacuum_cli_producer,
        maintenance_vacuum_cli_consumer,
        VacuumResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-vacuum-output.v1.valid.json")
    );
    adoption_pair!(
        maintenance_status_cli_producer,
        maintenance_status_cli_consumer,
        MaintenanceStatusResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-status-output.v1.valid.json")
    );
    adoption_pair!(
        maintenance_run_cli_producer,
        maintenance_run_cli_consumer,
        MaintenanceRunResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-run-output.v1.valid.json")
    );
    adoption_pair!(
        maintenance_rebuild_cli_producer,
        maintenance_rebuild_cli_consumer,
        MaintenanceRebuildResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-rebuild-output.v1.valid.json")
    );
    adoption_pair!(
        maintenance_cleanup_cli_producer,
        maintenance_cleanup_cli_consumer,
        MaintenanceRunResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-cleanup-output.v1.valid.json")
    );
    adoption_pair!(
        legacy_import_v30_cli_producer,
        legacy_import_v30_cli_consumer,
        LegacyImportResponse,
        include_str!("../../../schemas/fixtures/cli/import-v30-output.v1.valid.json")
    );
}
