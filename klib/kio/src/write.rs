use crate::codec::Encoder;

pub trait Write {
    /// # Errors
    ///
    /// This should error if a byte can't be written
    fn write(&mut self, buffer: &[u8]) -> crate::Result<usize>;

    /// # Errors
    ///
    /// Return the first error of [`Write::write`](Write::write)
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

pub trait Sink<Item> {
    type Error;

    /// # Errors
    ///
    /// Error if the item was not able to be sent
    fn send(&mut self, item: Item) -> Result<(), Self::Error>;
}

#[allow(clippy::module_name_repetitions)]
pub struct FramedWrite<B, W, E>
where
    B: AsMut<[u8]>,
{
    buffer: B,
    writer: W,
    encoder: E,
}

impl<B, W, E> FramedWrite<B, W, E>
where
    B: AsMut<[u8]>,
{
    #[must_use]
    pub fn new(buffer: B, writer: W, encoder: E) -> Self {
        Self {
            buffer,
            writer,
            encoder,
        }
    }
}

impl<B, W, E, Item> Sink<Item> for FramedWrite<B, W, E>
where
    B: AsMut<[u8]>,
    W: crate::write::Write,
    E: Encoder<Item>,
{
    type Error = <E as Encoder<Item>>::Error;

    fn send(&mut self, item: Item) -> Result<(), Self::Error> {
        let n = self.encoder.encode(item, self.buffer.as_mut())?;
        self.writer.write_all(&self.buffer.as_mut()[..n])?;
        Ok(())
    }
}
