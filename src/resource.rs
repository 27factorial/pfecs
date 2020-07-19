use crate::storage::{ResourceStorage, ResourceStorageAllocator};

pub trait Resource: Send + Sync + 'static {}

impl<T: Send + Sync + 'static> Resource for T {}

pub trait ResourceTuple: self::sealed::ResourceTupleSealed + 'static {
    fn store(self, allocator: &mut ResourceStorageAllocator);
}

pub trait IntoResourceTuple<U> {
    fn into(self) -> U;
}

impl<T: Resource> IntoResourceTuple<(T,)> for T {
    fn into(self) -> (T,) {
        (self,)
    }
}

impl<CT: ResourceTuple> IntoResourceTuple<Self> for CT {
    fn into(self) -> Self {
        self
    }
}

mod sealed {
    use super::*;

    pub trait ResourceTupleSealed {}

    // Implemented separately for unit, since it has different
    // behavior. Unit signifies no components are associated with
    // the entity, and it should not be stored as a component.
    impl ResourceTupleSealed for () {}

    impl ResourceTuple for () {
        fn store(self, _: &mut ResourceStorageAllocator) {}
    }

    #[allow(unused)]
    macro_rules! impl_rt {
        ($t:tt) => {
            impl<$t> ResourceTupleSealed for ($t,)
            where
                $t: Resource,
            {}
            impl<$t> ResourceTuple for ($t,)
            where
                $t: Resource,
            {
                fn store(
                    self,
                    allocator: &mut ResourceStorageAllocator
                ) {
                    #[allow(non_snake_case)]
                    let ($t,) = self;
                    if !allocator.register($t) {
                        panic!(
                            "Storage of type {} was already registered\
                             with the provided allocator!",
                            std::any::type_name::<ResourceStorage<$t>>(),
                        )
                    }
                }
            }
        };
        ($($t:tt),+) => {
            impl<$($t),+> ResourceTupleSealed for ($($t,)+)
            where
                $(
                    $t: Resource,
                )+
            {}

            impl<$($t),+> ResourceTuple for ($($t,)+)
            where
                $(
                    $t: Resource,
                )+
            {
                fn store(
                    self,
                    allocator: &mut ResourceStorageAllocator,
                ) {
                   #[allow(non_snake_case)]
                    let ($($t,)+) = self;


                    $(
                        if !allocator.register($t) {
                            panic!(
                                "Storage of type {} was already registered\
                                 with the provided allocator!",
                                std::any::type_name::<ResourceStorage<$t>>(),
                            )
                        }
                    )+
                }
            }
        };
        ($($t:tt),+; $ct:tt) => {
           impl<$($t),+, $ct> ResourceTupleSealed for ($($t,)+ $ct)
            where
                $(
                    $t: Resource,
                )+

                $ct: ResourceTuple
            {}

            impl<$($t),+, $ct> ResourceTuple for ($($t,)+ $ct)
            where
                $(
                    $t: Resource,
                )+

                $ct: ResourceTuple,
            {
                fn store(
                    self,
                    allocator: &mut ResourceStorageAllocator
                ) {
                   #[allow(non_snake_case)]
                    let ($($t,)+ $ct) = self;
                    <$ct as ResourceTuple>::store($ct, allocator);


                    $(
                        if !allocator.register($t) {
                            panic!(
                                "Storage of type {} was already registered\
                                 with the provided allocator!",
                                std::any::type_name::<ResourceStorage<$t>>(),
                            )
                        }
                    )+
                }
            }
        };
    }

    // First, impl up to a 12-tuple.
    impl_rt!(T0);
    impl_rt!(T0, T1);
    impl_rt!(T0, T1, T2);
    impl_rt!(T0, T1, T2, T3);
    impl_rt!(T0, T1, T2, T3, T4);
    impl_rt!(T0, T1, T2, T3, T4, T5);
    impl_rt!(T0, T1, T2, T3, T4, T5, T6);
    impl_rt!(T0, T1, T2, T3, T4, T5, T6, T7);
    impl_rt!(T0, T1, T2, T3, T4, T5, T6, T7, T8);
    impl_rt!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9);
    impl_rt!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
    impl_rt!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);

    // Then, recursively impl to allow
    // nested tuples of components.
    impl_rt!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11; CT);
}
