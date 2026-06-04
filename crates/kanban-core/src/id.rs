use ulid::Ulid;

pub fn new_typed_id(prefix: &str) -> String {
    format!("{prefix}_{}", Ulid::new())
}

pub fn new_board_id() -> String {
    new_typed_id("b")
}

pub fn new_task_id() -> String {
    new_typed_id("t")
}

pub fn new_run_id() -> String {
    new_typed_id("r")
}

pub fn new_event_id() -> String {
    new_typed_id("e")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_ids_have_expected_prefix() {
        assert!(new_board_id().starts_with("b_"));
        assert!(new_task_id().starts_with("t_"));
        assert!(new_run_id().starts_with("r_"));
        assert!(new_event_id().starts_with("e_"));
    }
}
