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

/// An event that can be programmed into a performance counter.
pub trait Event<V> {
    /// Type of performance counter that can be created from this event.
    type Ctr: Counter<V>;

    /// Create a schedulable counter from this event.
    fn get_counter(&self) -> Self::Ctr;
}

/// A group of events that must be measured together to c
pub trait EventGroup<V> {
    /// Types of individual events in this group.
    type Evt: Event<V>;

    /// Create a set of schedulable counters from this event.
    fn get_counters(&self) -> Vec<<Self::Evt as Event<V>>::Ctr>;

    /// Aggregate the set of values of the counters to calculate the aggregated value of this group.
    fn aggregate(&self, vals: Vec<V>) -> Result<V>;
}

/// Registry of available counters.
pub trait EventRegistry<V> {
    /// Type of directly polled performance counters for the `Registry`.
    type Evt: Event<V>;

    /// Query registry by name to identify counters.
    fn query(&self, predicate: impl FnMut(&Self::Evt) -> bool) -> Result<Vec<Self::Evt>>;
}
