#[derive(Debug)]
pub struct Flags {
	pub encode: bool,
	pub individual: bool,
	pub nul: bool
}

impl Default for Flags {
	fn default() -> Flags {
		Flags {
			encode: false,
			individual: false,
			nul: false
		}
	}
}
