pub use component::{
    ComponentStorage, ComponentStorageAllocator, Read as ReadComponent, Write as WriteComponent,
};
pub use resource::{
    Read as ReadResource, ResourceStorage, ResourceStorageAllocator, Write as WriteResource,
};

mod component;
mod resource;
