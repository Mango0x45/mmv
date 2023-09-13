use std::{
	cmp::Reverse,
	collections::{hash_map::DefaultHasher, HashSet},
	env,
	ffi::OsString,
	fs,
	hash::{Hash, Hasher},
	io::{self, BufWriter, Read, Write},
	iter,
	path::{Component, Display, Path, PathBuf},
	process::{self, Command, Stdio},
	time::{SystemTime, UNIX_EPOCH},
};

use itertools::Itertools;

use {
	cerm::{err, require, warn},
	tempfile::tempdir,
};


const MMV_DEFAULT_NAME: &str = "mmv";
const MCP_DEFAULT_NAME: &str = "mcp";

struct Flags {
	pub backup: bool,
	pub dryrun: bool,
	pub encode: bool,
	pub individual: bool,
	pub mcp: bool,
	pub nul: bool,
	pub verbose: bool,
}

impl Default for Flags {
	fn default() -> Self {
		Flags {
			backup: true,
			dryrun: false,
			encode: false,
			individual: false,
			mcp: false,
			nul: false,
			verbose: false,
		}
	}
}

impl Flags {
	fn parse() -> Result<(Flags, Vec<OsString>), lexopt::Error> {
		use lexopt::prelude::*;

		let mut rest = Vec::with_capacity(env::args().len());
		let mut flags = Flags::default();
		let mut parser = lexopt::Parser::from_env();

		let argv0 = env::args().next().unwrap();
		let p = Path::new(&argv0).file_name().unwrap();

		let mcp_name = option_env!("MCP_NAME").unwrap_or(MCP_DEFAULT_NAME);
		if p == mcp_name {
			flags.mcp = true;
			flags.backup = false;
		}

		while let Some(arg) = parser.next()? {
			match arg {
				Short('0') | Long("nul") => flags.nul = true,
				Short('d') | Long("dry-run") => flags.dryrun = true,
				Short('e') | Long("encode") => flags.encode = true,
				Short('i') | Long("individual") => flags.individual = true,
				Short('n') | Long("no-backup") if !flags.mcp => flags.backup = false,
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
	if let Some(e) = bad_flags {
		warn!("{e}");
	}
	let argv0 = env::args().next().unwrap();
	let p = Path::new(&argv0).file_name().unwrap();
	let mcp_name = option_env!("MCP_NAME").unwrap_or(MCP_DEFAULT_NAME);
	if p == mcp_name {
		eprintln!("Usage: {} [-0deiv] command [argument ...]", p.to_str().unwrap());
	} else {
		eprintln!("Usage: {} [-0deinv] command [argument ...]", p.to_str().unwrap());
	}
	process::exit(1);
}

fn main() {
	require!(work())
}

fn work() -> Result<(), io::Error> {
	let (flags, rest) = match Flags::parse() {
		Ok(a) => a,
		Err(e) => usage(Some(e)),
	};
	let (cmd, args) = rest.split_first().unwrap_or_else(|| usage(None));

	/* Collect sources from standard input */
	let srcs = io::stdin()
		.bytes()
		.map(|x| require!(x))
		.group_by(|b| is_terminal(flags.nul, b));
	let srcs = srcs
		.into_iter()
		.filter(|(x, _)| !x)
		.map(|(_, x)| String::from_utf8(x.collect_vec()))
		.collect::<Result<Vec<_>, _>>();
	let srcs = require!(srcs);

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
	if flags.verbose {
		eprintln!("created directory ‘{}’", dir.path().display());
	}

	let ps = srcs
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
		.map(|x| require!(x))
		.sorted_by_key(|s| Reverse(s.0.components().count()))
		.collect_vec();

	let mut cache_dir = PathBuf::default();
	if flags.backup {
		let ts = require!(SystemTime::now().duration_since(UNIX_EPOCH))
			.as_nanos()
			.to_string();
		let cache_base = env::var("XDG_CACHE_HOME").unwrap_or_else(|_| {
			err!("XDG_CACHE_HOME variable must be set");
		});
		let mmv_name = option_env!("MMV_NAME").unwrap_or(MMV_DEFAULT_NAME);
		cache_dir = [
			Path::new(cache_base.as_str()),
			Path::new(mmv_name),
			Path::new(ts.as_str()),
		]
		.iter()
		.collect::<PathBuf>();
		fs::create_dir_all(&cache_dir)?;

		if flags.verbose {
			eprintln!("created directory ‘{}’", cache_dir.display());
		}

		let cwd = require!(env::current_dir());
		require!(env::set_current_dir(&cache_dir));
		backup_srcs(&flags, &cache_dir, ps.iter().map(|(s, _, _)| s))?;
		require!(env::set_current_dir(cwd));
	}

	if flags.dryrun {
		for (s, _, d) in ps {
			eprintln!(
				"{} ‘{}’ -> ‘{}’",
				if flags.mcp { "copied" } else { "renamed" },
				disp(&s),
				disp(&d)
			);
		}
	} else {
		for (s, t, _) in ps.iter() {
			move_path(&flags, &s, &t);
		}
		for (_, t, d) in ps.iter().rev() {
			move_path(&flags, &t, &d);
		}
	}

	if flags.backup {
		fs::remove_dir_all(&cache_dir)?;
		if flags.verbose {
			eprintln!("removing directory ‘{}’", disp(&cache_dir));
		}
	}

	Ok(())
}

fn backup_srcs<'a, I>(flags: &Flags, cwd: &PathBuf, xs: I) -> Result<(), io::Error>
where
	I: Iterator<Item = &'a PathBuf>,
{
	for x in xs {
		let data = require!(fs::metadata(x));
		if data.is_dir() {
			let rel_x = require!(x.strip_prefix("/"));
			fs::create_dir_all(rel_x)?;
			if flags.verbose {
				eprintln!("created directory ‘{}/{}’", disp(cwd), rel_x.display());
			}
		} else {
			if let Some(p) = x.parent() {
				let rel_x = require!(p.strip_prefix("/"));
				fs::create_dir_all(rel_x)?;
				if flags.verbose {
					eprintln!("created directory ‘{}/{}’", disp(cwd), rel_x.display());
				}
			}
			let rel_x = require!(x.strip_prefix("/"));
			fs::copy(x, rel_x)?;
			if flags.verbose {
				eprintln!(
					"copied ‘{}’ -> ‘{}/{}’",
					disp(x),
					disp(cwd),
					rel_x.display()
				);
			}
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
		}

		let mut co = child.stdout.take().unwrap_or_else(|| {
			err!("Count not open the child process’ stdout.");
		});
		let mut s = String::with_capacity(src.len());
		co.read_to_string(&mut s)?;
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

	/* Pass the source files to the child process. */
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
			ci.write_all(if flags.nul && !flags.encode {
				&[b'\0']
			} else {
				&[b'\n']
			})?;
		}
	}

	/* Read the destination file list from the process. */
	let co = child.stdout.take().unwrap_or_else(|| {
		err!("Count not open the child process’ stdout.");
	});
	let groups = co
		.bytes()
		.map(|x| require!(x))
		.group_by(|b| is_terminal(flags.nul && !flags.encode, b));
	groups
		.into_iter()
		.filter_map(|(x, y)| match x {
			true => None,
			false => Some(y),
		})
		.for_each(|x| {
			let dst = require!(String::from_utf8(x.collect_vec()));
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
	if !flags.dryrun {
		copy_and_remove_file_or_dir(flags, &from, &to).unwrap_or_else(|(f, e)| {
			err!("{}: {e}", f.to_string_lossy());
		});
	}

	if flags.verbose {
		eprintln!(
			"{} ‘{}’ -> ‘{}’",
			if flags.mcp { "copied" } else { "renamed" },
			disp(&from),
			disp(&to)
		);
	}
}

fn copy_and_remove_file_or_dir<'a>(
	flags: &Flags,
	from: &'a PathBuf,
	to: &'a PathBuf,
) -> Result<(), (&'a PathBuf, io::Error)> {
	let data = fs::metadata(&from).map_err(|e| (from, e))?;
	if data.is_dir() {
		fs::create_dir(&to).map_err(|e| (to, e))?;
		if !flags.mcp {
			fs::remove_dir(&from).map_err(|e| (from, e))?
		}
	} else {
		fs::copy(&from, &to).map_err(|e| (to, e))?;
		if !flags.mcp {
			fs::remove_file(&from).map_err(|e| (from, e))?
		}
	}
	Ok(())
}

fn is_terminal(nul: bool, b: &u8) -> bool {
	*b == (b'\0' + b'\n' * !nul as u8)
}

fn disp(pb: &PathBuf) -> Display {
	pb.as_path().display()
}
