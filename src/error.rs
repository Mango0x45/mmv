use std::{
	env,
	fmt::{self, Display},
	io
};

pub enum Error {
	BadLengths,
	DuplicateElems(Vec<String>),
	IOError(io::Error),
	NoEditor,
	SpawnFailed(String, io::Error),
}

impl Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let p = env::args().next().unwrap();
		match self {
			Self::BadLengths => writeln!(f, "{p}: Files have been added or removed during editing"),
			Self::DuplicateElems(ds) => ds.iter().try_for_each(
				|d| writeln!(f, "{p}: Multiple files named \"{}\" specified", d)
			),
			Self::IOError(e) => writeln!(f, "{p}: {e}"),
			Self::NoEditor => writeln!(f, "{p}: \"EDITOR\" environment variable is not set"),
			Self::SpawnFailed(ed, e) => writeln!(f, "{p}: Failed to spawn editor \"{ed}\": {e}")
		}
	}
}

impl From<io::Error> for Error {
	fn from(e: io::Error) -> Self {
		Self::IOError(e)
	}	
}
