#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use rewrite_engine::ProtectionPlan;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = core::str::from_utf8(data) else {
        return;
    };
    let Ok(plan) = ProtectionPlan::build(source, &[]) else {
        return;
    };
    let masked = plan
        .mask_raw_candidate(source)
        .expect("source must contain its own extracted protected values");
    let restored = plan
        .restore(&masked)
        .expect("engine-produced sentinels must restore");
    assert_eq!(restored, source);
});
