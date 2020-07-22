#![deny(missing_debug_implementations)]

pub use archetype::Archetype;
pub use component::{Component, ComponentSet, ComponentTuple};
pub use entity::Entity;
pub use resource::{Resource, ResourceTuple};
pub use storage::{ReadComponent, ReadResource, WriteComponent, WriteResource};
pub use system::{dispatch, System};
pub use world::{query, World};

pub mod archetype;
pub mod cell;
pub mod component;
pub mod entity;
pub mod resource;
pub mod storage;
pub mod system;
pub mod utils;
pub mod world;
