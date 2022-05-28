pub trait Encoder<Item> {
    type Error: From<crate::Error>;

    /// # Errors
    ///
    /// Serialization errors
    fn encode<T>(&mut self, item: Item, dst: T) -> Result<usize, Self::Error>
    where
        T: AsMut<[u8]>;

    fn chain<B, C>(self, buffer: B, other: C) -> Chained<B, Self, C>
    where
        Self: Sized,
        B: AsMut<[u8]>,
        C: for<'a> Encoder<&'a [u8], Error = <Self as Encoder<Item>>::Error>,
    {
        Chained::new(buffer, self, other)
    }
}

pub trait Decoder {
    type Item;
    type Error: From<crate::Error>;

    /// # Errors
    ///
    /// Deserialization errors
    fn decode(&mut self, src: &[u8]) -> Result<Option<Self::Item>, Self::Error>;
}

pub struct Chained<B, First, Second> {
    buffer: B,
    first: First,
    second: Second,
}

impl<B, First, Second> Chained<B, First, Second> {
    #[must_use]
    pub fn new(buffer: B, first: First, second: Second) -> Self {
        Self {
            buffer,
            first,
            second,
        }
    }
}

impl<B, First, Second, Item> Encoder<Item> for Chained<B, First, Second>
where
    B: AsMut<[u8]>,
    First: Encoder<Item>,
    Second: for<'a> Encoder<&'a [u8], Error = <First as Encoder<Item>>::Error>,
{
    type Error = <First as Encoder<Item>>::Error;

    fn encode<T>(&mut self, item: Item, dst: T) -> Result<usize, Self::Error>
    where
        T: AsMut<[u8]>,
    {
        let n = self.first.encode(item, self.buffer.as_mut())?;
        self.second.encode(&self.buffer.as_mut()[..n], dst)
    }
}
