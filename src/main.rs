mod error;
mod main_result;

use std::{
	cmp::Reverse,
	collections::{hash_map::DefaultHasher, HashSet},
	env,
	ffi::OsString,
	fs,
	hash::{Hash, Hasher},
	io::{self, BufRead, BufReader, BufWriter, Write},
	iter,
	path::{Component, Path, PathBuf},
	process::{Command, Stdio},
};

use tempfile::tempdir;

use {error::Error, main_result::MainResult};

#[derive(Default)]
struct Flags {
	pub dryrun: bool,
	pub encode: bool,
	pub individual: bool,
	pub nul: bool,
	pub verbose: bool,
}

fn main() -> MainResult {
	work().into()
}

fn work() -> Result<(), Error> {
	let (flags, rest) = parse_args()?;
	let (cmd, args) = rest.split_first().ok_or(Error::BadArgs(None))?;

	// Collect sources from standard input
	let srcs = io::stdin()
		.lines()
		.map(|l| {
			l.map_err(Error::from).and_then(|l| {
				if l.is_empty() {
					Err(Error::BadArgs(None))
				} else {
					Ok(l)
				}
			})
		})
		.collect::<Result<Vec<String>, Error>>()?;

	// Spawn the child process
	let mut child = Command::new(cmd)
		.args(args)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.spawn()
		.map_err(|e| Error::SpawnFailed(cmd.to_owned(), e))?;

	// Pass the source files to the child process.
	// TODO: Don’t use expect; create a custom error
	{
		let ci = child
			.stdin
			.take()
			.expect("Could not open the child process’ stdin");
		let mut ci = BufWriter::new(ci);
		if flags.encode {
			srcs.iter()
				.try_for_each(|src| writeln!(ci, "{}", encode_string(src)))?;
		} else {
			srcs.iter().try_for_each(|src| writeln!(ci, "{}", src))?;
		}
	}

	// Read the destination file list from the process.
	let mut dsts = Vec::with_capacity(srcs.len());
	{
		let co = child
			.stdout
			.take()
			.expect("Could not open the child process’ stdout.");
		let co = BufReader::new(co);

		// TODO: Don’t allocate an intermediary String per line, by using the BufReader buffer.
		co.lines().try_for_each(|dst| -> Result<(), Error> {
			if flags.encode {
				dsts.push(decode_string(&dst?)?);
			} else {
				dsts.push(dst?);
			}
			Ok(())
		})?;

		if dsts.len() != srcs.len() {
			return Err(Error::BadLengths);
		}
	}

	/* If the process failed, it is expected to print an error message; as such,
	   we exit directly. */
	if !child.wait()?.success() {
		return Err(Error::Nop);
	}

	let mut uniq_srcs: HashSet<PathBuf> = HashSet::with_capacity(srcs.len());
	let mut uniq_dsts: HashSet<PathBuf> = HashSet::with_capacity(dsts.len());

	let dir = tempdir()?;
	let mut ps = srcs
		.iter()
		.zip(dsts)
		.map(|(s, d)| -> Result<(PathBuf, PathBuf, PathBuf), Error> {
			let s = fs::canonicalize(s)?;
			let d = env::current_dir()?.join(Path::new(&d));
			let d = normalize_path(&d);

			if !uniq_srcs.insert(s.clone()) {
				Err(Error::DuplicateInput(s))
			} else if !uniq_dsts.insert(d.clone()) {
				Err(Error::DuplicateOutput(d))
			} else {
				let mut hasher = DefaultHasher::new();
				s.hash(&mut hasher);
				let file = hasher.finish().to_string();
				let t = dir.path().join(&file);
				Ok((s, t, d))
			}
		})
		.collect::<Result<Vec<_>, Error>>()?;

	/* Sort the src/dst pairs so that the sources with the longest componenets
	   come first. */
	ps.sort_by_key(|s| Reverse(s.0.components().count()));

	for (s, t, _) in ps.iter() {
		move_path(&flags, &s, &t)?;
	}
	for (_, t, d) in ps.iter().rev() {
		move_path(&flags, &t, &d)?;
	}

	Ok(())
}

fn parse_args() -> Result<(Flags, Vec<OsString>), lexopt::Error> {
	use lexopt::prelude::*;

	let mut rest = Vec::with_capacity(env::args().len());
	let mut flags = Flags::default();
	let mut parser = lexopt::Parser::from_env();
	while let Some(arg) = parser.next()? {
		match arg {
			Short('0') | Long("nul") => flags.nul = true,
			Short('d') | Long("dryrun") => {
				flags.dryrun = true;
				flags.verbose = true;
			},
			Short('e') | Long("encode") => flags.encode = true,
			Short('i') | Long("individual") => flags.individual = true,
			Short('v') | Long("verbose") => flags.verbose = true,
			Value(v) => {
				rest.push(v);
				rest.extend(iter::from_fn(|| parser.value().ok()));
			}
			_ => return Err(arg.unexpected()),
		}
	}

	Ok((flags, rest))
}

fn encode_string(s: &str) -> String {
	s.chars()
		.flat_map(|c| {
			let cs = match c {
				'\\' => ['\\', '\\'],
				'\n' => ['\\', 'n'],
				_ => [c, '\0'],
			};
			cs.into_iter()
				.enumerate()
				.filter(|(i, c)| *i != 1 || *c != '\0')
				.map(|(_, c)| c)
		})
		.collect::<String>()
}

fn decode_string(s: &str) -> Result<String, Error> {
	let mut pv = false;

	s.chars()
		.map(|c| {
			Ok(match (pv, c) {
				(true, '\\') => {
					pv = false;
					Some('\\')
				}
				(true, 'n') => {
					pv = false;
					Some('\n')
				}
				(true, _) => {
					pv = false;
					return Err(());
				}
				(false, '\\') => {
					pv = true;
					None
				}
				(false, _) => Some(c),
			})
		})
		.filter_map(Result::transpose)
		.collect::<Result<String, ()>>()
		.map_err(|()| Error::BadDecoding(s.to_string()))
}

/* Blatantly stolen from the Cargo source code.  This is MIT licensed. */
fn normalize_path(path: &Path) -> PathBuf {
	let mut components = path.components().peekable();
	let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
		components.next();
		PathBuf::from(c.as_os_str())
	} else {
		PathBuf::new()
	};

	for component in components {
		match component {
			Component::Prefix(..) => unreachable!(),
			Component::RootDir => {
				ret.push(component.as_os_str());
			}
			Component::CurDir => {}
			Component::ParentDir => {
				ret.pop();
			}
			Component::Normal(c) => {
				ret.push(c);
			}
		}
	}
	ret
}

fn move_path(flags: &Flags, from: &PathBuf, to: &PathBuf) -> io::Result<()> {
	if flags.verbose {
		println!("{} -> {}", from.as_path().display(), to.as_path().display());
	}

	if !flags.dryrun {
		copy_and_remove_file_or_dir(&from, &to)?;
	}

	Ok(())
}

fn copy_and_remove_file_or_dir<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<()> {
	let data = fs::metadata(&from)?;
	if data.is_dir() {
		fs::create_dir(&to)?;
		fs::remove_dir(&from)
	} else {
		fs::copy(&from, &to)?;
		fs::remove_file(&from)
	}
}
