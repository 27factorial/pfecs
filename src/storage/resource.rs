use std::{
    any::{self, TypeId},
    collections::{hash_map::Entry, HashMap},
    fmt, mem,
    ops::{Deref, DerefMut},
};

use crate::{
    cell::{AtomicRef, AtomicRefCell, AtomicRefMut},
    resource::Resource,
    utils,
};

type ResourceDropFn = unsafe fn(ResourceStorageBytes);

#[derive(Debug)]
pub struct ResourceStorageAllocator {
    inner: HashMap<TypeId, AtomicRefCell<(ResourceStorageBytes, ResourceDropFn)>>,
}

impl ResourceStorageAllocator {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: HashMap::with_capacity(capacity),
        }
    }

    /// Registers a component type with the `StorageContainer`,
    /// using the default constructor. Returns a bool indicating
    /// whether the storage was registered. If this method returns
    /// `false`, it means that the storage was already registered.
    pub fn register<T: Resource>(&mut self, resource: T) -> bool {
        self.register_with::<T, _>(|| ResourceStorage::new(resource))
    }

    /// Registers a component type with the `StorageContainer` using
    /// the value provided by the passed closure. This can be used
    /// to call custom constructors for the specific storage type
    /// being used. Returns a bool indicating whether the storage
    /// was registered. If this method returns `false`, it means
    /// that the storage was already registered.
    pub fn register_with<T: Resource, F>(&mut self, f: F) -> bool
    where
        F: FnOnce() -> ResourceStorage<T>,
    {
        use Entry::*;

        let type_id = TypeId::of::<T>();

        match self.inner.entry(type_id) {
            Occupied(_) => false,
            Vacant(v) => {
                let storage = f();

                let drop_fn = ResourceStorage::<T>::drop_resource;
                let bytes = ResourceStorageBytes::new(storage);
                v.insert(AtomicRefCell::new((bytes, drop_fn)));
                true
            }
        }
    }

    pub fn contains<T: Resource>(&self) -> bool {
        let type_id = TypeId::of::<T>();
        self.inner.contains_key(&type_id)
    }

    /// Retrieves a reference to the storage associated with the
    /// component type. Returns `None` if no storage was registered
    /// for the component.
    pub fn get<T: Resource>(&self) -> Option<AtomicRef<'_, ResourceStorage<T>>> {
        self.inner
            .get(&TypeId::of::<T>())
            .map(|cell| AtomicRef::map(cell.borrow(), |(bytes, _)| unsafe { bytes.cast() }))
    }

    pub fn try_get<T: Resource>(&self) -> Option<AtomicRef<'_, ResourceStorage<T>>> {
        self.inner
            .get(&TypeId::of::<T>())
            .and_then(|cell| match cell.try_borrow() {
                Some(borrow) => Some(AtomicRef::map(borrow, |(bytes, _)| unsafe { bytes.cast() })),
                None => None,
            })
    }

    pub unsafe fn get_unchecked<T: Resource>(&self) -> AtomicRef<'_, ResourceStorage<T>> {
        let cell = self.inner.get(&TypeId::of::<T>()).unwrap_or_else(|| {
            utils::debug_closure(|| {
                panic!(
                    "Unable to retrieve storage of type {}",
                    any::type_name::<ResourceStorage<T>>()
                );
            })
        });

        AtomicRef::map(cell.borrow(), |(bytes, _)| bytes.cast())
    }

    pub unsafe fn try_get_unchecked<T: Resource>(
        &self,
    ) -> Option<AtomicRef<'_, ResourceStorage<T>>> {
        let cell = self.inner.get(&TypeId::of::<T>()).unwrap_or_else(|| {
            utils::debug_closure(|| {
                panic!(
                    "Unable to retrieve storage of type {}",
                    any::type_name::<ResourceStorage<T>>()
                );
            })
        });

        match cell.try_borrow() {
            Some(borrow) => Some(AtomicRef::map(borrow, |(bytes, _)| bytes.cast())),
            None => None,
        }
    }

    /// Retrieves a mutable reference to the storage associated with
    /// the component type. Returns `None` if no storage was registered
    /// for the component.
    pub fn get_mut<T: Resource>(&self) -> Option<AtomicRefMut<'_, ResourceStorage<T>>> {
        self.inner.get(&TypeId::of::<T>()).map(|cell| {
            AtomicRefMut::map(cell.borrow_mut(), |(bytes, _)| unsafe { bytes.cast_mut() })
        })
    }

    pub fn try_get_mut<T: Resource>(&self) -> Option<AtomicRefMut<'_, ResourceStorage<T>>> {
        self.inner
            .get(&TypeId::of::<T>())
            .and_then(|cell| match cell.try_borrow_mut() {
                Some(borrow) => Some(AtomicRefMut::map(borrow, |(bytes, _)| unsafe {
                    bytes.cast_mut()
                })),
                None => None,
            })
    }

    pub unsafe fn get_mut_unchecked<T: Resource>(&self) -> AtomicRefMut<'_, ResourceStorage<T>> {
        let cell = self.inner.get(&TypeId::of::<T>()).unwrap_or_else(|| {
            utils::debug_closure(|| {
                panic!(
                    "Unable to retrieve storage of type {}",
                    any::type_name::<ResourceStorage<T>>()
                );
            })
        });

        AtomicRefMut::map(cell.borrow_mut(), |(bytes, _)| bytes.cast_mut())
    }

    pub unsafe fn try_get_mut_unchecked<T: Resource>(
        &self,
    ) -> Option<AtomicRefMut<'_, ResourceStorage<T>>> {
        let cell = self.inner.get(&TypeId::of::<T>()).unwrap_or_else(|| {
            utils::debug_closure(|| {
                panic!(
                    "Unable to retrieve storage of type {}",
                    any::type_name::<ResourceStorage<T>>()
                );
            })
        });

        match cell.try_borrow_mut() {
            Some(borrow) => Some(AtomicRefMut::map(borrow, |(bytes, _)| bytes.cast_mut())),
            None => None,
        }
    }

    pub fn get_and_register<T: Resource>(
        &mut self,
        resource: T,
    ) -> AtomicRef<'_, ResourceStorage<T>> {
        // Attempt to register the storage.
        self.register(resource);

        self.get().unwrap_or_else(|| unsafe {
            utils::debug_unreachable("Storage could not be retrieved after it was registered.");
        })
    }

    pub fn get_mut_and_register<T: Resource>(
        &mut self,
        resource: T,
    ) -> AtomicRefMut<'_, ResourceStorage<T>> {
        // Attempt to register the storage.
        self.register(resource);

        self.get_mut().unwrap_or_else(|| unsafe {
            utils::debug_unreachable(
                "Storage could not be retrieved mutably after it was registered.",
            );
        })
    }

    pub fn get_and_register_with<T: Resource, F>(
        &mut self,
        f: F,
    ) -> AtomicRef<'_, ResourceStorage<T>>
    where
        F: FnOnce() -> ResourceStorage<T>,
    {
        // Attempt to register the storage with the provided closure.
        self.register_with::<T, _>(f);

        self.get().unwrap_or_else(|| unsafe {
            utils::debug_unreachable("Storage could not be retrieved after it was registered.");
        })
    }

    pub fn get_mut_and_register_with<T: Resource, F>(
        &mut self,
        f: F,
    ) -> AtomicRefMut<'_, ResourceStorage<T>>
    where
        F: FnOnce() -> ResourceStorage<T>,
    {
        // Attempt to register the storage with the provided closure.
        self.register_with::<T, _>(f);

        self.get_mut().unwrap_or_else(|| unsafe {
            utils::debug_unreachable(
                "Storage could not be retrieved mutably after it was registered.",
            );
        })
    }

    /// Removes the storage associated with the component type and
    /// returns it. Returns `None` if no storage registered for the
    /// component.
    pub fn remove_storage<T: Resource>(&mut self) -> Option<ResourceStorage<T>> {
        self.inner
            .remove(&TypeId::of::<T>())
            .map(|cell| unsafe { cell.into_inner().0.into_storage() })
    }
}

const RES_STORAGE_BYTES: usize = mem::size_of::<ResourceStorage<()>>();

#[cfg_attr(target_pointer_width = "32", repr(C, align(4)))]
#[cfg_attr(target_pointer_width = "64", repr(C, align(8)))]
pub struct ResourceStorageBytes {
    bytes: [u8; RES_STORAGE_BYTES],
}

impl ResourceStorageBytes {
    pub fn new<T: Resource>(storage: ResourceStorage<T>) -> Self {
        unsafe {
            // SAFETY: ResourceStorage<T> and StorageBytes both
            // have the same size and alignment, so this is just
            // a direct conversion to the raw bytes of the storage.
            mem::transmute(storage)
        }
    }

    pub unsafe fn cast<T: Resource>(&self) -> &ResourceStorage<T> {
        mem::transmute(self)
    }

    pub unsafe fn cast_mut<T: Resource>(&mut self) -> &mut ResourceStorage<T> {
        mem::transmute(self)
    }

    pub unsafe fn into_storage<T: Resource>(self) -> ResourceStorage<T> {
        mem::transmute(self)
    }
}

impl fmt::Debug for ResourceStorageBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(&self.bytes[..]).finish()
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct ResourceStorage<T: Resource> {
    resource: Box<T>,
}

impl<T: Resource> ResourceStorage<T> {
    pub fn new(resource: T) -> Self {
        Self {
            resource: Box::new(resource),
        }
    }

    unsafe fn drop_resource(bytes: ResourceStorageBytes) {
        drop(mem::transmute::<_, Self>(bytes));
    }
}

impl<T: Resource> Deref for ResourceStorage<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.resource
    }
}

impl<T: Resource> DerefMut for ResourceStorage<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.resource
    }
}

#[derive(Debug)]
pub struct Read<'a, T: Resource> {
    storage: AtomicRef<'a, ResourceStorage<T>>,
}

impl<'a, T: Resource> Read<'a, T> {
    pub fn new(storage: AtomicRef<'a, ResourceStorage<T>>) -> Self {
        Self { storage }
    }
}

impl<T: Resource> Deref for Read<'_, T> {
    type Target = ResourceStorage<T>;

    fn deref(&self) -> &ResourceStorage<T> {
        &*self.storage
    }
}

#[derive(Debug)]
pub struct Write<'a, T: Resource> {
    storage: AtomicRefMut<'a, ResourceStorage<T>>,
}

impl<'a, T: Resource> Write<'a, T> {
    pub fn new(storage: AtomicRefMut<'a, ResourceStorage<T>>) -> Self {
        Self { storage }
    }
}

impl<T: Resource> Deref for Write<'_, T> {
    type Target = ResourceStorage<T>;

    fn deref(&self) -> &ResourceStorage<T> {
        &*self.storage
    }
}

impl<T: Resource> DerefMut for Write<'_, T> {
    fn deref_mut(&mut self) -> &mut ResourceStorage<T> {
        &mut *self.storage
    }
}
