use std::{
	env,
	fmt::{self, Display},
	io,
	str,
};

pub enum Error {
	BadArgs,
	BadDecoding(String),
	BadLengths,
	DupInputElems(Vec<String>),
	DupOutputElems(Vec<String>),
	FileExists(String),
	IOError(io::Error),
	Nop,
	SpawnFailed(String, io::Error),
	UTF8Error(std::str::Utf8Error),
}

impl Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let p = env::args().next().unwrap();
		match self {
			Self::BadArgs => writeln!(f, "Usage: {p} [-0ei] [--] utility [argument ...]"),
			Self::BadDecoding(s) => writeln!(f, "{p}: Decoding the text {s:?} failed!"),
			Self::BadLengths => writeln!(f, "{p}: Files have been added or removed during editing"),
			Self::DupInputElems(ds) => ds.iter().try_for_each(
				|d| writeln!(f, "{p}: Multiple input files named \"{}\" specified", d)
			),
			Self::DupOutputElems(ds) => ds.iter().try_for_each(
				|d| writeln!(f, "{p}: Multiple output files named \"{}\" specified", d)
			),
			Self::FileExists(s) => writeln!(f, "{p}: Attempted to overwrite existing file {s}"),
			Self::IOError(e) => writeln!(f, "{p}: {e}"),
			Self::Nop => Ok(()),
			Self::SpawnFailed(ed, e) => writeln!(f, "{p}: Failed to spawn editor \"{ed}\": {e}"),
			Self::UTF8Error(e) => writeln!(f, "{p}: {e}"),
		}
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
