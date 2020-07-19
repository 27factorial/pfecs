use parking_lot::RwLock;

use crate::{
    archetype::Archetype,
    component::{ComponentSet, ComponentTuple, IntoComponentTuple},
    entity::{Entity, EntityId},
    storage::{ComponentStorageAllocator, ResourceStorageAllocator},
    utils, IntoResourceTuple, ResourceTuple,
};

pub mod query;

#[derive(Debug)]
pub struct World {
    archetypes: Vec<Archetype>,
    entities: Vec<Entity>,
    resource_storage: RwLock<ResourceStorageAllocator>,
    component_storage: RwLock<ComponentStorageAllocator>,
    next_id: EntityId,
}

impl World {
    pub fn new() -> Self {
        Self {
            archetypes: Vec::new(),
            entities: Vec::new(),
            resource_storage: RwLock::new(ResourceStorageAllocator::new()),
            component_storage: RwLock::new(ComponentStorageAllocator::new()),
            next_id: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            archetypes: Vec::with_capacity(capacity),
            entities: Vec::with_capacity(capacity),
            resource_storage: RwLock::new(ResourceStorageAllocator::new()),
            component_storage: RwLock::new(ComponentStorageAllocator::new()),
            next_id: 0,
        }
    }

    pub fn create_entity_iter<I, ICT, CT>(&mut self, container: I) -> &[Entity]
    where
        I: IntoIterator<Item = ICT>,
        ICT: IntoComponentTuple<CT>,
        CT: ComponentTuple,
    {
        let iter = container.into_iter();
        let start_index = self.entities.len();
        let comp_set = ComponentSet::from_tuple::<CT>();

        for into_ct in iter {
            let components = into_ct.into();
            let entity = Entity::new(self.next_id);
            self.create_entity_impl(entity, components, &comp_set);
        }

        &self.entities[start_index..]
    }

    pub fn create_entity<ICT, CT>(&mut self, components: ICT) -> Entity
    where
        ICT: IntoComponentTuple<CT>,
        CT: ComponentTuple,
    {
        let comp_set = ComponentSet::from_tuple::<CT>();
        let components = components.into();
        let entity = Entity::new(self.next_id);

        self.create_entity_impl(entity, components, &comp_set)
    }

    pub fn add_components<ICT, CT>(&mut self, entity: Entity, components: ICT) -> Result<(), CT>
    where
        ICT: IntoComponentTuple<CT>,
        CT: ComponentTuple,
    {
        let new_comp_set = ComponentSet::from_tuple::<CT>();
        let components = components.into();
        self.add_components_impl(entity, components, new_comp_set)
    }

    fn create_entity_impl<CT: ComponentTuple>(
        &mut self,
        entity: Entity,
        components: CT,
        comp_set: &ComponentSet,
    ) -> Entity {
        let archetype = match self.get_archetype_mut(&comp_set) {
            Some(arch) => arch,
            None => self.create_archetype::<CT>(comp_set.clone()),
        };
        archetype.push(entity);

        components.store(entity, self.component_storage.get_mut());
        self.entities.push(entity);
        self.next_id += 1;

        entity
    }

    fn add_components_impl<CT: ComponentTuple>(
        &mut self,
        entity: Entity,
        components: CT,
        new_comp_set: ComponentSet,
    ) -> Result<(), CT> {
        let arch = match self.archetype_of_mut(entity) {
            Some(arch) => arch,
            None => return Err(components),
        };
        let old_comp_set = arch.components().clone();

        arch.remove(entity);

        let comp_set = {
            let old = old_comp_set.into_inner().into_iter();
            let new = new_comp_set.into_inner().into_iter();
            ComponentSet::new(old.chain(new).collect())
        };

        let archetype = match self.get_archetype_mut(&comp_set) {
            Some(arch) => arch,
            None => self.create_archetype::<CT>(comp_set),
        };
        archetype.push(entity);
        components.store(entity, self.component_storage.get_mut());

        Ok(())
    }

    pub fn add_resources<IRT, RT>(&mut self, resources: IRT)
    where
        IRT: IntoResourceTuple<RT>,
        RT: ResourceTuple,
    {
        let resources = resources.into();
        resources.store(self.resource_storage.get_mut());
    }

    pub fn archetype_of(&self, entity: Entity) -> Option<&Archetype> {
        self.archetypes.iter().find(|arch| arch.contains(entity))
    }

    pub fn archetype_of_mut(&mut self, entity: Entity) -> Option<&mut Archetype> {
        self.archetypes
            .iter_mut()
            .find(|arch| arch.contains(entity))
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.iter().copied()
    }

    pub fn archetype_iter(&self) -> impl Iterator<Item = &'_ Archetype> {
        self.archetypes.iter()
    }

    pub(crate) fn resource_storage(&self) -> &RwLock<ResourceStorageAllocator> {
        &self.resource_storage
    }

    pub(crate) fn component_storage(&self) -> &RwLock<ComponentStorageAllocator> {
        &self.component_storage
    }

    fn get_archetype(&self, components: &ComponentSet) -> Option<&Archetype> {
        self.archetypes
            .iter()
            .find(|arch| arch.components() == components)
    }

    fn get_archetype_mut(&mut self, components: &ComponentSet) -> Option<&mut Archetype> {
        self.archetypes
            .iter_mut()
            .find(|arch| arch.components() == components)
    }

    fn create_archetype<CT: ComponentTuple>(&mut self, components: ComponentSet) -> &mut Archetype {
        debug_assert!(
            self.get_archetype(&components).is_none(),
            "This method should only be called if the archetype didn't already exist.\
             While this is not unsafe, it is a waste of memory.",
        );

        self.archetypes.push(Archetype::new(components));
        self.archetypes.last_mut().unwrap_or_else(|| unsafe {
            utils::debug_unreachable(
                "self.archetypes did not contain last element after it was pushed to.",
            )
        })
    }
}
