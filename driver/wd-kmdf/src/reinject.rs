use wd_kmdf_core::{FixedReinjectionTable, ReinjectionError, ReinjectionToken};

const REINJECTION_CAPACITY: usize = 256;

#[derive(Debug, Clone)]
pub struct ReinjectionTable {
    inner: FixedReinjectionTable<REINJECTION_CAPACITY>,
}

impl Default for ReinjectionTable {
    fn default() -> Self {
        Self {
            inner: FixedReinjectionTable::new(),
        }
    }
}

impl ReinjectionTable {
    pub fn issue_for_network_packet(&mut self, packet_id: u64) -> ReinjectionToken {
        self.inner.issue_for_network_packet(packet_id)
    }

    pub fn consume(&mut self, token: ReinjectionToken) -> Result<u64, ReinjectionError> {
        self.inner.consume(token)
    }

    pub fn consume_raw(&mut self, token: u64) -> Result<u64, ReinjectionError> {
        self.inner.consume_raw(token)
    }
}
