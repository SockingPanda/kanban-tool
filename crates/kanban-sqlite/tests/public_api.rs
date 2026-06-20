// The trybuild fixture recompiles kanban-sqlite. With vector-lancedb enabled,
// that pulls in the heavy Lance/DataFusion stack and can exceed nextest's
// per-test timeout; the default feature surface still covers this API guard.
#[cfg(not(feature = "vector-lancedb"))]
#[test]
fn raw_trusted_evidence_helper_is_not_public_api() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/trusted_evidence_private.rs");
}
