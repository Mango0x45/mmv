use std::{
	env,
	ffi::OsString,
	fmt::{self, Display},
	io,
	path::PathBuf,
	str,
};

use mmv::ConsError;

use lexopt;

pub enum Error {
	BadArgs(Option<lexopt::Error>),
	BadDecoding(String),
	BadLengths,
	ConsError(ConsError<PathBuf>),
	FileExists(String),
	IOError(io::Error),
	Nop,
	SpawnFailed(OsString, io::Error),
	UTF8Error(std::str::Utf8Error),
}

impl Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let p = env::args().next().unwrap();
		match self {
			Self::BadArgs(o) => {
				if let Some(v) = o {
					writeln!(f, "{p}: {v}")?;
				}
				writeln!(f, "Usage: {p} [-0eiv] utility [argument ...]")
			},
			Self::BadDecoding(s) => writeln!(f, "{p}: Decoding the text {s:?} failed!"),
			Self::BadLengths => writeln!(f, "{p}: Files have been added or removed during editing"),
			Self::ConsError(e) => writeln!(f, "{p}: The move set could not be constructed: {e}"),
			Self::FileExists(s) => writeln!(f, "{p}: Attempted to overwrite existing file {s}"),
			Self::IOError(e) => writeln!(f, "{p}: {e}"),
			Self::Nop => Ok(()),
			Self::SpawnFailed(ed, e) => writeln!(
				f,
				"{p}: Failed to spawn utility \"{}\": {e}",
				ed.to_string_lossy()
			),
			Self::UTF8Error(e) => writeln!(f, "{p}: {e}"),
		}
	}
}

impl From<ConsError<PathBuf>> for Error {
	fn from(e: ConsError<PathBuf>) -> Self {
		Self::ConsError(e)
	}
}

impl From<io::Error> for Error {
	fn from(e: io::Error) -> Self {
		Self::IOError(e)
	}
}

impl From<str::Utf8Error> for Error {
	fn from(e: str::Utf8Error) -> Self {
		Self::UTF8Error(e)
	}
}

impl From<lexopt::Error> for Error {
	fn from(e: lexopt::Error) -> Self {
		Self::BadArgs(Some(e))
	}
}
