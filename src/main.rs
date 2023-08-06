use std::{
	cmp::Reverse,
	collections::{hash_map::DefaultHasher, HashSet},
	env,
	ffi::OsString,
	fs,
	hash::{Hash, Hasher},
	io::{self, BufWriter, Read, Write},
	iter,
	path::{Component, Path, PathBuf},
	process::{self, Command, Stdio},
};

use itertools::Itertools;

use {
	cerm::{err, warn},
	tempfile::tempdir,
};

#[derive(Default)]
struct Flags {
	pub dryrun: bool,
	pub encode: bool,
	pub individual: bool,
	pub nul: bool,
	pub verbose: bool,
}

impl Flags {
	fn parse() -> Result<(Flags, Vec<OsString>), lexopt::Error> {
		use lexopt::prelude::*;

		let mut rest = Vec::with_capacity(env::args().len());
		let mut flags = Flags::default();
		let mut parser = lexopt::Parser::from_env();

		while let Some(arg) = parser.next()? {
			match arg {
				Short('0') | Long("nul") => flags.nul = true,
				Short('d') | Long("dryrun") => flags.dryrun = true,
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
}

fn usage(bad_flags: Option<lexopt::Error>) -> ! {
	let p = env::args().next().unwrap();
	if let Some(e) = bad_flags {
		warn!("{e}");
	}
	eprintln!("Usage: {p} [-0deiv] command [argument ...]");
	process::exit(1);
}

fn main() {
	if let Err(e) = work() {
		err!("{e}");
	}
}

fn work() -> Result<(), io::Error> {
	let (flags, rest) = match Flags::parse() {
		Ok(a) => a,
		Err(e) => usage(Some(e)),
	};
	let (cmd, args) = rest.split_first().unwrap_or_else(|| usage(None));

	// Collect sources from standard input
	let srcs = io::stdin()
		.bytes()
		.map(|x| {
			x.unwrap_or_else(|e| {
				err!("{e}");
			})
		})
		.group_by(|b| *b == (b'\0' + b'\n' * !flags.nul as u8));
	let srcs = srcs
		.into_iter()
		.filter(|(x, _)| !x)
		.map(|(_, x)| String::from_utf8(x.collect_vec()))
		.collect::<Result<Vec<_>, _>>()
		.unwrap_or_else(|e| {
			err!("{e}");
		});

	let mut dsts = Vec::with_capacity(srcs.len());
	if flags.individual {
		run_indiv(&srcs, &mut dsts, &flags, cmd, args)?;
	} else {
		run_multi(&srcs, &mut dsts, &flags, cmd, args)?;
	}

	if dsts.len() != srcs.len() {
		err!("Files have been added or removed during editing");
	}

	let mut uniq_srcs: HashSet<PathBuf> = HashSet::with_capacity(srcs.len());
	let mut uniq_dsts: HashSet<PathBuf> = HashSet::with_capacity(dsts.len());

	let dir = tempdir()?;
	let mut ps = srcs
		.iter()
		.zip(dsts)
		.map(|(s, d)| -> Result<(PathBuf, PathBuf, PathBuf), io::Error> {
			let s = fs::canonicalize(s)?;
			let d = env::current_dir()?.join(Path::new(&d));
			let d = normalize_path(&d);

			if !uniq_srcs.insert(s.clone()) {
				err!(
					"Input file “{}” specified more than once",
					s.to_string_lossy()
				);
			} else if !uniq_dsts.insert(d.clone()) {
				err!(
					"Output file “{}” specified more than once",
					d.to_string_lossy()
				);
			} else {
				let mut hasher = DefaultHasher::new();
				s.hash(&mut hasher);
				let file = hasher.finish().to_string();
				let t = dir.path().join(&file);
				Ok((s, t, d))
			}
		})
		.collect::<Result<Vec<_>, io::Error>>()?;

	/* Sort the src/dst pairs so that the sources with the longest componenets
	   come first. */
	ps.sort_by_key(|s| Reverse(s.0.components().count()));

	if flags.dryrun {
		for (s, _, d) in ps {
			println!("{} -> {}", s.as_path().display(), d.as_path().display());
		}
	} else {
		for (s, t, _) in ps.iter() {
			move_path(&flags, &s, &t);
		}
		for (_, t, d) in ps.iter().rev() {
			move_path(&flags, &t, &d);
		}
	}

	Ok(())
}

fn run_indiv(
	srcs: &Vec<String>,
	dsts: &mut Vec<String>,
	flags: &Flags,
	cmd: &OsString,
	args: &[OsString],
) -> Result<(), io::Error> {
	for src in srcs {
		let mut child = Command::new(cmd)
			.args(args)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.spawn()
			.unwrap_or_else(|e| {
				err!("Failed to spawn utility: “{}”: {e}", cmd.to_str().unwrap());
			});

		{
			let mut ci = child.stdin.take().unwrap_or_else(|| {
				err!("Could not open the child process’ stdin");
			});
			write!(
				ci,
				"{}",
				if flags.encode {
					encode_string(src)
				} else {
					src.to_owned()
				}
			)?;
			ci.write_all(if flags.nul { &[b'\0'] } else { &[b'\n'] })?;
		}

		let mut co = child.stdout.take().unwrap_or_else(|| {
			err!("Count not open the child process’ stdout.");
		});
		let mut s = String::with_capacity(src.len());
		co.read_to_string(&mut s)?;
		match s.chars().last().unwrap_or_else(|| {
			err!("Filename can’t be the empty string");
		}) {
			'\n' | '\0' => {
				s.pop();
			}
			_ => {}
		};
		dsts.push(if flags.encode {
			decode_string(s.as_str())
		} else {
			s
		});

		/* If the process failed, it is expected to print an error message; as such,
		we exit directly. */
		if !child.wait()?.success() {
			process::exit(1);
		}
	}

	Ok(())
}

fn run_multi(
	srcs: &Vec<String>,
	dsts: &mut Vec<String>,
	flags: &Flags,
	cmd: &OsString,
	args: &[OsString],
) -> Result<(), io::Error> {
	let mut child = Command::new(cmd)
		.args(args)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.spawn()
		.unwrap_or_else(|e| {
			err!("Failed to spawn utility “{}”: {e}", cmd.to_str().unwrap());
		});

	// Pass the source files to the child process.
	{
		let ci = child.stdin.take().unwrap_or_else(|| {
			err!("Could not open the child process’ stdin");
		});
		let mut ci = BufWriter::new(ci);
		for src in srcs {
			write!(
				ci,
				"{}",
				if flags.encode {
					encode_string(src)
				} else {
					src.to_owned()
				}
			)?;
			ci.write_all(if flags.nul { &[b'\0'] } else { &[b'\n'] })?;
		}
	}

	// Read the destination file list from the process.
	let co = child.stdout.take().unwrap_or_else(|| {
		err!("Count not open the child process’ stdout.");
	});
	let groups = co
		.bytes()
		.map(|x| {
			x.unwrap_or_else(|e| {
				err!("{e}");
			})
		})
		.group_by(|b| *b == (b'\0' + b'\n' * !flags.nul as u8));
	groups
		.into_iter()
		.filter_map(|(x, y)| match x {
			true => None,
			false => Some(y),
		})
		.for_each(|x| {
			let dst = String::from_utf8(x.collect_vec()).unwrap_or_else(|e| {
				err!("{e}");
			});
			dsts.push(if flags.encode {
				decode_string(&dst)
			} else {
				dst
			});
		});

	/* If the process failed, it is expected to print an error message; as such,
	   we exit directly. */
	if !child.wait()?.success() {
		process::exit(1);
	}

	Ok(())
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

fn decode_string(s: &str) -> String {
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
		.unwrap_or_else(|_| {
			err!("Decoding the file “{}” failed", s);
		})
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

fn move_path(flags: &Flags, from: &PathBuf, to: &PathBuf) {
	if flags.verbose {
		println!("{} -> {}", from.as_path().display(), to.as_path().display());
	}

	if !flags.dryrun {
		copy_and_remove_file_or_dir(&from, &to).unwrap_or_else(|(f, e)| {
			err!("{}: {e}", f.to_string_lossy());
		});
	}
}

fn copy_and_remove_file_or_dir<'a>(
	from: &'a PathBuf,
	to: &'a PathBuf,
) -> Result<(), (&'a PathBuf, io::Error)> {
	let data = fs::metadata(&from).map_err(|e| (from, e))?;
	if data.is_dir() {
		fs::create_dir(&to).map_err(|e| (to, e))?;
		fs::remove_dir(&from).map_err(|e| (from, e))?
	} else {
		fs::copy(&from, &to).map_err(|e| (to, e))?;
		fs::remove_file(&from).map_err(|e| (from, e))?
	}
	Ok(())
}
