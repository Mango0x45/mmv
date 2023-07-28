use std::{
	io::{self, Write},
	process::{ExitCode, Termination},
};

use super::error::Error;

pub enum MainResult {
	Success,
	Failure(Error),
}

impl Termination for MainResult {
	fn report(self) -> ExitCode {
		match self {
			Self::Success => ExitCode::SUCCESS,
			Self::Failure(e) => {
				let _ = write!(io::stderr(), "{e}");
				ExitCode::FAILURE
			}
		}
	}
}

impl From<Result<(), Error>> for MainResult {
	fn from(r: Result<(), Error>) -> Self {
		match r {
			Ok(()) => Self::Success,
			Err(e) => Self::Failure(e),
		}
	}
}
