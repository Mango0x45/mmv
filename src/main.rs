mod encoded_string;
mod error;
mod main_result;

use std::{
	collections::HashSet,
	env,
	hash::Hash,
	io::{self, BufRead, BufReader, BufWriter, Write, Seek, SeekFrom},
	process::Command
};

use encoded_string::*;
use error::*;
use main_result::*;

use {
	itertools::Itertools,
	tempfile::NamedTempFile
};

fn main() -> MainResult {
	work().into()
}

fn work() -> Result<(), Error> {
	let files = env::args().skip(1).collect::<Vec<String>>();
	let dups = duplicate_elements(files.clone());
	if !dups.is_empty() {
		return Err(Error::DuplicateElems(dups));
	}

	let mut tmpfile = NamedTempFile::new()?;
	let mut writer = BufWriter::new(&tmpfile);

	files.iter().try_for_each(|f| encode_to_file(&mut writer, f))?;
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

	assert_changes(
		    files.iter().cloned().collect(),
		new_files.iter().cloned().collect()
	)?;
	new_files.iter().for_each(|f| println!("{}", f));

	Ok(())
}

fn assert_changes(old: Vec<String>, new: Vec<String>) -> Result<(), Error> {
	if old.len() != new.len() {
		return Err(Error::BadLengths);
	}

	let dups = duplicate_elements(new);
	if !dups.is_empty() {
		return Err(Error::DuplicateElems(dups));
	}

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
			}
		)
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
