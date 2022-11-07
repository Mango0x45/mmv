mod error;
mod flags;
mod main_result;

use std::{
	borrow::Cow,
	collections::HashMap,
	env,
	fs,
	hash::Hash,
	io::{self, BufRead, BufReader, Write},
	iter,
	path::Path,
	process::{Command, Stdio}
};

use {
	error::Error,
	main_result::MainResult
};

use {
	flags::Flags,
	getopt::{Opt, Parser},
	tempfile::{Builder, NamedTempFile, TempDir}
};

fn main() -> MainResult {
	work().into()
}

fn work() -> Result<(), Error> {
	// TODO: Don't allocate the arguments in a Vec!
	let argv = env::args().collect::<Vec<String>>();
	let mut flags = Flags::default();
	let mut opts = Parser::new(&argv, ":0eiv");

	// TODO: Perhaps implement FromIterator for Flags?
	opts.by_ref().map(Result::ok).try_for_each(|o| Some(match o? {
		Opt('0', None) => flags.nul        = true,
		Opt('e', None) => flags.encode     = true,
		Opt('i', None) => flags.individual = true,
		Opt('v', None) => flags.verbose    = true,
		_              => return None,
	})).ok_or(Error::BadArgs)?;

	let (cmd, args) = argv[opts.index()..].split_first()
		.ok_or(Error::BadArgs)?;

	let old_files = io::stdin().lines()
		.map(|l| l.map_err(Error::from)
			 .and_then(|l| if l.is_empty() { Err(Error::BadArgs) } else { Ok(l) }))
		.collect::<Result<Vec<String>, Error>>()?;

	duplicate_elements(old_files.iter()).map_or(Ok(()), |dups| {
		Err(Error::DupInputElems(dups.cloned().collect()))
	})?;

	let mut child = Command::new(cmd)
		.args(args)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.spawn()
		.map_err(|e| Error::SpawnFailed(cmd.to_owned(), e))?;

	// TODO: Don’t use expect, create a custom error for this
	child.stdin.take().map(|mut stdin| {
		old_files.iter().try_for_each(|f|
			if flags.encode {
				writeln!(stdin, "{}", encode_string(&f))
			} else {
				writeln!(stdin, "{f}")
			}
		)
	}).expect("Failed to open child stdin")?;

	// On failure we exit with NOP error, because the expectation is that the process we spawned
	// that failed will have printed an error message to stderr.
	let output = child.wait_with_output().map_err(Error::from)
		.and_then(|o| if o.status.success() { Ok(o) } else { Err(Error::Nop) })?;

	let new_files = std::str::from_utf8(&output.stdout)?.lines()
		.map(|s| Ok(if flags.encode {
			Cow::Owned(decode_string(Cow::Borrowed(s))?)
		} else {
			Cow::Borrowed(s)
		})).collect::<Result<Vec<Cow<'_, str>>, Error>>()?;

	if old_files.len() != new_files.len() {
		return Err(Error::BadLengths);
	}

	duplicate_elements(new_files.iter()).map_or(Ok(()), |dups| {
		Err(Error::DupOutputElems(dups.map(|d| d.to_string()).collect()))
	})?;

	let tmpdir = Builder::new().prefix("mmv").tempdir()?;
	let mut conflicts = Vec::<&str>::new();

	iter::zip(old_files.iter(), new_files.iter())
		.filter(|(x, y)| x != y)
		.try_for_each(|(x, y)| try_move(&mut conflicts, &flags, &tmpdir, x, y))?;
	conflicts.iter().try_for_each(|c| do_move(&flags, &tmpdir, c))?;

	Ok(())
}

fn duplicate_elements<T>(iter: T) -> Option<impl Iterator<Item = T::Item>>
where
	T: IntoIterator,
	T::Item: Eq + Hash
{
	let mut elems: HashMap<T::Item, bool> = HashMap::new();
	let mut fail = false;

	for elem in iter.into_iter() {
		elems.entry(elem)
			.and_modify(|b| { *b = true; fail = true; })
			.or_insert(false);
	}

	fail.then(|| elems.into_iter().filter_map(|(e, b)| b.then_some(e)))
}

fn encode_string(s: &str) -> String {
	s.chars().flat_map(|c| {
		let cs = match c {
			'\\' => ['\\', '\\'],
			'\t' => ['\\', 't' ],
			'\n' => ['\\', 'n' ],
			_    => [c,    '\0'],
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
				(false, _)     => {},
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
				(false, _)     => true,
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

fn try_move<'a>(
	conflicts: &mut Vec<&'a str>,
	flags: &Flags,
	tmpdir: &TempDir,
	old: &str,
	new: &'a str
) -> Result<(), io::Error> {
	if Path::new(new).exists() {
		let new_loc = tmpdir.path().to_str().unwrap().to_owned() + "/" + new;
		fs::rename(old, &new_loc)?;
		if flags.verbose {
			eprintln!("renamed '{old}' -> '{new_loc}'");
		}
		conflicts.push(new);
	} else {
		fs::rename(old, new)?;
		if flags.verbose {
			eprintln!("renamed '{old}' -> '{new}'");
		}
	}
	Ok(())
}

fn do_move(flags: &Flags, tmpdir: &TempDir, new: &str) -> Result<(), io::Error> {
	let old = tmpdir.path().to_str().unwrap().to_owned() + "/" + new;
	fs::rename(&old, new)?;
	if flags.verbose {
		eprintln!("renamed '{old}' -> '{new}'");
	}
	Ok(())
}
