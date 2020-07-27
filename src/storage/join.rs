use crossbeam::channel::{self};
use rayon::prelude::*;

use crate::entity::EntityId;

fn intersect(ids: &[&[EntityId]]) -> Vec<EntityId> {
    let capacity: usize = ids.iter().map(|slice| slice.len()).sum();
    let mut intersect = Vec::with_capacity(capacity);

    ids.iter().flat_map(|slice| slice.iter()).for_each(|id| {
        if ids.iter().all(|slice| slice.contains(id)) {
            intersect.push(*id)
        }
    });

    // Ensure that there is only one of each EntityId.
    intersect.sort_unstable();
    intersect.dedup();

    intersect
}

fn par_intersect(ids: &[&[EntityId]]) -> Vec<EntityId> {
    let capacity: usize = ids.iter().map(|slice| slice.len()).sum();
    let (sender, receiver) = channel::bounded(capacity);

    ids.par_iter()
        .flat_map(|slice| slice.par_iter())
        .for_each_with(sender, |sender, id| {
            if ids.par_iter().all(|slice| slice.contains(id)) {
                sender
                    .send(*id)
                    .expect("Could not send over par_intersect channel.");
            }
        });

    // Ensure that there is only one of each EntityId.
    let mut intersect: Vec<_> = receiver.into_iter().collect();
    intersect.par_sort_unstable();
    intersect.dedup();

    intersect
}

pub trait Join: sealed::StorageTuple
where
    Self: Sized,
{
    fn join(self) -> JoinIter<Self>;
    fn par_join(self) -> ParJoinIter<Self>;
}

#[derive(Debug)]
pub struct JoinIter<ST: Join> {
    tuple: ST,
    ids: Vec<EntityId>,
    current: usize,
}

#[derive(Debug)]
pub struct ParJoinIter<ST: Join> {
    tuple: ST,
    ids: Vec<EntityId>,
}

mod sealed {
    use std::mem;

    use parking_lot::Mutex;
    use rayon::iter::plumbing::{Consumer, UnindexedConsumer};

    use crate::storage::ComponentStorage;
    use crate::{Component, ReadComponent, WriteComponent};

    use super::*;

    pub trait StorageTuple {}

    pub trait StoragePriv {
        type Item;

        fn ids(&self) -> &[EntityId];
        unsafe fn get_item(&mut self, id: EntityId) -> Self::Item;
    }

    impl<'a, T: Component + Send + Sync> StoragePriv for &'a ComponentStorage<T> {
        type Item = &'a T;

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

    impl<'a, T: Component + Send + Sync> StoragePriv for &'a mut ComponentStorage<T> {
        type Item = &'a mut T;

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

    impl<'a, T: Component + Send + Sync> StoragePriv for ReadComponent<'a, T> {
        type Item = &'a T;

        fn ids(&self) -> &[u64] {
            self.storage.data.ids()
        }

        unsafe fn get_item(&mut self, id: u64) -> Self::Item {
            self.storage.data.get_item(id)
        }
    }

    impl<'a, T: Component + Send + Sync> StoragePriv for &'a ReadComponent<'a, T> {
        type Item = &'a T;

        fn ids(&self) -> &[u64] {
            self.storage.data.ids()
        }

        unsafe fn get_item(&mut self, id: u64) -> Self::Item {
            (&mut &*self.storage.data).get_item(id)
        }
    }

    impl<'a, T: Component + Send + Sync> StoragePriv for &'a mut ReadComponent<'a, T> {
        type Item = &'a T;

        fn ids(&self) -> &[u64] {
            self.storage.data.ids()
        }

        unsafe fn get_item(&mut self, id: u64) -> Self::Item {
            self.storage.data.get_item(id)
        }
    }

    impl<'a, T: Component + Send + Sync> StoragePriv for WriteComponent<'a, T> {
        type Item = &'a mut T;

        fn ids(&self) -> &[u64] {
            self.storage.data.ids()
        }

        unsafe fn get_item(&mut self, id: u64) -> Self::Item {
            self.storage.data.get_item(id)
        }
    }

    impl<'a, T: Component + Send + Sync> StoragePriv for &'a WriteComponent<'a, T> {
        type Item = &'a T;

        fn ids(&self) -> &[u64] {
            self.storage.data.ids()
        }

        unsafe fn get_item(&mut self, id: u64) -> Self::Item {
            (&mut &*self.storage.data).get_item(id)
        }
    }

    impl<'a, T: Component + Send + Sync> StoragePriv for &'a mut WriteComponent<'a, T> {
        type Item = &'a mut T;

        fn ids(&self) -> &[u64] {
            self.storage.data.ids()
        }

        unsafe fn get_item(&mut self, id: u64) -> Self::Item {
            self.storage.data.get_item(id)
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
                        let ids = intersect(&[&$t.ids()]);
                        (($t,), ids)
                    };

                    JoinIter {
                        tuple,
                        ids,
                        current: 0,
                    }
                }

                fn par_join(self) -> ParJoinIter<Self> {
                    #[allow(non_snake_case)]
                    let (tuple, ids) = {
                        let ($t,) = self;
                        let ids = par_intersect(&[&$t.ids()]);
                        (($t,), ids)
                    };

                    ParJoinIter {
                        tuple,
                        ids,
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

            impl<$t> ParallelIterator for ParJoinIter<($t,)>
            where
                $t: StoragePriv + Send,
                <$t as StoragePriv>::Item: Send,
            {
                type Item = (<$t as StoragePriv>::Item,);

                fn drive_unindexed<C>(self, consumer: C) -> <C as Consumer<Self::Item>>::Result
                where
                    C: UnindexedConsumer<Self::Item>,
                {
                    let Self { tuple, ids } = self;
                    let tuple = Mutex::new(tuple);

                    ids.par_iter()
                        .map(|id| {
                            mem::forget(tuple.lock());

                            // TODO: Test extensively with miri.
                            #[allow(non_snake_case)]
                            unsafe {
                                let ($t,) = &mut *tuple.data_ptr();
                                let mapped = (StoragePriv::get_item($t, *id),);
                                tuple.force_unlock();
                                mapped
                            }
                        })
                        .drive_unindexed(consumer)
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
                        let ids = intersect(&[$(&$t.ids()),+]);
                        (($($t),+), ids)
                    };

                    JoinIter {
                        tuple,
                        ids,
                        current: 0,
                    }
                }

                fn par_join(self) -> ParJoinIter<Self> {
                    #[allow(non_snake_case)]
                    let (tuple, ids) = {
                        let ($($t),+) = self;
                        let ids = par_intersect(&[$(&$t.ids()),+]);
                        (($($t),+), ids)
                    };

                    ParJoinIter {
                        tuple,
                        ids,
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

            impl<$($t),+> ParallelIterator for ParJoinIter<($($t),+)>
            where
                $(
                    $t: StoragePriv + Send,
                    <$t as StoragePriv>::Item: Send,
                )+
            {
                type Item = ($(<$t as StoragePriv>::Item),+);

                fn drive_unindexed<C>(self, consumer: C) -> <C as Consumer<Self::Item>>::Result
                where
                    C: UnindexedConsumer<Self::Item>,
                {
                    let Self { tuple, ids } = self;
                    let tuple = Mutex::new(tuple);

                    ids.par_iter()
                        .map(|id| {
                            mem::forget(tuple.lock());

                            // TODO: Test extensively with miri.
                            #[allow(non_snake_case)]
                            unsafe {
                                let ($($t),+) = &mut *tuple.data_ptr();
                                let mapped = ($(StoragePriv::get_item($t, *id)),+);
                                tuple.force_unlock();
                                mapped
                            }
                        })
                        .drive_unindexed(consumer)
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
