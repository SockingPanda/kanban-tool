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
        let value: T = serde_json::from_str(raw).expect("CLI maintenance DTO");
        assert_eq!(
            serde_json::to_value(value).expect("serialize CLI maintenance DTO"),
            expected
        );
    }

    macro_rules! adoption_fixture {
        ($test:ident, $ty:ty, $fixture:expr) => {
            #[test]
            fn $test() {
                assert_fixture_roundtrip::<$ty>($fixture);
            }
        };
    }

    adoption_fixture!(
        maintenance_backup_fixture_roundtrip,
        BackupResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-backup-output.v1.valid.json")
    );
    adoption_fixture!(
        maintenance_export_fixture_roundtrip,
        ExportResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-export-output.v1.valid.json")
    );
    adoption_fixture!(
        maintenance_import_fixture_roundtrip,
        ImportResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-import-output.v1.valid.json")
    );
    adoption_fixture!(
        maintenance_vacuum_fixture_roundtrip,
        VacuumResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-vacuum-output.v1.valid.json")
    );
    adoption_fixture!(
        maintenance_status_fixture_roundtrip,
        MaintenanceStatusResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-status-output.v1.valid.json")
    );
    adoption_fixture!(
        maintenance_run_fixture_roundtrip,
        MaintenanceRunResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-run-output.v1.valid.json")
    );
    adoption_fixture!(
        maintenance_rebuild_fixture_roundtrip,
        MaintenanceRebuildResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-rebuild-output.v1.valid.json")
    );
    adoption_fixture!(
        maintenance_cleanup_fixture_roundtrip,
        MaintenanceRunResponse,
        include_str!("../../../schemas/fixtures/cli/maintenance-cleanup-output.v1.valid.json")
    );
    adoption_fixture!(
        legacy_import_v30_fixture_roundtrip,
        LegacyImportResponse,
        include_str!("../../../schemas/fixtures/cli/import-v30-output.v1.valid.json")
    );
}
