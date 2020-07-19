use std::{
    any::{self, TypeId},
    collections::HashSet,
};

use parking_lot::{RwLockUpgradableReadGuard, RwLockWriteGuard};

use crate::{entity::Entity, storage::ComponentStorageAllocator};

pub trait Component: Send + Sync + 'static {}

impl<T: Send + Sync + 'static> Component for T {}

pub trait ComponentTuple: self::sealed::ComponentTupleSealed + 'static {
    fn set() -> HashSet<TypeId>;
    fn store(self, entity: Entity, allocator: &mut ComponentStorageAllocator);
}

pub trait IntoComponentTuple<U> {
    fn into(self) -> U;
}

impl<T: Component> IntoComponentTuple<(T,)> for T {
    fn into(self) -> (T,) {
        (self,)
    }
}

impl<CT: ComponentTuple> IntoComponentTuple<Self> for CT {
    fn into(self) -> Self {
        self
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ComponentSet {
    ids: HashSet<TypeId>,
}

impl ComponentSet {
    pub fn new(ids: HashSet<TypeId>) -> Self {
        Self { ids }
    }

    pub fn from_tuple<T: ComponentTuple>() -> Self {
        Self { ids: T::set() }
    }

    pub fn set(&self) -> &HashSet<TypeId> {
        &self.ids
    }

    pub fn into_inner(self) -> HashSet<TypeId> {
        self.ids
    }
}

mod sealed {
    use super::*;

    pub trait ComponentTupleSealed {}

    // Implemented separately for unit, since it has different
    // behavior. Unit signifies no components are associated with
    // the entity, and it should not be stored as a component.
    impl ComponentTupleSealed for () {}

    impl ComponentTuple for () {
        fn set() -> HashSet<TypeId> {
            HashSet::new()
        }

        fn store(self, _: Entity, _: &mut ComponentStorageAllocator) {}
    }

    macro_rules! impl_ct {
        ($t:tt) => {
            impl<$t> ComponentTupleSealed for ($t,)
            where
                $t: Component,
            {}
            impl<$t> ComponentTuple for ($t,)
            where
                $t: Component,
            {
                fn set() -> HashSet<TypeId> {
                    let mut set = HashSet::with_capacity(1);
                    set.insert(TypeId::of::<$t>());
                    set
                }

                fn store(
                    self,
                    entity: Entity,
                    allocator: &mut ComponentStorageAllocator
                ) {
                    #[allow(non_snake_case)]
                    let ($t,) = self;

                    allocator
                        .get_mut_or_register::<$t>()
                        .push(entity.id(), $t)
                        .unwrap_or_else(|_| {
                            panic!(
                                "Entity {} already contained component of type {}",
                                entity.id(),
                                any::type_name::<$t>(),
                            )
                        });
                }
            }
        };
        ($($t:tt),+; $n:expr) => {
            impl<$($t),+> ComponentTupleSealed for ($($t,)+)
            where
                $(
                    $t: Component,
                )+
            {}

            impl<$($t),+> ComponentTuple for ($($t,)+)
            where
                $(
                    $t: Component,
                )+
            {
                fn set() -> HashSet<TypeId> {
                    let mut set = HashSet::with_capacity($n);
                    $(
                        if !set.insert(TypeId::of::<$t>()) {
                            panic!(
                                "Component set already contained component of type {}",
                                any::type_name::<$t>(),
                            );
                        };
                    )+
                    set
                }

                fn store(
                    self,
                    entity: Entity,
                    allocator: &mut ComponentStorageAllocator
                ) {
                    #[allow(non_snake_case)]
                    let ($($t,)+) = self;

                    $(
                        allocator
                            .get_mut_or_register::<$t>()
                            .push(entity.id(), $t)
                            .unwrap_or_else(|_| {
                                panic!(
                                    "Entity {} already contained component of type {}",
                                    entity.id(),
                                    any::type_name::<$t>(),
                                )
                            });
                    )+
                }
            }
        };
        ($($t:tt),+; $ct:tt; $n:expr) => {
           impl<$($t),+, $ct> ComponentTupleSealed for ($($t,)+ $ct)
            where
                $(
                    $t: Component,
                )+

                $ct: ComponentTuple
            {}
            impl<$($t),+, $ct> ComponentTuple for ($($t,)+ $ct)
            where
                $(
                    $t: Component,
                )+

                $ct: ComponentTuple,
            {
                fn set() -> HashSet<TypeId> {
                    let mut set = <$ct as ComponentTuple>::set();
                    set.reserve($n);

                    $(
                        if !set.insert(TypeId::of::<$t>()) {
                            panic!(
                                "Component set already contained component of type {}",
                                any::type_name::<$t>(),
                            );
                        }
                    )+

                    set
                }

                fn store(
                    self,
                    entity: Entity,
                    allocator: &mut ComponentStorageAllocator,
                ) {
                    #[allow(non_snake_case)]
                    let ($($t,)+ $ct) = self;

                    $(
                        allocator
                            .get_mut_or_register::<$t>()
                            .push(entity.id(), $t)
                            .unwrap_or_else(|_| {
                                panic!(
                                    "Entity {} already contained component of type {}",
                                    entity.id(),
                                    any::type_name::<$t>(),
                                )
                            });
                    )+


                    <$ct as ComponentTuple>::store($ct, entity, allocator)
                }
            }
        };
    }

    // First, impl up to a 12-tuple.
    impl_ct!(T0);
    impl_ct!(T0, T1; 2usize);
    impl_ct!(T0, T1, T2; 3usize);
    impl_ct!(T0, T1, T2, T3; 4usize);
    impl_ct!(T0, T1, T2, T3, T4; 5usize);
    impl_ct!(T0, T1, T2, T3, T4, T5; 6usize);
    impl_ct!(T0, T1, T2, T3, T4, T5, T6; 7usize);
    impl_ct!(T0, T1, T2, T3, T4, T5, T6, T7; 8usize);
    impl_ct!(T0, T1, T2, T3, T4, T5, T6, T7, T8; 9usize);
    impl_ct!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9; 10usize);
    impl_ct!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10; 11usize);
    impl_ct!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11; 12usize);

    // Then, recursively impl to allow
    // nested tuples of components.
    impl_ct!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11; CT; 12usize);
}
