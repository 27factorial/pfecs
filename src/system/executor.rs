use std::fmt;

use parking_lot::RwLock;

use crate::storage::{ComponentStorageAllocator, ResourceStorageAllocator};
use crate::system::{ComponentData, ResourceData, RetrievalError, System};

#[derive(Debug)]
pub struct SystemExecutor {
    raw: RawExecutor,
}

impl SystemExecutor {
    pub fn new<S>(system: S) -> Self
    where
        S: for<'a> System<'a> + Send + Sync,
    {
        Self {
            raw: RawExecutor::new(system),
        }
    }

    pub fn execute(
        &mut self,
        resources: &RwLock<ResourceStorageAllocator>,
        components: &RwLock<ComponentStorageAllocator>,
    ) -> Result<(), RetrievalError> {
        self.raw.execute(resources, components)
    }
}

#[derive(Debug)]
pub struct RawExecutor {
    inner: *mut &'static ExecutorVTable,
}

impl RawExecutor {
    pub fn new<S>(system: S) -> Self
    where
        S: for<'a> System<'a> + Send + Sync,
    {
        let vtable = &ExecutorVTable {
            execute: ExecutorVTable::execute::<S>,
            drop: ExecutorVTable::drop::<S>,
        };

        let inner =
            Box::into_raw(Box::new(Inner::new(vtable, system))) as *mut &'static ExecutorVTable;

        Self { inner }
    }

    pub fn execute(
        &mut self,
        resources: &RwLock<ResourceStorageAllocator>,
        components: &RwLock<ComponentStorageAllocator>,
    ) -> Result<(), RetrievalError> {
        unsafe { ((*self.inner).execute)(self.inner, resources, components) }
    }
}

unsafe impl Send for RawExecutor {}

unsafe impl Sync for RawExecutor {}

impl Drop for RawExecutor {
    fn drop(&mut self) {
        unsafe { ((*self.inner).drop)(self.inner) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct Inner<S>
where
    S: for<'a> System<'a> + Send + Sync,
{
    vtable: &'static ExecutorVTable,
    system: S,
}

impl<S> Inner<S>
where
    S: for<'a> System<'a> + Send + Sync,
{
    pub fn new(vtable: &'static ExecutorVTable, system: S) -> Self {
        Self { vtable, system }
    }
}

pub struct ExecutorVTable {
    /// This function will cast the vtable into an Inner<S> instance.
    execute: unsafe fn(
        *mut &'static Self,
        &RwLock<ResourceStorageAllocator>,
        &RwLock<ComponentStorageAllocator>,
    ) -> Result<(), RetrievalError>,

    drop: unsafe fn(*mut &'static Self),
}

impl ExecutorVTable {
    pub unsafe fn execute<S>(
        ptr: *mut &'static Self,
        resource_alloc: &RwLock<ResourceStorageAllocator>,
        component_alloc: &RwLock<ComponentStorageAllocator>,
    ) -> Result<(), RetrievalError>
    where
        S: for<'a> System<'a> + Send + Sync,
    {
        let inner = ptr as *mut Inner<S>;

        let resource_guard = resource_alloc
            .try_read()
            .ok_or(RetrievalError::ResourceLockedExclusive)?;
        let component_guard = component_alloc
            .try_read()
            .ok_or(RetrievalError::ComponentLockedExclusive)?;

        let resources = S::Resources::fetch(&resource_guard)?;
        let components = S::Components::fetch(&component_guard)?;

        (*inner).system.execute(resources, components);

        Ok(())
    }

    pub unsafe fn drop<S>(ptr: *mut &'static Self)
    where
        S: for<'a> System<'a> + Send + Sync,
    {
        Box::from_raw(ptr as *mut Inner<S>);
    }
}

impl fmt::Debug for ExecutorVTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ExecutorVTable")
            .field("execute", &(self.execute as *const ()))
            .field("drop", &(self.drop as *const ()))
            .finish()
    }
}
