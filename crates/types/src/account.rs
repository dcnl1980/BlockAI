use crate::{AccountId, AgentId, AmountMicros};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountKind {
    Human,
    Agent,
    Contract,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub kind: AccountKind,
    pub balance_available: AmountMicros,
    pub balance_locked: AmountMicros,
    pub stake: AmountMicros,
    pub agent_id: Option<AgentId>,
    pub reputation: i64,
    pub suspended: bool,
}

impl Account {
    pub fn new_human(id: AccountId, balance: AmountMicros) -> Self {
        Self {
            id,
            kind: AccountKind::Human,
            balance_available: balance,
            balance_locked: AmountMicros(0),
            stake: AmountMicros(0),
            agent_id: None,
            reputation: 0,
            suspended: false,
        }
    }

    pub fn new_agent(id: AccountId, agent_id: AgentId, balance: AmountMicros) -> Self {
        Self {
            id,
            kind: AccountKind::Agent,
            balance_available: balance,
            balance_locked: AmountMicros(0),
            stake: AmountMicros(0),
            agent_id: Some(agent_id),
            reputation: 0,
            suspended: false,
        }
    }
}
