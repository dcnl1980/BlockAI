//! Lab demo: userspace XDP ingress → filter pipeline + HSM 3-of-5 root op + hardware attest gate.

use blockai_attest::{Attestor, HardwareAttestor, SoftwareAttestor, TestPlatform};
use blockai_dataplane::{AfXdpSocket, DataplanePipeline, IngressPacket, PipelineConfig, UserspaceXdp};
use blockai_hsm::{RootOp, SoftHsm3of5, HSM_QUORUM};
use blockai_net::{encode_frame, AppFrame};
use blockai_types::{AgentId, AmountMicros, CapabilityId, Epoch, Pay, Sequence};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "dataplane_sim")]
struct Args {
    #[arg(long, default_value_t = false)]
    measured_hw: bool,
}

fn main() {
    let args = Args::parse();
    let cap = *blake3::hash(b"cap-demo").as_bytes();

    let mut xdp = UserspaceXdp::new();
    let pay = Pay {
        capability_id: CapabilityId(cap),
        epoch: Epoch(1),
        sequence: Sequence(1),
        agent_id: AgentId([2u8; 32]),
        service_id: "inference/x".into(),
        amount: AmountMicros(1),
        currency: "EURC".into(),
        request_hash: [3u8; 32],
        price_quote_hash: [4u8; 32],
        max_amount: AmountMicros(5),
        pricing_schedule_version: 1,
        expiry_unix_ms: 9_999_999_999,
        agent_signature: vec![0u8; 64],
    };
    xdp.inject(IngressPacket {
        bytes: encode_frame(&AppFrame::Pay { pay }).unwrap(),
        is_early_data: false,
        src_hint: "agent-1".into(),
    });

    let mut pipe = DataplanePipeline::new(PipelineConfig::default());
    pipe.warm_capability(cap);
    let pkt = xdp.recv().unwrap().expect("packet");
    let frame = pipe.process(&pkt).expect("pipeline");
    assert!(matches!(frame, AppFrame::Pay { .. }));

    let hsm = SoftHsm3of5::generate();
    let root = hsm
        .sign_with(
            &RootOp::AuthorizeIssuer {
                issuer_pubkey: [9u8; 32],
            },
            &[0, 1, 2],
        )
        .unwrap();
    hsm.verify(&root, HSM_QUORUM).unwrap();

    let sw = SoftwareAttestor::new();
    let sw_ev = sw.collect().unwrap();
    sw.verify_against(sw.policy(), &sw_ev).unwrap();

    let hw = if args.measured_hw {
        HardwareAttestor::with_measurement(TestPlatform::new())
    } else {
        HardwareAttestor::unmeasured(TestPlatform::new())
    };
    let hw_status = match hw.collect() {
        Ok(ev) => {
            hw.verify_against(hw.policy(), &ev).unwrap();
            "measured_ok"
        }
        Err(_) => "fail_closed",
    };

    println!(
        "ok dataplane ingress={} pipeline=pay hsm_quorum={}/{} attest_sw=ok attest_hw={}",
        xdp.name(),
        HSM_QUORUM,
        blockai_hsm::HSM_SHARES,
        hw_status
    );
}
