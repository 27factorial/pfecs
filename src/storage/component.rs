use std::fmt;
use std::ops::{Deref, DerefMut};
use std::{
    any::{self, TypeId},
    collections::{hash_map::Entry, HashMap},
    mem,
};

use crate::{
    cell::{AtomicRef, AtomicRefCell, AtomicRefMut},
    component::Component,
    entity::Entity,
    entity::EntityId,
    utils,
};

type ComponentDropFn = unsafe fn(*mut ComponentStorageBytes, Entity) -> bool;

/// A container for a dynamic storage type.
#[derive(Debug)]
pub struct ComponentStorageAllocator {
    inner: HashMap<TypeId, AtomicRefCell<(ComponentStorageBytes, ComponentDropFn)>>,
}

impl ComponentStorageAllocator {
    /// Constructs a new `StorageContainer`.
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
    pub fn register<T: Component>(&mut self) -> bool {
        self.register_with::<T, _>(ComponentStorage::new)
    }

    /// Registers a component type with the `StorageContainer` using
    /// the value provided by the passed closure. This can be used
    /// to call custom constructors for the specific storage type
    /// being used. Returns a bool indicating whether the storage
    /// was registered. If this method returns `false`, it means
    /// that the storage was already registered.
    pub fn register_with<T: Component, F>(&mut self, f: F) -> bool
    where
        F: FnOnce() -> ComponentStorage<T>,
    {
        use Entry::*;

        let type_id = TypeId::of::<T>();

        match self.inner.entry(type_id) {
            Occupied(_) => false,
            Vacant(v) => {
                let storage = f();

                let drop_fn = ComponentStorage::<T>::drop_component;
                let bytes = ComponentStorageBytes::new(storage);
                v.insert(AtomicRefCell::new((bytes, drop_fn)));
                true
            }
        }
    }

    pub fn contains<T: Component>(&self) -> bool {
        let type_id = TypeId::of::<T>();
        self.inner.contains_key(&type_id)
    }

    /// Retrieves a reference to the storage associated with the
    /// component type. Returns `None` if no storage was registered
    /// for the component.
    pub fn get<T: Component>(&self) -> Option<AtomicRef<'_, ComponentStorage<T>>> {
        self.inner
            .get(&TypeId::of::<T>())
            .map(|cell| AtomicRef::map(cell.borrow(), |(bytes, _)| unsafe { bytes.cast() }))
    }

    pub fn try_get<T: Component>(&self) -> Option<AtomicRef<'_, ComponentStorage<T>>> {
        self.inner
            .get(&TypeId::of::<T>())
            .and_then(|cell| match cell.try_borrow() {
                Some(borrow) => Some(AtomicRef::map(borrow, |(bytes, _)| unsafe { bytes.cast() })),
                None => None,
            })
    }

    pub unsafe fn get_unchecked<T: Component>(&self) -> AtomicRef<'_, ComponentStorage<T>> {
        let cell = self.inner.get(&TypeId::of::<T>()).unwrap_or_else(|| {
            utils::debug_closure(|| {
                panic!(
                    "Unable to retrieve storage of type {}",
                    any::type_name::<ComponentStorage<T>>()
                );
            })
        });

        AtomicRef::map(cell.borrow(), |(bytes, _)| bytes.cast())
    }

    pub unsafe fn try_get_unchecked<T: Component>(
        &self,
    ) -> Option<AtomicRef<'_, ComponentStorage<T>>> {
        let cell = self.inner.get(&TypeId::of::<T>()).unwrap_or_else(|| {
            utils::debug_closure(|| {
                panic!(
                    "Unable to retrieve storage of type {}",
                    any::type_name::<ComponentStorage<T>>()
                );
            })
        });

        match cell.try_borrow() {
            Some(borrow) => AtomicRef::map(borrow, |(bytes, _)| bytes.cast()).into(),
            None => None,
        }
    }

    /// Retrieves a mutable reference to the storage associated with
    /// the component type. Returns `None` if no storage was registered
    /// for the component.
    pub fn get_mut<T: Component>(&self) -> Option<AtomicRefMut<'_, ComponentStorage<T>>> {
        self.inner.get(&TypeId::of::<T>()).map(|cell| {
            AtomicRefMut::map(cell.borrow_mut(), |(bytes, _)| unsafe { bytes.cast_mut() })
        })
    }

    pub fn try_get_mut<T: Component>(&self) -> Option<AtomicRefMut<'_, ComponentStorage<T>>> {
        self.inner
            .get(&TypeId::of::<T>())
            .and_then(|cell| match cell.try_borrow_mut() {
                Some(borrow) => {
                    AtomicRefMut::map(borrow, |(bytes, _)| unsafe { bytes.cast_mut() }).into()
                }
                None => None,
            })
    }

    pub unsafe fn get_mut_unchecked<T: Component>(&self) -> AtomicRefMut<'_, ComponentStorage<T>> {
        let cell = self.inner.get(&TypeId::of::<T>()).unwrap_or_else(|| {
            utils::debug_closure(|| {
                panic!(
                    "Unable to retrieve storage of type {}",
                    any::type_name::<ComponentStorage<T>>()
                );
            })
        });

        AtomicRefMut::map(cell.borrow_mut(), |(bytes, _)| bytes.cast_mut())
    }

    pub unsafe fn try_get_mut_unchecked<T: Component>(
        &self,
    ) -> Option<AtomicRefMut<'_, ComponentStorage<T>>> {
        let cell = self.inner.get(&TypeId::of::<T>()).unwrap_or_else(|| {
            utils::debug_closure(|| {
                panic!(
                    "Unable to retrieve storage of type {}",
                    any::type_name::<ComponentStorage<T>>()
                );
            })
        });

        match cell.try_borrow_mut() {
            Some(borrow) => AtomicRefMut::map(borrow, |(bytes, _)| bytes.cast_mut()).into(),
            None => None,
        }
    }

    pub fn get_or_register<T: Component>(&mut self) -> AtomicRef<'_, ComponentStorage<T>> {
        // Attempt to register the storage.
        self.register::<T>();

        self.get().unwrap_or_else(|| unsafe {
            utils::debug_unreachable("Storage could not be retrieved after it was registered.");
        })
    }

    pub fn get_mut_or_register<T: Component>(&mut self) -> AtomicRefMut<'_, ComponentStorage<T>> {
        // Attempt to register the storage.
        self.register::<T>();

        self.get_mut().unwrap_or_else(|| unsafe {
            utils::debug_unreachable(
                "Storage could not be retrieved mutably after it was registered.",
            );
        })
    }

    pub fn get_or_register_with<T: Component, F>(
        &mut self,
        f: F,
    ) -> AtomicRef<'_, ComponentStorage<T>>
    where
        F: FnOnce() -> ComponentStorage<T>,
    {
        // Attempt to register the storage with the provided closure.
        self.register_with::<T, _>(f);

        self.get().unwrap_or_else(|| unsafe {
            utils::debug_unreachable("Storage could not be retrieved after it was registered.");
        })
    }

    pub fn get_mut_or_register_with<T: Component, F>(
        &mut self,
        f: F,
    ) -> AtomicRefMut<'_, ComponentStorage<T>>
    where
        F: FnOnce() -> ComponentStorage<T>,
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
    pub fn remove_storage<T: Component>(&mut self) -> Option<ComponentStorage<T>> {
        self.inner
            .remove(&TypeId::of::<T>())
            .map(|cell| unsafe { cell.into_inner().0.into_storage() })
    }

    pub fn remove_components(&mut self, entity: Entity) {
        self.inner.values_mut().for_each(|cell| {
            let (bytes, drop_fn) = cell.get_mut();
            unsafe {
                drop_fn(bytes, entity);
            }
        })
    }

    /// Retrieves a mutable reference to the storage associated with
    /// the component type and calls the provided closure with it.
    /// Returns a `bool` indicating whether the closure was called.
    pub fn get_and_update<T: Component, F>(&self, f: F) -> bool
    where
        F: FnOnce(&mut ComponentStorage<T>),
    {
        match self.get_mut::<T>() {
            Some(mut s) => {
                f(&mut s);
                true
            }
            _ => false,
        }
    }

    pub unsafe fn get_and_update_unchecked<T: Component, F>(&self, f: F)
    where
        F: FnOnce(&mut ComponentStorage<T>),
    {
        f(&mut self.get_mut_unchecked::<T>())
    }
}

const COMP_STORAGE_BYTES: usize = mem::size_of::<ComponentStorage<()>>();

// Important implementation note: this type relies
// on the internal representation of ComponentStorage<T>,
// which has a size of 48 and an alignment of 8.
// This means a transmute between these two types
// *should* be safe assuming T is the correct type
// when transmuting back to the ComponentStorage<T>.
#[repr(C, align(8))]
pub struct ComponentStorageBytes {
    bytes: [u8; COMP_STORAGE_BYTES],
}

impl ComponentStorageBytes {
    pub fn new<T: Component>(storage: ComponentStorage<T>) -> Self {
        unsafe {
            // SAFETY: ComponentStorage<T> and StorageBytes both
            // have the same size and alignment, so this is just
            // a direct conversion to the raw bytes of the storage.
            mem::transmute(storage)
        }
    }

    pub unsafe fn cast<T: Component>(&self) -> &ComponentStorage<T> {
        mem::transmute(self)
    }

    pub unsafe fn cast_mut<T: Component>(&mut self) -> &mut ComponentStorage<T> {
        mem::transmute(self)
    }

    pub unsafe fn into_storage<T: Component>(self) -> ComponentStorage<T> {
        mem::transmute(self)
    }
}

impl fmt::Debug for ComponentStorageBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(&self.bytes[..]).finish()
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ComponentStorage<T: Component> {
    ids: Vec<EntityId>,
    comps: Vec<T>,
}

impl<T: Component> ComponentStorage<T> {
    pub fn new() -> Self {
        Self {
            ids: Vec::new(),
            comps: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ids: Vec::with_capacity(capacity),
            comps: Vec::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        let len = self.comps.len();
        debug_assert_eq!(
            self.ids.len(),
            len,
            "ID & Component Vec lengths do not match."
        );

        len
    }

    pub fn push(&mut self, id: EntityId, t: T) -> Result<(), T> {
        if self.ids.contains(&id) {
            Err(t)
        } else {
            self.ids.push(id);
            self.comps.push(t);
            Ok(())
        }
    }

    pub fn pop(&mut self) -> Option<(EntityId, T)> {
        let id = self.ids.pop();
        let comp = self.comps.pop();

        match (id, comp) {
            (Some(id), Some(comp)) => Some((id, comp)),
            (None, None) => None,
            _ => unsafe {
                utils::debug_unreachable(
                    "Invalid ComponentStorage state. ID & Component Vec lengths do not match.",
                )
            },
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<(EntityId, T)> {
        if index >= self.len() {
            None
        } else {
            let id = self.ids.remove(index);
            let comp = self.comps.remove(index);

            Some((id, comp))
        }
    }

    pub fn remove_by_id(&mut self, id: EntityId) -> Option<T> {
        self.ids
            .iter()
            .enumerate()
            .find(|(_, &other_id)| id == other_id)
            .map(|(index, _)| index)
            .and_then(|index| {
                self.ids.remove(index);
                self.comps.remove(index).into()
            })
    }

    pub fn entities(&self) -> &[EntityId] {
        &self.ids
    }

    pub fn components(&self) -> &[T] {
        &self.comps
    }

    pub fn components_mut(&mut self) -> &mut [T] {
        &mut self.comps
    }

    pub fn iter(&self) -> impl Iterator<Item = (EntityId, &'_ T)> {
        self.ids.iter().copied().zip(self.comps.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (EntityId, &'_ mut T)> {
        self.ids.iter().copied().zip(self.comps.iter_mut())
    }

    pub fn comp_iter(&self) -> impl Iterator<Item = &'_ T> {
        self.comps.iter()
    }

    pub fn comp_iter_mut(&mut self) -> impl Iterator<Item = &'_ mut T> {
        self.comps.iter_mut()
    }

    unsafe fn drop_component(ptr: *mut ComponentStorageBytes, entity: Entity) -> bool {
        let storage = &mut *mem::transmute::<_, *mut Self>(ptr);

        match storage.remove_by_id(entity.id()) {
            Some(_) => true,
            None => false,
        }
    }
}

#[derive(Debug)]
pub struct Read<'a, T: Component> {
    storage: AtomicRef<'a, ComponentStorage<T>>,
}

impl<'a, T: Component> Read<'a, T> {
    pub fn new(storage: AtomicRef<'a, ComponentStorage<T>>) -> Self {
        Self { storage }
    }
}

impl<T: Component> Deref for Read<'_, T> {
    type Target = ComponentStorage<T>;

    fn deref(&self) -> &ComponentStorage<T> {
        &*self.storage
    }
}

#[derive(Debug)]
pub struct Write<'a, T: Component> {
    storage: AtomicRefMut<'a, ComponentStorage<T>>,
}

impl<'a, T: Component> Write<'a, T> {
    pub fn new(storage: AtomicRefMut<'a, ComponentStorage<T>>) -> Self {
        Self { storage }
    }
}

impl<T: Component> Deref for Write<'_, T> {
    type Target = ComponentStorage<T>;

    fn deref(&self) -> &ComponentStorage<T> {
        &*self.storage
    }
}

impl<T: Component> DerefMut for Write<'_, T> {
    fn deref_mut(&mut self) -> &mut ComponentStorage<T> {
        &mut *self.storage
    }
}
