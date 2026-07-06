mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use pretty_assertions::assert_eq;

fn create_task(temp: &TempDb, title: &str) -> anyhow::Result<String> {
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "default",
            "task",
            "create",
            title,
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    created["data"]["id"]
        .as_str()
        .map(str::to_owned)
        .context("expected task id")
}

#[test]
fn signal_record_creates_ledger_and_backlink_comment() -> anyhow::Result<()> {
    let temp = TempDb::new("signal_record_creates_ledger_and_backlink_comment")?;
    kanban(&temp.path, &["--board", "default", "init"])?.success()?;
    let task_id = create_task(&temp, "signal target")?;
    let input = temp.dir.join("signal.json");
    std::fs::write(
        &input,
        format!(
            r#"{{"kind":"agent_cli_failure","title":"Bad flag","summary":"comment add rejected body-file","severity":"medium","task_ref":"{task_id}","actor":"codex","agent_type":"executor","dedupe_key":"cli-body-file","source":"test","evidence":{{"stderr":"unexpected argument"}},"comment":{{"body":"Signal backlink body"}}}}"#
        ),
    )?;

    let recorded = kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "default",
            "signal",
            "record",
            "--input",
            input.to_str().context("utf8 path")?,
        ],
    )?
    .success_json()?;
    let signal = &recorded["data"]["signal"];
    assert!(signal["id"].as_str().unwrap_or("").starts_with("sig_"));
    assert_eq!(signal["status"], "open");
    assert_eq!(signal["kind"], "agent_cli_failure");
    assert_eq!(signal["observation"]["task_id"], task_id);
    assert_eq!(signal["observation"]["actor"], "codex");

    let backlink = &recorded["data"]["backlink_comment"];
    assert_eq!(backlink["kind"], "signal");
    assert_eq!(backlink["body"], "Signal backlink body");
    let metadata: serde_json::Value =
        serde_json::from_str(backlink["metadata_json"].as_str().unwrap())?;
    assert_eq!(metadata["type"], "signal_link");
    assert_eq!(metadata["signal_id"], signal["id"]);
    assert_eq!(metadata["signal_status"], "open");

    let comments = kanban(
        &temp.path,
        &["--json", "--board", "default", "comment", "list", &task_id],
    )?
    .success_json()?;
    assert_eq!(comments["data"][0]["kind"], "signal");

    let listed = kanban(
        &temp.path,
        &["--json", "--board", "default", "signal", "list"],
    )?
    .success_json()?;
    assert_eq!(listed["data"].as_array().unwrap().len(), 1);
    assert_eq!(listed["data"][0]["id"], signal["id"]);

    let shown = kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "default",
            "signal",
            "show",
            signal["id"].as_str().unwrap(),
        ],
    )?
    .success_json()?;
    assert_eq!(shown["data"]["id"], signal["id"]);
    Ok(())
}

#[test]
fn signal_export_import_round_trips_and_doctor_remains_clean() -> anyhow::Result<()> {
    let source = TempDb::new("signal_export_import_round_trips_source")?;
    kanban(&source.path, &["--board", "default", "init"])?.success()?;
    let task_id = create_task(&source, "exported signal target")?;
    let signal_id = record_signal_for_task(&source, "default", "exported", &task_id)?;

    let export_path = source.dir.join("signals.jsonl");
    let exported = kanban(
        &source.path,
        &[
            "--json",
            "export",
            "--out",
            export_path.to_str().context("utf8 path")?,
        ],
    )?
    .success_json()?;
    assert!(
        exported["data"]["records"]
            .as_u64()
            .context("expected record count")?
            >= 2
    );
    let content = std::fs::read_to_string(&export_path)?;
    assert!(content.contains(r#""type":"signal_observation""#));
    assert!(content.contains(r#""type":"signal""#));
    assert!(content.contains(&signal_id));

    let target = TempDb::new("signal_export_import_round_trips_target")?;
    kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().context("utf8 path")?,
            "--replace",
        ],
    )?
    .success_json()?;
    let listed = kanban(
        &target.path,
        &["--json", "--board", "default", "signal", "list"],
    )?
    .success_json()?;
    assert_eq!(listed["data"][0]["id"], signal_id);
    let comments = kanban(
        &target.path,
        &["--json", "--board", "default", "comment", "list", &task_id],
    )?
    .success_json()?;
    assert_eq!(comments["data"][0]["kind"], "signal");
    let metadata: serde_json::Value =
        serde_json::from_str(comments["data"][0]["metadata_json"].as_str().unwrap())?;
    assert_eq!(metadata["type"], "signal_link");
    assert_eq!(metadata["signal_id"], signal_id);
    let doctor = kanban(&target.path, &["--json", "doctor"])?.success_json()?;
    assert_eq!(doctor["data"]["ok"], true);
    assert_eq!(doctor["data"]["consistency_errors"], 0);
    Ok(())
}

#[test]
fn signal_show_and_lifecycle_reject_cross_board_ids() -> anyhow::Result<()> {
    let temp = TempDb::new("signal_show_and_lifecycle_reject_cross_board_ids")?;
    kanban(&temp.path, &["--board", "default", "init"])?.success()?;
    kanban(
        &temp.path,
        &["--json", "board", "create", "other", "--name", "Other"],
    )?
    .success_json()?;
    let default_signal = record_minimal_signal_on_board(&temp, "default", "default-signal")?;
    let other_signal = record_minimal_signal_on_board(&temp, "other", "other-signal")?;

    kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "other",
            "signal",
            "show",
            &default_signal,
        ],
    )?
    .json_failure_containing("signal not found")?;
    kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "other",
            "signal",
            "confirm",
            &default_signal,
            "--reason",
            "wrong board",
        ],
    )?
    .json_failure_containing("signal not found")?;
    kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "default",
            "signal",
            "supersede",
            &default_signal,
            "--by",
            &other_signal,
            "--reason",
            "cross board replacement",
        ],
    )?
    .json_failure_containing("signal not found")?;

    let shown = kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "default",
            "signal",
            "show",
            &default_signal,
        ],
    )?
    .success_json()?;
    assert_eq!(shown["data"]["id"], default_signal);
    assert_eq!(shown["data"]["status"], "open");
    Ok(())
}

#[test]
fn signal_lifecycle_confirm_resolve_and_supersede() -> anyhow::Result<()> {
    let temp = TempDb::new("signal_lifecycle_confirm_resolve_and_supersede")?;
    kanban(&temp.path, &["--board", "default", "init"])?.success()?;
    let first = record_minimal_signal(&temp, "one")?;
    let second = record_minimal_signal(&temp, "two")?;

    let confirmed = kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "default",
            "signal",
            "confirm",
            &first,
            "--reason",
            "reproduced",
        ],
    )?
    .success_json()?;
    assert_eq!(confirmed["data"][0]["status"], "confirmed");

    let resolved = kanban(
        &temp.path,
        &[
            "--json", "--board", "default", "signal", "resolve", &first, "--reason", "fixed",
        ],
    )?
    .success_json()?;
    assert_eq!(resolved["data"][0]["status"], "resolved");

    let superseded = kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "default",
            "signal",
            "supersede",
            &second,
            "--by",
            &first,
            "--reason",
            "duplicate",
        ],
    )?
    .success_json()?;
    assert_eq!(superseded["data"][0]["status"], "superseded");
    assert_eq!(superseded["data"][0]["superseded_by_signal_id"], first);
    Ok(())
}

fn record_minimal_signal(temp: &TempDb, title: &str) -> anyhow::Result<String> {
    record_minimal_signal_on_board(temp, "default", title)
}

fn record_minimal_signal_on_board(
    temp: &TempDb,
    board: &str,
    title: &str,
) -> anyhow::Result<String> {
    let input = temp.dir.join(format!("{board}-{title}.json"));
    std::fs::write(
        &input,
        format!(r#"{{"kind":"test","title":"{title}","summary":"summary","actor":"codex"}}"#),
    )?;
    let recorded = kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            board,
            "signal",
            "record",
            "--input",
            input.to_str().context("utf8 path")?,
        ],
    )?
    .success_json()?;
    Ok(recorded["data"]["signal"]["id"]
        .as_str()
        .unwrap()
        .to_owned())
}

fn record_signal_for_task(
    temp: &TempDb,
    board: &str,
    title: &str,
    task_id: &str,
) -> anyhow::Result<String> {
    let input = temp.dir.join(format!("{board}-{title}-task.json"));
    std::fs::write(
        &input,
        format!(
            r#"{{"kind":"test","title":"{title}","summary":"summary","actor":"codex","task_ref":"{task_id}","evidence":{{"source":"roundtrip"}},"comment":{{"body":"Signal backlink body"}}}}"#
        ),
    )?;
    let recorded = kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            board,
            "signal",
            "record",
            "--input",
            input.to_str().context("utf8 path")?,
        ],
    )?
    .success_json()?;
    Ok(recorded["data"]["signal"]["id"]
        .as_str()
        .unwrap()
        .to_owned())
}
