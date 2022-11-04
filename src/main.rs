mod encoded_string;
mod error;
mod main_result;

use std::{
	collections::HashSet,
	env,
	fs,
	hash::Hash,
	io::{self, BufRead, BufReader, BufWriter, Write, Seek, SeekFrom},
	path::Path,
	process::Command
};

use {
	encoded_string::EncodedString,
	error::Error,
	main_result::MainResult
};

use {
	itertools::Itertools,
	tempfile::{Builder, NamedTempFile, TempDir}
};

fn main() -> MainResult {
	work().into()
}

fn work() -> Result<(), Error> {
	let old_files = env::args().skip(1).collect::<Vec<String>>();
	let dups = duplicate_elements(old_files.clone());
	if !dups.is_empty() {
		return Err(Error::DuplicateElems(dups));
	}

	let mut tmpfile = NamedTempFile::new()?;
	let mut writer = BufWriter::new(&tmpfile);

	old_files.iter().try_for_each(|f| encode_to_file(&mut writer, f))?;
	writer.flush()?;
	drop(writer);

	let editor = env::var("EDITOR")
		.ok()
		.filter(|e| !e.is_empty())
		.ok_or(Error::NoEditor)?;

	Command::new(&editor)
		.arg(tmpfile.path().as_os_str())
		.spawn()
		.map_err(|err| Error::SpawnFailed(editor, err))?
		.wait()?;

	tmpfile.seek(SeekFrom::Start(0))?;

	let new_files = decode_from_file(&tmpfile)?;
	if old_files.len() != new_files.len() {
		return Err(Error::BadLengths);
	}

	let dups = duplicate_elements(new_files.iter().cloned().collect::<Vec<_>>());
	if !dups.is_empty() {
		return Err(Error::DuplicateElems(dups));
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
		.collect::<Result<Vec<String>, _>>()
}

fn try_move<'a>(
	conflicts: &mut Vec<&'a str>,
	tmpdir: &TempDir,
	old: &str,
	new: &'a str
) -> Result<(), io::Error> {
	if Path::new(new).exists() {
		let new_loc = tmpdir.path().to_str().unwrap().to_owned() + new;
		fs::rename(old, new_loc)?;
		conflicts.push(new);
	} else {
		fs::rename(old, new)?;
	}
	Ok(())
}

fn do_move(tmpdir: &TempDir, new: &str) -> Result<(), io::Error> {
	let old = tmpdir.path().to_str().unwrap().to_owned() + new;
	fs::rename(old, new)?;
	Ok(())
}
