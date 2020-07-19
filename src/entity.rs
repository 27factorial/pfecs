pub type EntityId = u64;
pub type EntityGen = u64;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct Entity {
    id: EntityId,
}

impl Entity {
    pub fn new(id: EntityId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> EntityId {
        self.id
    }
}
