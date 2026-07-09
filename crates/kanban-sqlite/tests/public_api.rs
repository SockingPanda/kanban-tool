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

#[test]
fn adapter_api_facade_is_public_api() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/api_facade_contract.rs");
}

#[test]
fn adapter_api_facade_excludes_non_slice_symbols() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/api_facade_excludes_non_slice_symbol.rs");
}

#[test]
fn crate_root_legacy_reexports_are_removed() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/root_legacy_reexport_removed.rs");
}
