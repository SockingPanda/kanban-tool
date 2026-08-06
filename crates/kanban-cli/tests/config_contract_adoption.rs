mod config_adoption {
    use kanban_protocol::{ProjectConfigInput, WorkerProfileInput};
    use serde::{Serialize, de::DeserializeOwned};
    use serde_json::Value;

    fn assert_fixture_roundtrip<T>(raw: &str)
    where
        T: DeserializeOwned + Serialize,
    {
        let expected: Value = serde_json::from_str(raw).expect("config fixture JSON");
        let value: T = serde_json::from_value(expected.clone()).expect("config fixture DTO");
        assert_eq!(serde_json::to_value(value).expect("serialize config DTO"), expected);
    }

    #[test]
    fn project_config_input_fixture_is_produced_by_runtime_config_dto() {
        assert_fixture_roundtrip::<ProjectConfigInput>(include_str!(
            "../../../schemas/fixtures/config/project-input.v1.valid.json"
        ));
    }

    #[test]
    fn project_config_input_fixture_is_consumed_by_real_toml_decoder() {
        let source = "board = \"kanban-tool\"\ndb = \".kb/kb.db\"\n\n[vector]\nprovider = \"ollama\"\nendpoint = \"http://127.0.0.1:11434\"\nmodel = \"qwen3-embedding:0.6b\"\ndimensions = 1024\n";
        let value: ProjectConfigInput = toml::from_str(source).expect("project config TOML");
        assert_eq!(value.board.as_deref(), Some("kanban-tool"));
    }

    #[test]
    fn selected_worker_profile_input_fixture_is_produced_by_runtime_config_dto() {
        assert_fixture_roundtrip::<WorkerProfileInput>(include_str!(
            "../../../schemas/fixtures/config/selected-worker-profile-input.v1.valid.json"
        ));
    }

    #[test]
    fn selected_worker_profile_input_fixture_is_consumed_by_real_toml_decoder() {
        let source = "command = \"echo $KB_TASK_ID\"\nclaim_ttl_ms = 300000\nheartbeat_interval_ms = 30000\non_success = \"done\"\non_failure = \"blocked\"\nlog_dir = \".kb/logs\"\n";
        let value: WorkerProfileInput = toml::from_str(source).expect("worker profile TOML");
        assert_eq!(value.command.as_deref(), Some("echo $KB_TASK_ID"));
    }
}
