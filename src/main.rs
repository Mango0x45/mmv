mod encoded_string;
mod error;
mod flags;
mod main_result;

use std::{
	collections::HashSet,
	env,
	fs,
	hash::Hash,
	io::{self, BufRead, BufReader, Write},
	path::Path,
	process::{Command, Stdio}
};

use {
	encoded_string::EncodedString,
	error::Error,
	main_result::MainResult
};

use {
	flags::Flags,
	getopt::{Opt, Parser},
	itertools::Itertools,
	tempfile::{Builder, NamedTempFile, TempDir}
};

fn main() -> MainResult {
	work().into()
}

fn work() -> Result<(), Error> {
	let mut argv = env::args().collect::<Vec<String>>();
	let mut flags = Flags { ..Default::default() };
	let mut opts = Parser::new(&argv, ":0a");

	loop {
		match opts.next().transpose() {
			Ok(v) => match v {
				None => break,
				Some(opt) => match opt {
					Opt('0', None) => flags.nul = true,
					Opt('e', None) => flags.encode = true,
					Opt('i', None) => flags.individual = true,
					_ => { return Err(Error::BadArgs); }
				}
			},
			Err(_) => { return Err(Error::BadArgs); }
		}
	}

	let rest = argv.split_off(opts.index());
	let cmd  = rest.get(0).ok_or(Error::BadArgs)?;
	let args = &rest[1..];

	let old_files: Vec<_> = io::stdin()
		.lines()
		.collect::<Result<_, _>>()
		.unwrap();
	if old_files.iter().any(|s| s.is_empty()) {
		return Err(Error::BadArgs);
	}

	let dups = duplicate_elements(old_files.clone());
	if !dups.is_empty() {
		return Err(Error::DupInputElems(dups));
	}

	let mut child = Command::new(cmd)
		.args(args)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.spawn()
		.map_err(|e| Error::SpawnFailed(cmd.to_owned(), e))?;

	{
		let mut stdin = child.stdin.take().expect("Failed to open stdin");
		old_files.iter().try_for_each(|f| writeln!(stdin, "{f}"))?;
	}

	let output = child.wait_with_output()?;
	if !output.status.success() {
		return Err(Error::Nop);
	}

	let new_files = String::from_utf8(output.stdout)?
		.lines()
		.map(|s| s.to_string())
		.collect::<Vec<String>>();

	if old_files.len() != new_files.len() {
		return Err(Error::BadLengths);
	}

	let dups = duplicate_elements(new_files.iter().cloned().collect::<Vec<_>>());
	if !dups.is_empty() {
		return Err(Error::DupOutputElems(dups));
	}

	let tmpdir = Builder::new().prefix("mmv").tempdir()?;
	let mut conflicts = Vec::<&str>::new();

	old_files
		.iter()
		.zip(new_files.iter())
		.filter(|(x, y)| x != y)
		.try_for_each(|(x, y)| try_move(&mut conflicts, &tmpdir, x, y))?;
	conflicts
		.iter()
		.try_for_each(|c| do_move(&tmpdir, c))?;

	Ok(())
}

fn duplicate_elements<T>(iter: T) -> Vec<T::Item>
where
	T: IntoIterator,
	T::Item: Clone + Eq + Hash
{
	let mut uniq = HashSet::new();
	iter
		.into_iter()
		.filter(|x| !uniq.insert(x.clone()))
		.unique()
		.collect::<Vec<_>>()
}

fn encode_to_file<W: Write>(f: &mut W, s: &str) -> io::Result<()> {
	s.chars().try_for_each(|c| {
		write!(f, "{}", match c {
			'\\' => "\\\\",
			'\n' => "\\n",
			_ => return write!(f, "{}", c),
		})
	})?;
	write!(f, "{}", '\n')
}

fn decode_from_file(tmpfile: &NamedTempFile) -> Result<Vec<String>, io::Error> {
	BufReader::new(tmpfile)
		.lines()
		.map(|r| match r {
			Ok(s) => {
				let es = EncodedString { s: s.bytes() };
				Ok(es.decode())
			},
			Err(_) => r
		})
		.filter(|r| match r {
			Ok(s) => !s.is_empty(),
			_ => true
		})
		.collect::<Result<Vec<String>, _>>()
}

fn try_move<'a>(
	conflicts: &mut Vec<&'a str>,
	tmpdir: &TempDir,
	old: &str,
	new: &'a str
) -> Result<(), io::Error> {
	if Path::new(new).exists() {
		let new_loc = tmpdir.path().to_str().unwrap().to_owned() + "/" + new;
		fs::rename(old, new_loc)?;
		conflicts.push(new);
	} else {
		fs::rename(old, new)?;
	}
	Ok(())
}

fn do_move(tmpdir: &TempDir, new: &str) -> Result<(), io::Error> {
	let old = tmpdir.path().to_str().unwrap().to_owned() + "/" + new;
	fs::rename(old, new)?;
	Ok(())
}
