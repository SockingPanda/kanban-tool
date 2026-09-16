use std::collections::BTreeMap;

use kanban_protocol::rpc::{query, v1};
use prost::Message;

#[test]
fn each_typed_query_has_its_matching_complete_result() {
    let proto = include_str!("../../proto/kanban/v1/query.proto");
    let requests = proto
        .split("oneof query {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    let responses = proto
        .split("oneof result {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    let fields = |text: &str| -> BTreeMap<String, (String, u32)> {
        text.lines()
            .filter_map(|line| {
                let parts = line.split_whitespace().collect::<Vec<_>>();
                if parts.len() != 4 {
                    return None;
                }
                Some((
                    parts[1].to_owned(),
                    (
                        parts[0].to_owned(),
                        parts[3].trim_end_matches(';').parse().unwrap(),
                    ),
                ))
            })
            .collect()
    };
    let expected = fields(requests)
        .into_iter()
        .map(|(name, (message, number))| {
            let response = if name == "recent_events" {
                "ListEventsResponse".into()
            } else {
                format!("{}Response", message.strip_suffix("Request").unwrap())
            };
            (name, (response, number))
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(expected, fields(responses));
    assert!(expected.contains_key("get_task_details"));
    assert!(expected.contains_key("list_tasks_by_status"));
    assert!(expected.contains_key("get_run_log"));
}

#[test]
fn projection_cursor_and_paged_query_keep_exact_wire_values() {
    let original = v1::WatchQueriesRequest {
        protocol_version: query::PROTOCOL_VERSION,
        queries: vec![v1::QueryDefinition {
            client_query_id: "page-2".into(),
            projection_version: query::PROJECTION_VERSION,
            resume: Some(v1::QueryCursor {
                epoch: "runtime".into(),
                scope: "opaque-scope".into(),
                revision: u64::MAX,
            }),
            refresh: true,
            query: Some(v1::query_definition::Query::ListTasks(
                v1::ListTasksRequest {
                    board: Some("b_one".into()),
                    status: vec![
                        v1::DtoApiTaskStatus::Ready.into(),
                        v1::DtoApiTaskStatus::Todo.into(),
                    ],
                    include_archived: Some(false),
                    limit: Some(25),
                    offset: Some(50),
                    ..Default::default()
                },
            )),
        }],
    };
    let decoded = v1::WatchQueriesRequest::decode(original.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(
        decoded.queries[0].resume.as_ref().unwrap().revision,
        u64::MAX
    );
}

#[test]
fn maximum_payload_chunk_fits_the_encoded_frame_budget() {
    let frame = v1::QueryFrame {
        client_query_id: "q".repeat(128),
        body: Some(v1::query_frame::Body::Chunk(v1::QueryChunk {
            index: u32::MAX,
            data: vec![0; query::QUERY_CHUNK_BYTES],
        })),
    };
    assert!(frame.encoded_len() <= query::MAX_QUERY_FRAME_BYTES);
}
