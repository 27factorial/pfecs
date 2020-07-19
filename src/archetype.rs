use crate::{component::ComponentSet, entity::Entity};

#[derive(Debug)]
pub struct Archetype {
    entities: Vec<Entity>,
    components: ComponentSet,
}

impl Archetype {
    pub fn new(components: ComponentSet) -> Self {
        Self {
            entities: Vec::new(),
            components,
        }
    }

    pub fn push(&mut self, entity: Entity) {
        self.entities.push(entity);
    }

    pub fn pop(&mut self) -> Option<Entity> {
        self.entities.pop()
    }

    pub fn remove(&mut self, entity: Entity) -> bool {
        let index = self
            .entities
            .iter()
            .enumerate()
            .find(|(_, other)| entity.id() == other.id())
            .map(|(index, _)| index);

        match index {
            Some(index) => {
                self.entities.remove(index);
                true
            }
            None => false,
        }
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.entities.contains(&entity)
    }

    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub fn components(&self) -> &ComponentSet {
        &self.components
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = &'_ Entity> {
        self.entities.iter()
    }
}
