mod error;
mod main_result;

use std::{
	borrow::Cow,
	env,
	ffi::OsString,
	io::{self, BufRead, BufReader, Write, BufWriter},
	iter,
	path::Path,
	process::{Command, Stdio}
};

use {
	error::Error,
	main_result::MainResult,
	mmv::{ConsError, Move}
};

use tempfile::{Builder, NamedTempFile, TempDir};

#[derive(Default)]
struct Flags {
	pub encode: bool,
	pub individual: bool,
	pub nul: bool,
	pub verbose: bool
}

fn main() -> MainResult {
	work().into()
}

fn work() -> Result<(), Error> {
	let (flags, rest) = parse_args()?;
	let (cmd, args) = rest.split_first().ok_or(Error::BadArgs(None))?;

	// Collect source paths from standard input.
	let srcs = io::stdin().lines()
		.map(|l| l.map_err(Error::from).and_then(|l|
			if l.is_empty() { Err(Error::BadArgs(None)) } else { Ok(l) }))
		.collect::<Result<Vec<String>, Error>>()?;

	// Launch the child process.
	let mut child = Command::new(cmd).args(args)
		.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()
		.map_err(|e| Error::SpawnFailed(cmd.to_owned(), e))?;

	// Pass the source file list to the process.
	// TODO: Don’t use expect, create a custom error for this
	{
		let ci = child.stdin.take().expect("Could not open the child process' stdin.");
		let mut ci = BufWriter::new(ci);
		for src in srcs.iter() {
			if flags.encode {
				writeln!(ci, "{}", encode_string(&src))?;
			} else {
				writeln!(ci, "{}", src)?;
			}
		}
	}

	// Read the destination file list from the process.
	let mut dsts = Vec::with_capacity(srcs.len());
	{
		let co = child.stdout.take().expect("Could not open the child process' stdout.");
		let co = BufReader::new(co);
		// TODO: Don't allocate an intermediary String per line, by using the BufReader buffer.
		co.lines().try_for_each(|dst| {
			if dsts.len() == srcs.len() { return Err(Error::BadLengths); }
			if flags.encode {
				dsts.push(decode_string(Cow::Owned(dst?))?);
			} else {
				dsts.push(dst?);
			}
			Ok(())
		})?;

		if dsts.len() != srcs.len() { return Err(Error::BadLengths); }
	}

	// If the process failed, it is expected to print an error message; as such, we exit directly.
	if !child.wait()?.success() { return Err(Error::Nop); }

	// Set up the move.
	let this = Move::new();
	ConsError::from_iter(iter::zip(srcs.iter(), dsts.iter())
		.filter_map(|(src, dst)| this.add(src.as_ref(), dst.as_ref()).err())
		.map(|err| err.map_paths(Path::to_path_buf)))?;

	// TODO: Execute the move.

	Ok(())
}

fn parse_args() -> Result<(Flags, Vec<OsString>), lexopt::Error> {
	use lexopt::prelude::*;

	let mut rest = Vec::with_capacity(env::args().len());
	let mut flags = Flags::default();
	let mut parser = lexopt::Parser::from_env();
	while let Some(arg) = parser.next()? {
		match arg {
			Short('0') | Long("nul")        => flags.nul        = true,
			Short('e') | Long("encode")     => flags.encode     = true,
			Short('i') | Long("individual") => flags.individual = true,
			Short('v') | Long("verbose")    => flags.verbose    = true,
			Value(v) => {
				rest.push(v);
				rest.extend(iter::from_fn(|| parser.value().ok()));
			},
			_ => return Err(arg.unexpected())
		}
	}

	Ok((flags, rest))
}

fn encode_string(s: &str) -> String {
	s.chars().flat_map(|c| {
		let cs = match c {
			'\\' => ['\\', '\\'],
			'\t' => ['\\', 't' ],
			'\n' => ['\\', 'n' ],
			_	 => [c,    '\0'],
		};
		cs.into_iter().enumerate()
			.filter(|(i, c)| *i != 1 || *c != '\0')
			.map(|(_, c)| c)
	}).collect::<String>()
}

fn decode_string(s: Cow<'_, str>) -> Result<String, Error> {
	let mut pv = false;
	let mut fail = false;

	match s {
		Cow::Owned(s) => {
			let bs = s.as_bytes();
			bs.iter().for_each(|b| match (pv, *b) {
				(true, b'\\')  => pv = false,
				(true, b'n')   => pv = false,
				(true, b't')   => pv = false,
				(true, _)	   => { pv = false; fail = true },
				(false, b'\\') => pv = true,
				(false, _)	   => {},
			});

			if fail || pv {
				return Err(Error::BadDecoding(s.to_string()));
			}

			let mut bs = s.into_bytes();
			bs.retain_mut(|b| match (pv, *b) {
				(true, b'\\')  => { pv = false; true },
				(true, b'n')   => { pv = false; *b = b'\n'; true },
				(true, b't')   => { pv = false; *b = b'\t'; true },
				(true, _)	   => unreachable!(),
				(false, b'\\') => { pv = true; false },
				(false, _)	   => true,
			});

			Ok(String::from_utf8(bs).unwrap())
		},

		Cow::Borrowed(s) => {
			s.chars()
				.map(|c| Ok(match (pv, c) {
					(true, '\\')  => { pv = false; Some('\\') },
					(true, 'n')   => { pv = false; Some('\n') },
					(true, 't')   => { pv = false; Some('\t') },
					(true, _)	  => { pv = false; return Err(()); },
					(false, '\\') => { pv = true;  None },
					(false, _)	  => { Some(c) }
				}))
				.filter_map(Result::transpose)
				.collect::<Result<String, ()>>()
				.map_err(|()| Error::BadDecoding(s.to_string()))
		},
	}
}

fn decode_from_file(tmpfile: &NamedTempFile) -> Result<Vec<String>, Error> {
	BufReader::new(tmpfile).lines()
		.map(|l| l.map_err(Error::from).and_then(|l| decode_string(Cow::Owned(l))))
		.filter(|s| s.as_ref().map_or(true, |s| !s.is_empty()))
		.collect()
}
