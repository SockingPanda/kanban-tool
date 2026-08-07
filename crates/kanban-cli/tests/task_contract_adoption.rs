use kanban_protocol::{CliTaskBlockOutput, CliTaskDoneOutput};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

fn assert_fixture_roundtrip<T>(raw: &str)
where
    T: DeserializeOwned + Serialize,
{
    let expected: Value = serde_json::from_str(raw).expect("CLI task fixture JSON");
    let value: T = serde_json::from_value(expected.clone()).expect("CLI task DTO");
    assert_eq!(
        serde_json::to_value(value).expect("serialize CLI task DTO"),
        expected
    );
}

#[test]
fn task_done_output_contract() {
    assert_fixture_roundtrip::<CliTaskDoneOutput>(include_str!(
        "../../../schemas/fixtures/cli/task-done-output.v1.valid.json"
    ));
}

#[test]
fn task_block_output_contract() {
    assert_fixture_roundtrip::<CliTaskBlockOutput>(include_str!(
        "../../../schemas/fixtures/cli/task-block-output.v1.valid.json"
    ));
}
