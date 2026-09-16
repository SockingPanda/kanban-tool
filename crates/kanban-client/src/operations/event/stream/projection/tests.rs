use super::*;

fn response(count: usize) -> ListEventsResponse {
    let mut page: ListEventsResponse = serde_json::from_str(include_str!(
        "../../../../../../../schemas/fixtures/api/list-events-response.v1.valid.json"
    ))
    .unwrap();
    let event = page.data.pop().unwrap();
    page.data = (0..count)
        .map(|index| {
            let mut event = event.clone();
            event.id = 42 + index as i64;
            event.event_id = format!("e_{index}");
            event
        })
        .collect();
    page.meta.next_after = page.data.last().map_or(0, |event| event.id);
    page
}

fn window() -> ListEventsQuery {
    ListEventsQuery {
        board: "fixture-board".into(),
        task_id: Some("t_fixture".into()),
        after: 0,
        limit: 1000,
    }
}

fn bytes(page: ListEventsResponse) -> Vec<u8> {
    v1::QueryResult {
        result: Some(v1::query_result::Result::ListEvents(
            page.try_into().unwrap(),
        )),
    }
    .encode_to_vec()
}

fn cursor(revision: u64) -> v1::QueryCursor {
    v1::QueryCursor {
        epoch: "epoch".into(),
        scope: "event-window".into(),
        revision,
    }
}

fn snapshot(data: &[u8], revision: u64) -> v1::QueryBegin {
    v1::QueryBegin {
        cursor: Some(cursor(revision)),
        base: None,
        snapshot: true,
        result_size: data.len() as u64,
        result_sha256: Sha256::digest(data).to_vec(),
        offset: 0,
        delete_length: 0,
        patch_size: data.len() as u64,
        chunk_count: data.len().div_ceil(QUERY_CHUNK_BYTES) as u32,
    }
}

fn chunks(target: &mut Projection, patch: &[u8]) {
    for (index, data) in patch.chunks(QUERY_CHUNK_BYTES).enumerate() {
        target
            .chunk(v1::QueryChunk {
                index: index as u32,
                data: data.to_vec(),
            })
            .unwrap();
    }
}

fn commit(target: &mut Projection, data: &[u8], revision: u64) -> ListEventsResponse {
    target.begin(snapshot(data, revision)).unwrap();
    chunks(target, data);
    target
        .end(
            v1::QueryEnd {
                cursor: Some(cursor(revision)),
            },
            &window(),
        )
        .unwrap()
}

#[test]
fn interrupted_multichunk_snapshot_keeps_last_complete_cursor() {
    let mut projection = Projection::default();
    let original = bytes(response(1));
    commit(&mut projection, &original, 1);
    let large = bytes(response(900));
    assert!(large.len() > QUERY_CHUNK_BYTES);
    projection.begin(snapshot(&large, 2)).unwrap();
    projection
        .chunk(v1::QueryChunk {
            index: 0,
            data: large[..QUERY_CHUNK_BYTES].to_vec(),
        })
        .unwrap();
    assert_eq!(projection.cursor(), Some(&cursor(1)));
    assert_eq!(projection.bytes, original);
    assert!(
        projection
            .end(
                v1::QueryEnd {
                    cursor: Some(cursor(2))
                },
                &window()
            )
            .is_err()
    );
    assert_eq!(projection.cursor(), Some(&cursor(1)));
    assert_eq!(commit(&mut projection, &large, 2).data.len(), 900);
}

#[test]
fn exact_delta_commits_complete_projection_and_rejects_wrong_base() {
    let mut projection = Projection::default();
    let original = bytes(response(1));
    commit(&mut projection, &original, 1);
    let mut next = response(1);
    next.data[0].actor = Some("完整 actor 修改".into());
    let result = bytes(next);
    let prefix = original
        .iter()
        .zip(&result)
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = original[prefix..]
        .iter()
        .rev()
        .zip(result[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let patch = &result[prefix..result.len() - suffix];
    let mut begin = snapshot(&result, 2);
    begin.snapshot = false;
    begin.base = Some(cursor(1));
    begin.offset = prefix as u64;
    begin.delete_length = (original.len() - prefix - suffix) as u64;
    begin.patch_size = patch.len() as u64;
    begin.chunk_count = patch.len().div_ceil(QUERY_CHUNK_BYTES) as u32;
    let mut wrong = begin.clone();
    wrong.base.as_mut().unwrap().revision = 0;
    assert!(projection.begin(wrong).is_err());
    projection.begin(begin).unwrap();
    chunks(&mut projection, patch);
    let page = projection
        .end(
            v1::QueryEnd {
                cursor: Some(cursor(2)),
            },
            &window(),
        )
        .unwrap();
    assert_eq!(page.data[0].actor.as_deref(), Some("完整 actor 修改"));
    assert_eq!(projection.bytes, result);
}

#[test]
fn malformed_budget_order_hash_and_scope_never_publish() {
    let mut projection = Projection::default();
    let original = bytes(response(1));
    commit(&mut projection, &original, 1);
    let mut enormous = snapshot(&original, 2);
    enormous.patch_size = u64::MAX;
    assert!(projection.begin(enormous).is_err());
    let mut bad_hash = snapshot(&original, 2);
    bad_hash.result_sha256[0] ^= 1;
    projection.begin(bad_hash).unwrap();
    assert!(
        projection
            .chunk(v1::QueryChunk {
                index: 1,
                data: original.clone()
            })
            .is_err()
    );
    chunks(&mut projection, &original);
    assert!(
        projection
            .end(
                v1::QueryEnd {
                    cursor: Some(cursor(2))
                },
                &window()
            )
            .is_err()
    );
    let mut wrong_board = response(1);
    wrong_board.data[0].board_id = "other-board".into();
    let wrong_board = bytes(wrong_board);
    projection.begin(snapshot(&wrong_board, 2)).unwrap();
    chunks(&mut projection, &wrong_board);
    assert!(
        projection
            .end(
                v1::QueryEnd {
                    cursor: Some(cursor(2))
                },
                &window()
            )
            .is_err()
    );
    assert_eq!(projection.bytes, original);
    assert_eq!(projection.cursor(), Some(&cursor(1)));
}

#[test]
fn valid_hash_does_not_hide_wrong_result_type_or_cursor() {
    let mut projection = Projection::default();
    let wrong = v1::QueryResult {
        result: Some(v1::query_result::Result::GetHealth(Default::default())),
    }
    .encode_to_vec();
    projection.begin(snapshot(&wrong, 1)).unwrap();
    chunks(&mut projection, &wrong);
    assert!(
        projection
            .end(
                v1::QueryEnd {
                    cursor: Some(cursor(1))
                },
                &window()
            )
            .is_err()
    );
    let data = bytes(response(1));
    projection.begin(snapshot(&data, 1)).unwrap();
    chunks(&mut projection, &data);
    assert!(
        projection
            .end(
                v1::QueryEnd {
                    cursor: Some(cursor(2))
                },
                &window()
            )
            .is_err()
    );
    assert!(projection.cursor().is_none());
}
