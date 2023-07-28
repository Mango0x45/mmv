use std::{
	env,
	ffi::OsString,
	fmt::{self, Display, Formatter},
	io,
	path::PathBuf,
};

pub enum Error {
	BadArgs(Option<lexopt::Error>),
	BadDecoding(String),
	BadLengths,
	DuplicateInput(PathBuf),
	DuplicateOutput(PathBuf),
	IO(io::Error),
	Nop,
	SpawnFailed(OsString, io::Error),
}

impl Display for Error {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		let p = env::args().next().unwrap();
		match self {
			Self::BadArgs(o) => {
				if let Some(v) = o {
					writeln!(f, "{p}: {v}")?;
				}
				writeln!(f, "Usage: {p} [-0eiv] command [argument ...]")
			}
			Self::BadDecoding(s) => writeln!(f, "{p}: Decoding the file “{s}” failed!"),
			Self::BadLengths => writeln!(f, "{p}: Files have been added or removed during editing"),
			Self::DuplicateInput(s) => writeln!(
				f,
				"{p}: Input file “{}” specified more than once",
				s.to_string_lossy()
			),
			Self::DuplicateOutput(s) => writeln!(
				f,
				"{p}: Output file “{}” specified more than once",
				s.to_string_lossy()
			),
			Self::IO(e) => writeln!(f, "{p}: {e}"),
			Self::Nop => Ok(()),
			Self::SpawnFailed(ed, e) => writeln!(
				f,
				"{p}: Failed to spawn utility “{}”: {e}",
				ed.to_string_lossy()
			),
		}
	}
}

impl From<io::Error> for Error {
	fn from(e: io::Error) -> Self {
		Self::IO(e)
	}
}

impl From<lexopt::Error> for Error {
	fn from(e: lexopt::Error) -> Self {
		Self::BadArgs(Some(e))
	}
}
