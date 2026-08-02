//! Production process bootstrap for the YM Connect desktop Bridge.

mod bootstrap;
mod error;
mod shutdown;

use std::process::ExitCode;

fn main() -> ExitCode {
    match bootstrap::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error::report_fatal(&error);
            ExitCode::FAILURE
        }
    }
}
