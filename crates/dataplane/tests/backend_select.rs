use blockai_dataplane::{select_backend, DataplaneBackend};

#[test]
fn privileged_backends_fall_back_without_env() {
    std::env::remove_var("BLOCKAI_AF_XDP");
    std::env::remove_var("BLOCKAI_DPDK");
    assert_eq!(
        select_backend(DataplaneBackend::AfXdp),
        DataplaneBackend::Userspace
    );
    assert_eq!(
        select_backend(DataplaneBackend::Dpdk),
        DataplaneBackend::Userspace
    );
    assert_eq!(
        select_backend(DataplaneBackend::Userspace),
        DataplaneBackend::Userspace
    );
}
