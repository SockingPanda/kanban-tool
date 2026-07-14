use std::collections::{BTreeMap, BTreeSet};

use crate::common::*;
use axum::body::to_bytes;
use kanban_contract::{
    ApiHeaderProfile, EndpointObligation, HttpMethod, WireParameterCardinality,
    api_header_contract_specs,
};

fn probe_path(path: &str) -> String {
    path.split('/')
        .map(|segment| match segment {
            ":board" => "default",
            segment if segment.starts_with(':') => "header-contract-probe",
            segment => segment,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn method_name(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

fn fixture_value(profile: ApiHeaderProfile) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    object.insert("Accept-Language".to_owned(), json!("zh-CN"));
    if matches!(
        profile,
        ApiHeaderProfile::LocaleActor
            | ApiHeaderProfile::LocaleActorJson
            | ApiHeaderProfile::LocaleActorOptionalJson
    ) {
        object.insert("X-KB-Actor".to_owned(), json!("schema-agent"));
    }
    if matches!(
        profile,
        ApiHeaderProfile::LocaleJson | ApiHeaderProfile::LocaleActorJson
    ) {
        object.insert("Content-Type".to_owned(), json!("application/json"));
    }
    serde_json::Value::Object(object)
}

#[test]
fn exact_header_profile_fixtures_are_produced() -> anyhow::Result<()> {
    let specs = api_header_contract_specs();
    assert_eq!(specs.len(), 83);
    let profiles = specs
        .iter()
        .map(|spec| spec.profile)
        .collect::<BTreeSet<_>>();
    assert_eq!(profiles.len(), 5);

    for profile in profiles {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../schemas/fixtures/api/headers/{}.v1.valid.json",
            profile.fixture_stem()
        ));
        let fixture: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(fixture_path)?)?;
        assert_eq!(fixture_value(profile), fixture, "{profile:?}");
    }
    Ok(())
}

#[tokio::test]
async fn exact_header_profiles_are_consumed_by_real_router() -> anyhow::Result<()> {
    let specs = api_header_contract_specs();
    assert_eq!(specs.len(), 83);
    assert!(specs.iter().all(|spec| {
        spec.endpoint.obligations.headers == EndpointObligation::Contract(spec.contract_id)
            && spec
                .profile
                .parameters()
                .iter()
                .any(|parameter| parameter.name == "Accept-Language")
    }));

    let profile_counts = specs.iter().fold(BTreeMap::new(), |mut counts, spec| {
        *counts.entry(spec.profile).or_insert(0_usize) += 1;
        counts
    });
    assert_eq!(profile_counts.values().sum::<usize>(), 83);
    assert!(profile_counts.values().all(|count| *count > 0));

    let route_probe = TestApp::new()?;
    for spec in &specs {
        let content_type = spec
            .profile
            .parameters()
            .iter()
            .find(|parameter| parameter.name == "Content-Type");
        if content_type.and_then(|parameter| parameter.cardinality)
            == Some(WireParameterCardinality::RequiredOne)
        {
            let missing = route_probe
                .router()
                .oneshot(
                    Request::builder()
                        .method(method_name(spec.endpoint.method))
                        .uri(probe_path(spec.endpoint.path))
                        .header("Accept-Language", "en")
                        .body(Body::from("{}"))?,
                )
                .await?;
            assert_eq!(
                missing.status(),
                StatusCode::BAD_REQUEST,
                "{}",
                spec.contract_id
            );
            let missing: serde_json::Value =
                serde_json::from_slice(&to_bytes(missing.into_body(), usize::MAX).await?)?;
            assert_eq!(
                missing["error"]["code"], "invalid_input",
                "{}",
                spec.contract_id
            );
            assert!(
                missing["error"]["message"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("Content-Type"),
                "{}: {missing}",
                spec.contract_id
            );
        }

        let send_content_type = content_type.and_then(|parameter| parameter.cardinality)
            == Some(WireParameterCardinality::RequiredOne);
        let mut request = Request::builder()
            .method(method_name(spec.endpoint.method))
            .uri(probe_path(spec.endpoint.path))
            .header("Accept-Language", "zh-CN");
        if send_content_type {
            request = request.header("Content-Type", "application/json");
        }
        let response = route_probe
            .router()
            .oneshot(request.body(if send_content_type {
                Body::from("{}")
            } else {
                Body::empty()
            })?)
            .await?;
        let status = response.status();
        assert_eq!(
            response
                .headers()
                .get("Content-Type")
                .and_then(|value| value.to_str().ok()),
            Some("application/json"),
            "{} {} did not reach its JSON handler (status={})",
            method_name(spec.endpoint.method),
            spec.endpoint.path,
            status
        );
        assert_ne!(
            status,
            StatusCode::METHOD_NOT_ALLOWED,
            "{}",
            spec.contract_id
        );
        let response: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
        assert!(
            response.get("data").is_some() || response.get("error").is_some(),
            "{} did not produce an API envelope: {response}",
            spec.contract_id
        );
    }

    let health = TestApp::new()?;
    let db_path = health.db_path().to_path_buf();
    std::fs::remove_file(&db_path)?;
    let (_, zh) = get_json_with_accept_language(health.router(), "/health", "zh-CN").await?;
    let (_, en) = get_json_with_accept_language(health.router(), "/health", "en").await?;
    assert_ne!(zh["error"]["message"], en["error"]["message"]);
    assert!(
        zh["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("数据库")
    );

    let test = TestApp::with_actor("default-actor")?;
    let app = test.router();
    let missing_content_type = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/boards")
                .body(Body::from(
                    json!({
                        "slug": "missing-content-type",
                        "name": "Missing content type",
                        "description": null,
                        "actor": null
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(missing_content_type.status(), StatusCode::BAD_REQUEST);

    let (status, _) = request_json(
        app.clone(),
        "POST",
        "/api/v1/boards",
        Some(json!({
            "slug": "body-actor",
            "name": "Body actor",
            "description": null,
            "actor": "body-actor"
        })),
        Some("header-actor"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    let events = kanban_sqlite::api::list_events(test.db_path(), "body-actor", None)?;
    assert_eq!(
        events.last().and_then(|event| event.actor.as_deref()),
        Some("body-actor")
    );

    let (status, _) = request_json(
        app.clone(),
        "POST",
        "/api/v1/boards",
        Some(json!({
            "slug": "header-actor",
            "name": "Header actor",
            "description": null,
            "actor": null
        })),
        Some("header-actor"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    let events = kanban_sqlite::api::list_events(test.db_path(), "header-actor", None)?;
    assert_eq!(
        events.last().and_then(|event| event.actor.as_deref()),
        Some("header-actor")
    );

    let optional_body = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/boards/header-actor/archive")
                .header("X-KB-Actor", "archive-actor")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(optional_body.status(), StatusCode::OK);

    for path in [
        "/api/v1/maintenance/doctor",
        "/api/v1/maintenance/checkpoint",
    ] {
        let no_body = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(no_body.status(), StatusCode::OK, "{path}");
    }

    let stream = kanban_contract::endpoint_descriptor("sse.stream-events").expect("SSE descriptor");
    assert!(matches!(
        stream.obligations.headers,
        EndpointObligation::Excluded { .. }
    ));
    Ok(())
}
