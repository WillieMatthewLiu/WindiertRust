use std::collections::HashMap;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReinjectionToken(u64);

#[derive(Debug, Clone)]
pub struct ReinjectionTable {
    next_token: u64,
    outstanding: HashMap<ReinjectionToken, u64>,
}

impl Default for ReinjectionTable {
    fn default() -> Self {
        Self {
            next_token: 1,
            outstanding: HashMap::new(),
        }
    }
}

impl ReinjectionTable {
    pub fn issue_for_network_packet(&mut self, packet_id: u64) -> ReinjectionToken {
        let token = ReinjectionToken(self.next_token);
        self.next_token = self.next_token.saturating_add(1);
        self.outstanding.insert(token, packet_id);
        token
    }

    pub fn consume(&mut self, token: ReinjectionToken) -> Result<u64, ReinjectionError> {
        self.outstanding
            .remove(&token)
            .ok_or(ReinjectionError::UnknownToken)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReinjectionError {
    UnknownToken,
}

impl Display for ReinjectionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownToken => write!(f, "unknown reinjection token"),
        }
    }
}

impl std::error::Error for ReinjectionError {}
