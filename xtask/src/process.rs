use std::process::ExitStatus;

pub(crate) fn status_description(status: ExitStatus) -> String {
    status
        .code()
        .map_or_else(|| "signal".to_owned(), |code| format!("exit={code}"))
}
