use crate::pipeline::{DataplaneError, IngressPacket};
use std::collections::VecDeque;

/// Preferred NIC dataplane backend (production selection seam).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataplaneBackend {
    Userspace,
    AfXdp,
    Dpdk,
}

/// Probe which backend can run in this process. Privileged AF_XDP/DPDK fall back in CI.
pub fn select_backend(prefer: DataplaneBackend) -> DataplaneBackend {
    match prefer {
        DataplaneBackend::Userspace => DataplaneBackend::Userspace,
        DataplaneBackend::AfXdp => {
            if AfXdpProbe::available() {
                DataplaneBackend::AfXdp
            } else {
                DataplaneBackend::Userspace
            }
        }
        DataplaneBackend::Dpdk => {
            if DpdkProbe::available() {
                DataplaneBackend::Dpdk
            } else {
                DataplaneBackend::Userspace
            }
        }
    }
}

/// Capability probe for real AF_XDP (always false without CAP_NET_RAW + driver).
pub struct AfXdpProbe;
impl AfXdpProbe {
    pub fn available() -> bool {
        // Lab/CI: never claim privileged XDP without explicit env opt-in.
        std::env::var_os("BLOCKAI_AF_XDP").is_some()
    }
}

/// Capability probe for DPDK PMD (always false without hugepages + driver).
pub struct DpdkProbe;
impl DpdkProbe {
    pub fn available() -> bool {
        std::env::var_os("BLOCKAI_DPDK").is_some()
    }
}

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
