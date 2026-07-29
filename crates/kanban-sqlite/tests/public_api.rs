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
fn adapter_api_facade_excludes_provider_vector_helpers() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/api_facade_excludes_provider_vector_helper.rs");
}

#[test]
fn crate_root_legacy_reexports_are_removed() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/root_legacy_reexport_removed.rs");
}

#[test]
fn adapter_api_facade_excludes_provider_helpers_independently() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/api_root_excludes_provider/*.rs");
}

#[test]
fn provider_plane_is_public_api() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/api_provider_plane_contract.rs");
}

#[test]
fn adapter_api_facade_excludes_lifecycle_helpers_independently() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/api_root_excludes_lifecycle/*.rs");
}

#[test]
fn lifecycle_plane_is_public_api() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/api_lifecycle_plane_contract.rs");
}

#[test]
fn projection_v2_plane_is_public_api() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/api_projection_v2_contract.rs");
}

#[test]
fn projection_v2_provider_seam_is_excluded_from_api_root() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/api_projection_v2_provider_root_private.rs");
}

#[test]
fn adapter_api_facade_excludes_db_init_helpers_independently() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/api_root_excludes_db_init/*.rs");
}

#[test]
fn db_init_modules_remain_explicit_public_api() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/db_init_module_contract.rs");
}
