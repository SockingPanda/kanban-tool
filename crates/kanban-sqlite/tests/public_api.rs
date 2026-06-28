#[test]
fn raw_trusted_evidence_helper_is_not_public_api() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/trusted_evidence_private.rs");
}

#[test]
fn structure_plan_writer_is_not_public_api() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/structure_plan_writer_private.rs");
}
