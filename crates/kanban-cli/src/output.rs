use std::process::ExitCode;

use serde::Serialize;

use crate::error::{CliErrorBody, CliErrorEnvelope, CliFailure};

pub(crate) fn print_json<T: Serialize>(value: &T) {
    println!(
        "{}",
        serde_json::to_string(value).expect("CLI 响应必须可序列化")
    );
}

pub(crate) fn finish_failure(json: bool, error: &CliFailure) -> ExitCode {
    if json {
        print_json(&CliErrorEnvelope {
            error: CliErrorBody {
                code: error.code,
                message: error.message.clone(),
                exit_code: error.exit_code,
            },
        });
    } else {
        eprintln!("{}: {}", error.code, error.message);
    }
    ExitCode::from(error.exit_code)
}
