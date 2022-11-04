use std::str;

pub struct EncodedString<'a> {
    pub s: str::Bytes<'a>,
}

impl<'a> Iterator for EncodedString<'a> {
	type Item = u8;

	fn next(&mut self) -> Option<Self::Item> {
		self.s.next().map(|c| match c {
			b'\\' => match self.s.next() {
				Some(b'n') => b'\n',
				Some(b'\\') | None => b'\\',
				Some(c) => c
			},
			c => c
		})
	}
}

impl<'a> EncodedString<'a> {
	pub fn decode(self) -> String {
		String::from_utf8(self.s.collect()).unwrap()
	}
}
