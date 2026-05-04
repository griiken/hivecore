//! In-memory registry of validated `Role`s. Parallel to `AgentRegistry`.
//! Lookup by name. ADR-033.

use std::collections::HashMap;

use crate::role::Role;

#[derive(Debug, Default, Clone)]
pub struct RoleRegistry {
    by_name: HashMap<String, Role>,
}

impl RoleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_roles(roles: impl IntoIterator<Item = Role>) -> Self {
        let mut by_name = HashMap::new();
        for r in roles {
            by_name.insert(r.name.clone(), r);
        }
        Self { by_name }
    }

    pub fn insert(&mut self, role: Role) {
        self.by_name.insert(role.name.clone(), role);
    }

    pub fn get(&self, name: &str) -> Option<&Role> {
        self.by_name.get(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    pub fn len(&self) -> usize {
        self.by_name.len()
    }
}
