use std::future::Future;

use bollard::Docker;

use crate::docker;

pub trait Inspectable: Sized + Send + Sync {
    type Model: Send + Sync;
    type Ref;

    fn inspect(
        api: &Docker,
        args: Self::Ref,
    ) -> impl Future<Output = docker::Result<Inspected<Self>>>;
}

pub enum Inspected<T: Inspectable> {
    /// Represents a valid Docker resource which can be reused.
    Some(T),
    /// Represents an invalid Docker resource which was not create properly or
    /// cannot be reused, and shall be deleted.
    Invalid(Box<T::Model>),
    /// No resource was found.
    None,
}

pub use volume::{VolumeName, Volume};
pub use container::{Container, ContainerName};
pub use bind_mount::BindMount;

pub mod volume;
pub mod container;
pub mod bind_mount;
pub mod network;
