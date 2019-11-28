//! Defines core API interfaces used by this crate to access performance counters.

use crate::Result;

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

/// Defines a measured value that can be scaled to correct for measurement errors.
pub trait ScaledValue<V> {
    /// Get the raw measurement value.
    fn raw_value(&self) -> V;

    /// Get the scaled measurement value.
    fn scaled_value(&self) -> V;
}

/// Trait allowing access to the basic metadata of all events.
pub trait BaseEvent {
    /// Get the name of the event.
    fn name(&self) -> &str;

    /// Get the topic containing the event.
    fn topic(&self) -> &str;

    /// Get a description of the event.
    fn desc(&self) -> &str;
}

/// An event that can be programmed into a performance counter.
pub trait Event<V, C>
where
    Self: BaseEvent,
    C: Counter<V>,
{
    /// Create a schedulable counter from this event.
    fn get_counter(&self) -> Result<C>;
}

/// A group of events that must be measured together.
pub trait EventGroup<V, C>
where
    Self: BaseEvent,
    C: Counter<V>,
{
    /// Create a set of schedulable counters from this event.
    fn get_counters(&self) -> Vec<C>;

    /// Aggregate the set of values of the counters to calculate the aggregated value of this group.
    fn aggregate(&self, vals: Vec<V>) -> Result<V>;
}

/// Registry of available counters.
pub trait EventRegistry<V, E, C>
where
    E: Event<V, C>,
    C: Counter<V>,
{
    /// Query registry to identify counters.
    fn query(&self, predicate: impl FnMut(&E) -> bool) -> Result<Vec<E>>;

    /// Query registry by name of event to identify counters.
    fn query_name(&self, name: &str) -> Result<Option<E>> {
        let mut qres = self.query(|e: &E| e.name() == name)?;
        Ok(qres.pop())
    }
}
