use std::{marker::PhantomData, mem, ops::Deref};

use crate::{
    system::{ComponentData, ResourceData, RetrievalError, System},
    world::World,
};

#[derive(Debug)]
pub struct Query<'a, R: ResourceData<'a>, C: ComponentData<'a>> {
    world: &'a World,
    _spooky: PhantomData<&'a (R, C)>,
}

impl<'a, R: ResourceData<'a>, C: ComponentData<'a>> Query<'a, R, C> {
    pub fn query(world: &'a World) -> Self {
        Self {
            world,
            _spooky: PhantomData,
        }
    }

    pub fn fetch(&self) -> Result<(QueryResources<'a, R>, QueryComponents<'a, C>), RetrievalError> {
        Ok((self.fetch_resources()?, self.fetch_components()?))
    }

    pub fn fetch_resources(&self) -> Result<QueryResources<'a, R>, RetrievalError> {
        unsafe { QueryResources::new(self.world) }
    }

    pub fn fetch_components(&self) -> Result<QueryComponents<'a, C>, RetrievalError> {
        unsafe { QueryComponents::new(self.world) }
    }

    pub fn build_system<F>(&self, f: F) -> QuerySystem<'a, F, R, C>
    where
        F: FnMut(R, C),
    {
        QuerySystem::new(f)
    }
}

#[derive(Debug)]
pub struct QueryResources<'a, R: ResourceData<'a>> {
    world: &'a World,
    resources: R,
}

impl<'a, R: ResourceData<'a>> QueryResources<'a, R> {
    unsafe fn new(world: &'a World) -> Result<Self, RetrievalError> {
        // Acquire a read lock on the resource allocator
        // and then immediately forget it, since the
        // Drop impl handles unlocking the RwLock
        let guard = world.resource_storage().read();
        let ptr = &*guard as *const _;
        mem::forget(guard);

        let allocator = &*ptr;
        let resources = R::fetch(allocator).map_err(|e| {
            // If an error is returned, the RwLock needs to
            // be unlocked, else it would just be read locked
            // forever.
            world.resource_storage().force_unlock_read();
            e
        })?;

        Ok(Self { world, resources })
    }
}

impl<'a, R: ResourceData<'a>> Deref for QueryResources<'a, R> {
    type Target = R;

    fn deref(&self) -> &R {
        &self.resources
    }
}

impl<'a, R: ResourceData<'a>> Drop for QueryResources<'a, R> {
    fn drop(&mut self) {
        unsafe {
            self.world.resource_storage().force_unlock_read();
        }
    }
}

#[derive(Debug)]
pub struct QueryComponents<'a, C: ComponentData<'a>> {
    world: &'a World,
    components: C,
}

impl<'a, C: ComponentData<'a>> Deref for QueryComponents<'a, C> {
    type Target = C;

    fn deref(&self) -> &C {
        &self.components
    }
}

impl<'a, C: ComponentData<'a>> QueryComponents<'a, C> {
    unsafe fn new(world: &'a World) -> Result<Self, RetrievalError> {
        // Acquire a read lock on the component allocator
        // and then immediately forget it, since the
        // Drop impl handles unlocking the RwLock
        let guard = world.component_storage().read();
        let ptr = &*guard as *const _;
        mem::forget(guard);

        let allocator = &*ptr;
        let components = C::fetch(allocator).map_err(|e| {
            // If an error is returned, the RwLock needs to
            // be unlocked, else it would just be read locked
            // forever.
            world.component_storage().force_unlock_read();
            e
        })?;

        Ok(Self { world, components })
    }
}

impl<'a, R: ComponentData<'a>> Drop for QueryComponents<'a, R> {
    fn drop(&mut self) {
        unsafe {
            self.world.component_storage().force_unlock_read();
        }
    }
}

#[derive(Debug)]
pub struct QuerySystem<'a, F, R, C>
where
    F: FnMut(R, C),
    R: ResourceData<'a>,
    C: ComponentData<'a>,
{
    f: F,
    _spooky: PhantomData<&'a (R, C)>,
}

impl<'a, F, R, C> QuerySystem<'a, F, R, C>
where
    F: FnMut(R, C),
    R: ResourceData<'a>,
    C: ComponentData<'a>,
{
    pub fn new(f: F) -> Self {
        Self {
            f,
            _spooky: PhantomData,
        }
    }
}

impl<'a, F, R, C> System<'a> for QuerySystem<'a, F, R, C>
where
    F: FnMut(R, C),
    R: ResourceData<'a>,
    C: ComponentData<'a>,
{
    type Resources = R;
    type Components = C;

    fn execute(&mut self, resources: Self::Resources, components: Self::Components) {
        (self.f)(resources, components)
    }
}
