use quote::ToTokens;
use syn::visit::Visit;

fn imported_exactly(file: &syn::File, name: &str) -> bool {
    let matching = file.items.iter().filter(|item| match item {
        syn::Item::Use(item) => {
            let tokens = item.to_token_stream().to_string();
            tokens.starts_with("use kanban_contract")
                && !tokens.contains(" as ")
                && tokens
                    .split(|character: char| !character.is_alphanumeric() && character != '_')
                    .any(|word| word == name)
        }
        _ => false,
    });
    matching.count() == 1
}

#[derive(Default)]
struct Audit {
    explicit_returns: usize,
    snapshot_calls: usize,
    adapter_calls: usize,
    serializations: usize,
    forbidden: Vec<String>,
}

impl<'ast> Visit<'ast> for Audit {
    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        if matches!(
            pattern.ident.to_string().as_str(),
            "StreamEventsQuery" | "StreamEventData"
        ) {
            self.forbidden.push(format!("shadow {}", pattern.ident));
        }
        syn::visit::visit_pat_ident(self, pattern);
    }

    fn visit_expr_return(&mut self, node: &'ast syn::ExprReturn) {
        self.explicit_returns += 1;
        syn::visit::visit_expr_return(self, node);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*call.func {
            let name = path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string());
            match name.as_deref() {
                Some("events_snapshot") => self.snapshot_calls += 1,
                Some("stream_event_data") => self.adapter_calls += 1,
                Some("to_string") => self.serializations += 1,
                _ => {}
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        if matches!(
            path.segments
                .last()
                .map(|segment| segment.ident.to_string())
                .as_deref(),
            Some("EventDto" | "EventsQuery" | "Value")
        ) {
            self.forbidden.push(path.to_token_stream().to_string());
        }
        syn::visit::visit_path(self, path);
    }
}

fn validate(events_source: &str, shared_source: &str) -> Vec<String> {
    let events = match syn::parse_file(events_source) {
        Ok(file) => file,
        Err(error) => return vec![format!("events parse: {error}")],
    };
    let shared = match syn::parse_file(shared_source) {
        Ok(file) => file,
        Err(error) => return vec![format!("shared parse: {error}")],
    };
    let mut violations = Vec::new();
    for name in ["StreamEventsQuery", "StreamEventData"] {
        if !imported_exactly(&events, name) {
            violations.push(format!("canonical import {name}"));
        }
    }
    for item in &events.items {
        if matches!(item, syn::Item::Type(item_type) if item_type.ident == "StreamEventsQuery" || item_type.ident == "StreamEventData")
        {
            violations.push("local contract type shadow".into());
        }
    }
    let handlers = events
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == "stream_events" => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    if handlers.len() != 1 {
        violations.push(format!("stream_events count {}", handlers.len()));
        return violations;
    }
    let handler = handlers[0];
    let signature = handler.sig.to_token_stream().to_string();
    if !signature.contains("Query < StreamEventsQuery >")
        || !signature.contains("Sse < impl futures_util :: Stream")
    {
        violations.push("typed signature".into());
    }
    let mut audit = Audit::default();
    audit.visit_item_fn(handler);
    if audit.explicit_returns != 0 || audit.snapshot_calls != 1 || audit.serializations != 1 {
        violations.push(format!(
            "handler flow return={} snapshot={} serialize={}",
            audit.explicit_returns, audit.snapshot_calls, audit.serializations
        ));
    }
    if !audit.forbidden.is_empty() {
        violations.push(format!("handler forbidden {:?}", audit.forbidden));
    }
    let tail = handler
        .block
        .stmts
        .last()
        .map(ToTokens::to_token_stream)
        .map(|t| t.to_string());
    if tail.as_deref() != Some("Ok (Sse :: new (stream :: iter (frames)))") {
        violations.push(format!("implicit tail {tail:?}"));
    }

    let adapters = shared
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == "stream_event_data" => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    if adapters.len() != 1 {
        violations.push(format!("stream_event_data count {}", adapters.len()));
        return violations;
    }
    let adapter = adapters[0];
    let signature = adapter.sig.to_token_stream().to_string();
    if !signature.contains("StreamEventData") {
        violations.push("adapter return type".into());
    }
    let mut adapter_audit = Audit::default();
    adapter_audit.visit_item_fn(adapter);
    if adapter_audit.explicit_returns != 0 || !adapter_audit.forbidden.is_empty() {
        violations.push("adapter escape".into());
    }
    let constructors = adapter
        .block
        .to_token_stream()
        .to_string()
        .matches("StreamEventData {")
        .count();
    if constructors != 1 {
        violations.push(format!("adapter constructors {constructors}"));
    }
    let snapshots = shared
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == "events_snapshot" => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    if snapshots.len() != 1 {
        violations.push(format!("events_snapshot count {}", snapshots.len()));
        return violations;
    }
    let mut snapshot_audit = Audit::default();
    snapshot_audit.visit_item_fn(snapshots[0]);
    if snapshot_audit.adapter_calls != 1
        || snapshot_audit.explicit_returns != 0
        || !snapshot_audit.forbidden.is_empty()
    {
        violations.push(format!(
            "snapshot adapter={} return={} forbidden={:?}",
            snapshot_audit.adapter_calls, snapshot_audit.explicit_returns, snapshot_audit.forbidden
        ));
    }
    violations
}

#[test]
fn sse_handler_has_exact_contract_ownership_and_hostile_mutations_fail_closed() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let events =
        std::fs::read_to_string(root.join("crates/kanban-server/src/handlers/events.rs")).unwrap();
    let shared =
        std::fs::read_to_string(root.join("crates/kanban-server/src/handlers/shared.rs")).unwrap();
    let violations = validate(&events, &shared);
    assert!(violations.is_empty(), "{violations:#?}");

    let mutations = [
        (
            events.replace("StreamEventsQuery", "PrivateQuery"),
            shared.clone(),
        ),
        (
            events.replace("StreamEventsQuery", "serde_json::Value"),
            shared.clone(),
        ),
        (
            events.replace("StreamEventData", "EventDto"),
            shared.clone(),
        ),
        (
            events.replace("events_snapshot(", "private_snapshot("),
            shared.clone(),
        ),
        (
            events.replace("serde_json::to_string", "private_serialize"),
            shared.clone(),
        ),
        (
            events.replace(
                "Ok(Sse::new(stream::iter(frames)))",
                "return Ok(Sse::new(stream::iter(frames)));",
            ),
            shared.clone(),
        ),
        (
            events.replace(
                "StreamEventData, StreamEventsQuery,",
                "StreamEventData, StreamEventsQuery as PrivateQuery,",
            ),
            shared.clone(),
        ),
        (
            format!("type StreamEventsQuery = serde_json::Value;\n{events}"),
            shared.clone(),
        ),
        (
            events.clone(),
            shared.replace("fn stream_event_data", "fn private_event_data"),
        ),
        (
            events,
            shared.replace(
                "kanban_contract::StreamEventData {",
                "kanban_contract::EventDto {",
            ),
        ),
    ];
    for (index, (events, shared)) in mutations.into_iter().enumerate() {
        syn::parse_file(&events).expect("events mutation must remain valid Rust");
        syn::parse_file(&shared).expect("shared mutation must remain valid Rust");
        assert!(
            !validate(&events, &shared).is_empty(),
            "mutation {index} escaped"
        );
    }
}
