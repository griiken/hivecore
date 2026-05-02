//! Agent registry. Combines builtins with user-supplied directories;
//! enforces unique ids and offers fast lookup. Designed so the ACP server
//! can do `registry.get("default").unwrap_or_else(|| registry.first())`
//! at session start.

use std::collections::HashMap;

use crate::agent::Agent;
use crate::error::ConfigError;

#[derive(Debug, Default, Clone)]
pub struct AgentRegistry {
    by_id: HashMap<String, Agent>,
    order: Vec<String>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_agents(agents: Vec<Agent>) -> Result<Self, ConfigError> {
        let mut reg = Self::new();
        for a in agents {
            reg.insert(a)?;
        }
        Ok(reg)
    }

    pub fn insert(&mut self, agent: Agent) -> Result<(), ConfigError> {
        if self.by_id.contains_key(&agent.id) {
            return Err(ConfigError::Duplicate(agent.id));
        }
        self.order.push(agent.id.clone());
        self.by_id.insert(agent.id.clone(), agent);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Agent> {
        self.by_id.get(id)
    }

    pub fn first(&self) -> Option<&Agent> {
        self.order.first().and_then(|id| self.by_id.get(id))
    }

    pub fn ids(&self) -> &[String] {
        &self.order
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
