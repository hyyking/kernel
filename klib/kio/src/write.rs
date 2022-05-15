pub trait Write {
    fn write(&mut self, buffer: &[u8]) -> crate::Result<usize>;

    fn write_all(&mut self, mut buffer: &[u8]) -> crate::Result<()> {
        while !buffer.is_empty() {
            match self.write(buffer) {
                Ok(0) => unimplemented!("zero write"),
                Ok(n) => buffer = &buffer[n..],
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}
