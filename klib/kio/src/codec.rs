pub trait Encoder<Item> {
    type Error: From<crate::Error>;

    fn encode<T>(&mut self, item: Item, dst: T) -> Result<usize, Self::Error>
    where
        T: AsMut<[u8]>;
}

pub trait Decoder {
    type Item;
    type Error: From<crate::Error>;

    fn decode(&mut self, src: &[u8]) -> Result<Option<Self::Item>, Self::Error>;
}

pub trait Sink<Item> {
    type Error;

    fn send(&mut self, item: Item) -> Result<(), Self::Error>;
}


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
    pub fn new(buffer: B, writer: W, encoder: E) -> Self { Self { buffer, writer, encoder } }
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

pub struct FramedRead {}

pub struct ChainedCodec<B, First, Second> {
    buffer: B,
    first: First,
    second: Second,
}

impl<B, First, Second> ChainedCodec<B, First, Second> {
    #[must_use]
    pub fn new(buffer: B, first: First, second: Second) -> Self { Self { buffer, first, second } }
}

impl<B, First, Second, Item> Encoder<Item> for ChainedCodec<B, First, Second> where
    B: AsMut<[u8]>,
    First: Encoder<Item>,
    Second: for<'a> Encoder<&'a [u8], Error = <First as Encoder<Item>>::Error>,
{
    type Error = <First as Encoder<Item>>::Error;

    fn encode<T>(&mut self, item: Item, dst: T) -> Result<usize, Self::Error>
    where
        T: AsMut<[u8]> {
        let n = self.first.encode(item, self.buffer.as_mut())?;
        self.second.encode(&self.buffer.as_mut()[..n], dst)
    }
}
