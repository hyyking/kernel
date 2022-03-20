pub struct Cursor<'a> {
    data: &'a mut [u8],
    cursor: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self { data, cursor: 0 }
    }

    pub fn buffer(&self) -> &[u8] {
        &self.data[..self.cursor]
    }
}

impl<'a> core::fmt::Write for Cursor<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let prev = self.cursor;
        self.cursor += s.as_bytes().len();

        if self.cursor > self.data.len() {
            Err(core::fmt::Error)
        } else {
            self.data[prev..self.cursor].copy_from_slice(s.as_bytes());
            Ok(())
        }
    }
}
