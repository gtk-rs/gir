use crate::{config::gobjects::*, library::*};

#[derive(Debug, Clone)]
pub struct StatusedTypeId {
    pub type_id: TypeId,
    pub name: String,
    pub status: GStatus,
}
