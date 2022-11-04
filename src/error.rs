use std::{
	env,
	fmt::{self, Display},
	io,
	string
};

pub enum Error {
	BadArgs,
	BadLengths,
	DupInputElems(Vec<String>),
	DupOutputElems(Vec<String>),
	IOError(io::Error),
	Nop,
	UTF8Error(string::FromUtf8Error),
	SpawnFailed(String, io::Error),
}

impl Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let p = env::args().next().unwrap();
		match self {
			Self::BadArgs => writeln!(f, "Usage: {p} [-0ei] [--] utility [argument ...]"),
			Self::BadLengths => writeln!(f, "{p}: Files have been added or removed during editing"),
			Self::DupInputElems(ds) => ds.iter().try_for_each(
				|d| writeln!(f, "{p}: Multiple input files named \"{}\" specified", d)
			),
			Self::DupOutputElems(ds) => ds.iter().try_for_each(
				|d| writeln!(f, "{p}: Multiple output files named \"{}\" specified", d)
			),
			Self::IOError(e) => writeln!(f, "{p}: {e}"),
			Self::Nop => Ok(()),
			Self::UTF8Error(e) => writeln!(f, "{p}: {e}"),
			Self::SpawnFailed(ed, e) => writeln!(f, "{p}: Failed to spawn editor \"{ed}\": {e}")
		}
	}
}

impl From<io::Error> for Error {
	fn from(e: io::Error) -> Self {
		Self::IOError(e)
	}	
}

impl From<string::FromUtf8Error> for Error {
	fn from(e: string::FromUtf8Error) -> Self {
		Self::UTF8Error(e)
	}
}
