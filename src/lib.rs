//! # `mmv`: batched file moves
//! 
//! This library provides functionality for moving many filesystem paths simultaneously.  It handles
//! overlapping source/destination paths gracefully, but is specifically dedicated to moving paths:
//! it does not support copying a single path to multiple destinations.
//! 
//! The `mmv` program is the main user of this library, and the API provided here is tailored to its
//! use.
//! 
//! ---
//! 
//! Copyright 2022 The Depressed Milkman

use std::borrow::Borrow;
use std::cell::Cell;
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::iter::Extend;
use std::mem::ManuallyDrop;
use std::num::NonZeroUsize;
use std::path::{Component, Path};

/// A batched move.
/// 
/// An instance of this structure represents a command to simultaneously move a set of source paths
/// to a set of corresponding destination paths.  After the move finishes, each destination path
/// will correspond to the source path as it was before the move began.  This means, for example,
/// that if two paths are being moved to each other, a temporary file will have to be used to move
/// between them, so that they are swapped without accidentally overwriting one with the other.
/// 
/// At the moment, the limitations of the [`std::path`] API mean that it is not possible to store
/// the heap-allocated memory for the paths used by this structure within it; they can only be used
/// by reference here, and so have to be stored by the user.  This limitation may be lifted in the
/// future by a rewritten implementation.
pub struct Move<'a> {
	/// A list of nodes.
	data: Cell<Option<Vec<Node<'a>>>>,
	/// The index of the next free node.
	free: Cell<Option<NonZeroUsize>>,
}

/// A node in a [`Move`].
union Node<'a> {
	/// A full node.
	full: ManuallyDrop<FullNode<'a>>,
	/// A free node.
	free: FreeNode,
}

/// A full node in a [`Move`].
#[derive(Default)]
struct FullNode<'a> {
	/// Information about the node.
	info: NodeInfo<'a>,
	/// The children of this node.
	kids: HashMap<Component<'a>, usize>,
}

/// A free node in a [`Move`].
#[derive(Default, Copy, Clone)]
struct FreeNode {
	/// The index of the next free node.
	next: Option<NonZeroUsize>,
}

/// Information about a path involved in a batched move.
/// 
/// An instance of this structure is maintained for every path involved in a batched move.	It keeps
/// track of move-related information about the path, such as its status as a source and/or as a
/// destination path.
struct NodeInfo<'a> {
	/// The current path.
	cur: &'a Path,

	/// If this is a source path: its destination.
	dst: Option<NonZeroUsize>,

	/// If this is a destination path: its source.
	src: Option<NonZeroUsize>,
}

/// An error arising from constructing a [`Move`].
pub struct ConsError<P: Borrow<Path>> {
	/// A list of duplicate sources.
	dup_srcs: Vec<P>,

	/// A list of duplicate destinations.
	dup_dsts: Vec<P>,
}

/// An error arising in [`Move::add`].
pub enum AddError<P: Borrow<Path>> {
	/// A single path is being used as a source for two moves.
	DupSrc { src: P, new: P, old: P },

	/// A single path is being used as a destination for two moves.
	DupDst { dst: P, new: P, old: P },
}

impl<'a> Move<'a> {
	/// Construct a new, empty [`Move`].
	pub fn new() -> Self {
		Self {
			data: Cell::new(Some(Vec::new())),
			free: Cell::new(None),
		}
	}

	/// Add the given source-destination pair to the set.
	/// 
	/// Note that this only requires a shared reference to the set, thereby allowing it to be added
	/// to from multiple sources concurrently.
	pub fn add(&self, src: &'a Path, dst: &'a Path) -> Result<(), AddError<&'a Path>> {
		let mut data = self.data.take().unwrap();
		let mut free = self.free.take();
		if data.is_empty() {
			data.push(Node { full: Default::default() });
		}

		// Lookup src and dst (creating nodes as necessary).
		let src_node = Self::get(&mut data, &mut free, src);
		let dst_node = Self::get(&mut data, &mut free, dst);

		// Link src to dst.
		let src_dst = &mut unsafe { &mut *data.get_unchecked_mut(src_node).full }.info.dst;
		if let Some(old) = src_dst.as_ref().copied() {
			return Err(AddError::DupSrc { src, new: dst,
				old: unsafe { &*data.get_unchecked(old.get()).full }.info.cur });
		} else {
			*src_dst = Some(NonZeroUsize::new(dst_node).unwrap());
		}

		// Link dst to src.
		let dst_src = &mut unsafe { &mut *data.get_unchecked_mut(dst_node).full }.info.src;
		if let Some(old) = dst_src.as_ref().copied() {
			return Err(AddError::DupDst { dst, new: src,
				old: unsafe { &*data.get_unchecked(old.get()).full }.info.cur });
		} else {
			*dst_src = Some(NonZeroUsize::new(src_node).unwrap());
		}

		// Return the updated fields to the structure.
		self.free.set(free);
		self.data.set(Some(data));

		Ok(())
	}

	/// Get the node for the given path, creating it if it does not exist.
	/// 
	/// NOTE: If a Path::iter_prefixes() method is ever introduced, or if path::Ancestors is made a
	///		  double-ended iteratior, the implementation here can be improved significantly.
	fn get(data: &mut Vec<Node<'a>>, free: &mut Option<NonZeroUsize>, path: &'a Path) -> usize {
		let (mut root, mut leaf) = (0, 0);

		// Find the last existing node along the path.
		let mut parts = path.components();
		while let Some(part) = parts.next() {
			let node = unsafe { &mut *data.get_unchecked_mut(root).full };
			if let Some(next) = node.kids.get_mut(&part).copied() {
				leaf = next; root = next;
			} else { break }
		}

		// Construct new nodes for the remainder of the path.
		let mut prev = None;
		for (path, part) in path.ancestors().zip(parts.rev()) {
			// Retrieve a free node.
			let (node, idx) = if let Some(idx) = free.as_ref().copied() {
				let node = unsafe { data.get_unchecked_mut(idx.get()) };
				*free = unsafe { node.free.next };
				(node, idx.get())
			} else {
				let idx = data.len();
				data.push(Node { free: FreeNode { next: None }});
				(unsafe { data.get_unchecked_mut(idx) }, idx)
			};

			// Fill the free node.
			let first = prev.is_none();
			node.full = ManuallyDrop::new(FullNode {
				info: NodeInfo { cur: path, dst: None, src: None },
				kids: prev.into_iter().collect(),
			});
			if first { leaf = idx; }
			prev = Some((part, idx));
		}

		// Link the constructed nodes to the last existing node.
		if let Some((part, idx)) = prev {
			unsafe { &mut *data.get_unchecked_mut(root).full }.kids.insert(part, idx);
		}

		// Return the location of the leaf node.
		leaf
	}
}

impl<'a> Default for NodeInfo<'a> {
	fn default() -> Self {
		Self { cur: Path::new(""), dst: None, src: None }
	}
}

impl<P: Borrow<Path>> AddError<P> {
	/// Map all contained paths to a different path type.
	pub fn map_paths<N: Borrow<Path>, F: FnMut(P) -> N>(self, mut f: F) -> AddError<N> {
		match self {
			Self::DupSrc { src, new, old } =>
				AddError::DupSrc { src: f(src), new: f(new), old: f(old) },
			Self::DupDst { dst, new, old } =>
				AddError::DupDst { dst: f(dst), new: f(new), old: f(old) },
		}
	}
}

impl<P: Borrow<Path>> ConsError<P> {
	/// Whether this is really an error or not.
	fn is_err(&self) -> bool {
		!self.dup_srcs.is_empty() || !self.dup_dsts.is_empty()
	}

	/// Attempt to construct a [`ConsError`] from the given iterator.
	pub fn from_iter<Iter: IntoIterator<Item = AddError<P>>>(iter: Iter) -> Result<(), Self> {
		let mut this = Self { dup_srcs: Vec::new(), dup_dsts: Vec::new() };
		this.extend(iter);
		if this.is_err() { Err(this) } else { Ok(()) }
	}
}

impl<P: Borrow<Path>> Display for ConsError<P> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let mut first = true;

		if !self.dup_srcs.is_empty() {
			if first {
				write!(f, "The paths ")?;
			} else {
				write!(f, "and the paths ")?;
			}

			self.dup_srcs.iter().enumerate().try_for_each(|(i, ds)| {
				if i == 0 {
					write!(f, "[{}", ds.borrow().display())
				} else {
					write!(f, ", {}", ds.borrow().display())
				}
			})?;
			write!(f, "] were listed as sources multiple times")?;
			first = false;
		}

		if !self.dup_dsts.is_empty() {
			if first {
				write!(f, "The paths ")?;
			} else {
				write!(f, "and the paths ")?;
			}

			self.dup_dsts.iter().enumerate().try_for_each(|(i, dd)| {
				if i == 0 {
					write!(f, "[{}", dd.borrow().display())
				} else {
					write!(f, ", {}", dd.borrow().display())
				}
			})?;
			write!(f, "] were listed as destinations multiple times")?;
			first = false;
		}

		Ok(())
	}
}

impl<P: Borrow<Path>> Extend<AddError<P>> for ConsError<P> {
	fn extend<Iter: IntoIterator<Item = AddError<P>>>(&mut self, iter: Iter) {
		iter.into_iter().for_each(|err| match err {
			AddError::DupSrc { src, .. } => self.dup_srcs.push(src),
			AddError::DupDst { dst, .. } => self.dup_dsts.push(dst),
		});
	}
}
