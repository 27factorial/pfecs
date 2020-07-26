use crate::entity::EntityId;

fn intersection(vecs: &[&[EntityId]]) -> Vec<EntityId> {
    let capacity: usize = vecs.iter().map(|vec| vec.len()).sum();
    let mut intersect = Vec::with_capacity(capacity);

    vecs.iter().flat_map(|vec| vec.iter()).for_each(|id| {
        if vecs.iter().all(|vec| vec.contains(id)) {
            intersect.push(*id)
        }
    });

    // Ensure that there is only one of each EntityId.
    intersect.sort_unstable();
    intersect.dedup();

    intersect
}

pub trait Join: sealed::StorageTuple
where
    Self: Sized,
{
    fn join(self) -> JoinIter<Self>;
}

#[derive(Debug)]
pub struct JoinIter<ST: Join> {
    tuple: ST,
    ids: Vec<EntityId>,
    current: usize,
}

mod sealed {
    use crate::storage::ComponentStorage;
    use crate::Component;

    use super::*;

    pub trait StorageTuple {}

    pub trait StoragePriv {
        type Item;

        fn ids(&self) -> &[EntityId];
        unsafe fn get_item(&mut self, id: EntityId) -> Self::Item;
    }

    impl<'a, T0: Component + Send + Sync> StoragePriv for &'a ComponentStorage<T0> {
        type Item = &'a T0;

        fn ids(&self) -> &[EntityId] {
            &self.entities()
        }

        unsafe fn get_item(&mut self, id: EntityId) -> Self::Item {
            let index = self
                .ids()
                .iter()
                .copied()
                .enumerate()
                .find(|(_, other_id)| id == *other_id)
                .map(|(index, _)| index)
                .unwrap();

            self.components().get_unchecked(index)
        }
    }

    impl<'a, T0: Component + Send + Sync> StoragePriv for &'a mut ComponentStorage<T0> {
        type Item = &'a mut T0;

        fn ids(&self) -> &[EntityId] {
            &self.entities()
        }

        unsafe fn get_item(&mut self, id: EntityId) -> Self::Item {
            let index = self
                .ids()
                .iter()
                .copied()
                .enumerate()
                .find(|(_, other_id)| id == *other_id)
                .map(|(index, _)| index)
                .unwrap();

            // SAFETY: Since `self` is borrowed mutably, there can be
            // no overlapping mutable references to the same data.
            // This reborrow is required since `Self::Item` has no
            // lifetime relationship to `self`. This defines the
            // relationship, and statically assures that there
            // can be no other mutable borrows to `self`.
            &mut *(self.components_mut().get_unchecked_mut(index) as *mut _)
        }
    }

    macro_rules! impl_join {
        ($t:tt$(,)?) => {
            impl<$t> StorageTuple for ($t,)
            where
                $t: StoragePriv,
            {}

            impl<$t> Join for ($t,)
            where
                $t: StoragePriv,
            {
                fn join(self) -> JoinIter<Self> {
                    #[allow(non_snake_case)]
                    let (tuple, ids) = {
                        let ($t,) = self;
                        let ids = intersection(&[&$t.ids()]);
                        (($t,), ids)
                    };

                    JoinIter {
                        tuple,
                        ids,
                        current: 0,
                    }
                }
            }

            impl<$t> Iterator for JoinIter<($t,)>
            where
                $t: StoragePriv,
            {
                type Item = (<$t as StoragePriv>::Item,);

                fn next(&mut self) -> Option<Self::Item> {
                    if self.current < self.ids.len() {
                        #[allow(non_snake_case)]
                        let ($t,) = &mut self.tuple;

                        let tuple = unsafe {
                            let id = *self.ids.get_unchecked(self.current);
                            (StoragePriv::get_item($t, id),)
                        };
                        self.current += 1;

                        Some(tuple)
                    } else {
                        None
                    }
                }
            }
        };
        ($($t:tt),+$(,)?) => {
            impl<$($t),+> StorageTuple for ($($t),+)
            where
                $(
                    $t: StoragePriv,
                )+
            {}

            impl<$($t),+> Join for ($($t),+)
            where
                $(
                    $t: StoragePriv,
                )+
            {
                fn join(self) -> JoinIter<Self> {
                    #[allow(non_snake_case)]
                    let (tuple, ids) = {
                        let ($($t),+) = self;
                        let ids = intersection(&[$(&$t.ids()),+]);
                        (($($t),+), ids)
                    };

                    JoinIter {
                        tuple,
                        ids,
                        current: 0,
                    }
                }
            }

            impl<$($t),+> Iterator for JoinIter<($($t),+)>
            where
                $(
                    $t: StoragePriv,
                )+
            {
                type Item = ($(<$t as StoragePriv>::Item),+);

                fn next(&mut self) -> Option<Self::Item> {
                    if self.current < self.ids.len() {
                        #[allow(non_snake_case)]
                        let ($($t),+) = &mut self.tuple;

                        let tuple = unsafe {
                            let id = *self.ids.get_unchecked(self.current);
                            ($(StoragePriv::get_item($t, id)),+)
                        };
                        self.current += 1;

                        Some(tuple)
                    } else {
                        None
                    }
                }
            }
        };
    }

    impl_join!(T0);
    impl_join!(T0, T1);
    impl_join!(T0, T1, T2);
    impl_join!(T0, T1, T2, T3);
    impl_join!(T0, T1, T2, T3, T4);
    impl_join!(T0, T1, T2, T3, T4, T5);
    impl_join!(T0, T1, T2, T3, T4, T5, T6);
    impl_join!(T0, T1, T2, T3, T4, T5, T6, T7);
    impl_join!(T0, T1, T2, T3, T4, T5, T6, T7, T8);
    impl_join!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9);
    impl_join!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
    impl_join!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
}
