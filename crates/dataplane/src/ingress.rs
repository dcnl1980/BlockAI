use crate::pipeline::{DataplaneError, IngressPacket};
use std::collections::VecDeque;

/// AF_XDP-shaped receive socket. Real bindings plug in here; lab uses userspace.
pub trait AfXdpSocket: Send {
    fn name(&self) -> &'static str;
    fn recv(&mut self) -> Result<Option<IngressPacket>, DataplaneError>;
    fn bind_ok(&self) -> bool;
}

/// DPDK-shaped port stub (no PMD in CI).
pub trait DpdkPort: Send {
    fn name(&self) -> &'static str;
    fn rx_burst(&mut self, max: usize) -> Result<Vec<IngressPacket>, DataplaneError>;
    fn started(&self) -> bool;
}

/// In-process queue standing in for AF_XDP fill/comp rings.
#[derive(Default)]
pub struct UserspaceXdp {
    pub bound: bool,
    q: VecDeque<IngressPacket>,
}

impl UserspaceXdp {
    pub fn new() -> Self {
        Self {
            bound: true,
            q: VecDeque::new(),
        }
    }

    pub fn inject(&mut self, pkt: IngressPacket) {
        self.q.push_back(pkt);
    }
}

impl AfXdpSocket for UserspaceXdp {
    fn name(&self) -> &'static str {
        "userspace-xdp"
    }

    fn recv(&mut self) -> Result<Option<IngressPacket>, DataplaneError> {
        if !self.bound {
            return Err(DataplaneError::NotBound);
        }
        Ok(self.q.pop_front())
    }

    fn bind_ok(&self) -> bool {
        self.bound
    }
}

/// Placeholder that refuses to start — documents fail-closed without privileged DPDK.
#[derive(Default)]
pub struct DpdkStub {
    pub started: bool,
}

impl DpdkPort for DpdkStub {
    fn name(&self) -> &'static str {
        "dpdk-stub"
    }

    fn rx_burst(&mut self, _max: usize) -> Result<Vec<IngressPacket>, DataplaneError> {
        if !self.started {
            return Err(DataplaneError::DpdkNotAvailable);
        }
        Ok(vec![])
    }

    fn started(&self) -> bool {
        self.started
    }
}
