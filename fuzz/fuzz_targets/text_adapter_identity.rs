#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use rewrite_text_adapter::TextAdapter;

fuzz_target!(|data: &[u8]| {
    let Ok(parsed) = TextAdapter::parse(data) else {
        return;
    };
    let output = TextAdapter::apply(&parsed, &[]).expect("empty edit set must be valid");
    assert_eq!(output, data);
    assert!(TextAdapter::verify(&parsed, &output, &[]).valid);
});
