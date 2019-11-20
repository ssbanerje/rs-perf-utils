use crate::Result;

/// Registry of available counters.
pub trait Registry<V> {
    type CT: Counter<V>;
    type SCT: SampledCounter<V>;

    /// Query registry by name to identify counters.
    fn query(&self, name: &str) -> Result<Vec<Self::CT>>;

    /// Query registry by name to identify sampled counters.
    fn query_sampled(&self, name: &str) -> Result<Vec<Self::SCT>> {
        let mut q = self.query(name)?;
        let mut res = Vec::with_capacity(q.len());
        for _ in 0..q.len() {
            res.push(q.pop().unwrap().into_sampled()?);
        }
        Ok(res)
    }
}

/// A generic synchronous access performance counter.
pub trait Counter<V> {
    /// Name of the performance counter.
    fn name(&self) -> &String;

    /// Enable the counter.
    fn enable(&self) -> Result<()>;

    /// Disable the counter.
    fn disable(&self) -> Result<()>;

    /// Reset the counter.
    fn reset(&self) -> Result<()>;

    /// Read the latest value of the counter.
    fn read_sync(&self) -> Result<V>;

    /// Convert this `Counter` into a `SampledCounter`.
    fn into_sampled<R, It>(self) -> Result<R>
    where
        R: SampledCounter<V, Iter = It>,
        It: Iterator<Item = V>;
}

/// A generic sampled performance counter.
pub trait SampledCounter<V>
where
    Self: Counter<V>,
{
    /// Type of the sample iterator.
    type Iter: Iterator<Item = V>;

    /// Get an iterator over the unread samples in the performance counter.
    fn iter(&self) -> Self::Iter;

    /// Checks if there are sampled items waiting to be read.
    fn unread_events(&self) -> bool;

    /// Mark `num_items` samples as read.
    fn advance(&self, num_items: usize);
}

/// A generic sampled performance counter that can be directly read without OS assistance.
pub trait HardwareCounter<V>
where
    Self: SampledCounter<V>,
{
    /// Read the latest value of the counter.
    fn read_direct(&self) -> Result<V>;
}
