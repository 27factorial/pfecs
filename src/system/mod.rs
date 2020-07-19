use std::fmt;

use crate::storage::{ComponentStorageAllocator, ResourceStorageAllocator};

pub mod dispatch;
pub mod executor;

pub trait ResourceData<'a>
where
    Self: Sized + 'a,
{
    fn fetch(allocator: &'a ResourceStorageAllocator) -> Result<Self, RetrievalError>;
}

pub trait ComponentData<'a>
where
    Self: Sized + 'a,
{
    fn fetch(allocator: &'a ComponentStorageAllocator) -> Result<Self, RetrievalError>;
}

pub trait System<'a> {
    type Resources: ResourceData<'a>;
    type Components: ComponentData<'a>;

    fn execute(&mut self, _: Self::Resources, _: Self::Components);
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum RetrievalError {
    ResourceLockedExclusive,
    ResourceLockedShared,
    ResourceStorageInUse,
    NoSuchResourceStorage,
    ComponentLockedExclusive,
    ComponentLockedShared,
    ComponentStorageInUse,
    NoSuchComponentStorage,
}

impl fmt::Display for RetrievalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use RetrievalError::*;

        let msg = match *self {
            ResourceLockedExclusive => "The world resource allocator is currently locked (W).",
            ResourceLockedShared => "The world resource allocator is currently locked (R).",
            ResourceStorageInUse => "The requested resource storage is currently in use.",
            NoSuchResourceStorage => "No storage has been registered for the requested resource.",
            ComponentLockedExclusive => "The world component allocator is currently locked (W).",
            ComponentLockedShared => "The world component allocator is currently locked (R).",
            ComponentStorageInUse => "The requested component storage is currently in use.",
            NoSuchComponentStorage => "No storage has been registered for the requested component",
        };

        f.pad(msg)
    }
}

mod impls {
    use crate::{
        component::Component,
        resource::Resource,
        storage::{ReadComponent, ReadResource, WriteComponent, WriteResource},
    };

    use super::*;

    impl<'a, T: Resource> ResourceData<'a> for ReadResource<'a, T> {
        fn fetch(allocator: &'a ResourceStorageAllocator) -> Result<Self, RetrievalError> {
            if allocator.contains::<T>() {
                let storage = unsafe {
                    allocator
                        .try_get_unchecked::<T>()
                        .ok_or(RetrievalError::ResourceStorageInUse)?
                };
                Ok(ReadResource::new(storage))
            } else {
                Err(RetrievalError::NoSuchResourceStorage)
            }
        }
    }

    impl<'a, T: Resource> ResourceData<'a> for WriteResource<'a, T> {
        fn fetch(allocator: &'a ResourceStorageAllocator) -> Result<Self, RetrievalError> {
            if allocator.contains::<T>() {
                let storage = unsafe {
                    allocator
                        .try_get_mut_unchecked::<T>()
                        .ok_or(RetrievalError::ResourceStorageInUse)?
                };
                Ok(WriteResource::new(storage))
            } else {
                Err(RetrievalError::NoSuchResourceStorage)
            }
        }
    }

    impl<'a, T: Component> ComponentData<'a> for ReadComponent<'a, T> {
        fn fetch(allocator: &'a ComponentStorageAllocator) -> Result<Self, RetrievalError> {
            if allocator.contains::<T>() {
                let storage = unsafe {
                    allocator
                        .try_get_unchecked::<T>()
                        .ok_or(RetrievalError::ComponentStorageInUse)?
                };
                Ok(ReadComponent::new(storage))
            } else {
                Err(RetrievalError::NoSuchComponentStorage)
            }
        }
    }

    impl<'a, T: Component> ComponentData<'a> for WriteComponent<'a, T> {
        fn fetch(allocator: &'a ComponentStorageAllocator) -> Result<Self, RetrievalError> {
            if allocator.contains::<T>() {
                let storage = unsafe {
                    allocator
                        .try_get_mut_unchecked::<T>()
                        .ok_or(RetrievalError::ComponentStorageInUse)?
                };
                Ok(WriteComponent::new(storage))
            } else {
                Err(RetrievalError::NoSuchComponentStorage)
            }
        }
    }

    impl ResourceData<'_> for () {
        fn fetch(_: &ResourceStorageAllocator) -> Result<Self, RetrievalError> {
            Ok(())
        }
    }

    impl ComponentData<'_> for () {
        fn fetch(_: &ComponentStorageAllocator) -> Result<Self, RetrievalError> {
            Ok(())
        }
    }

    macro_rules! impl_rd {
        ($($t:tt),+) => {
            impl<'a, $($t),+> ResourceData<'a> for ($($t,)+)
            where
                $(
                    $t: ResourceData<'a>,
                )+
            {
                fn fetch(
                    allocator: &'a ResourceStorageAllocator
                ) -> Result<Self, RetrievalError> {
                    Ok(($(<$t as ResourceData<'_>>::fetch(allocator)?),*,))
                }
            }
        }
    }

    macro_rules! impl_cd {
        ($($t:tt),+) => {
            impl<'a, $($t),+> ComponentData<'a> for ($($t,)+)
            where
                $(
                    $t: ComponentData<'a>,
                )+
            {
                fn fetch(
                    allocator: &'a ComponentStorageAllocator
                ) -> Result<Self, RetrievalError> {
                    Ok(($(<$t as ComponentData<'_>>::fetch(allocator)?),*,))
                }
            }
        }
    }

    // ResourceData<'_> implementations
    impl_rd!(T0);
    impl_rd!(T0, T1);
    impl_rd!(T0, T1, T2);
    impl_rd!(T0, T1, T2, T3);
    impl_rd!(T0, T1, T2, T3, T4);
    impl_rd!(T0, T1, T2, T3, T4, T5);
    impl_rd!(T0, T1, T2, T3, T4, T5, T6);
    impl_rd!(T0, T1, T2, T3, T4, T5, T6, T7);
    impl_rd!(T0, T1, T2, T3, T4, T5, T6, T7, T8);
    impl_rd!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9);
    impl_rd!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
    impl_rd!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
    impl_rd!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);

    // ComponentData<'_> implementations
    impl_cd!(T0);
    impl_cd!(T0, T1);
    impl_cd!(T0, T1, T2);
    impl_cd!(T0, T1, T2, T3);
    impl_cd!(T0, T1, T2, T3, T4);
    impl_cd!(T0, T1, T2, T3, T4, T5);
    impl_cd!(T0, T1, T2, T3, T4, T5, T6);
    impl_cd!(T0, T1, T2, T3, T4, T5, T6, T7);
    impl_cd!(T0, T1, T2, T3, T4, T5, T6, T7, T8);
    impl_cd!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9);
    impl_cd!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
    impl_cd!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
    impl_cd!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
}
