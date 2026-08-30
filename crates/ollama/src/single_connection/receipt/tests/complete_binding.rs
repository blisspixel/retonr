use super::*;
use crate::single_connection::receipt::RESIDENT_SESSION_EXECUTION_RECEIPT_COMPLETE_BINDING_DOMAIN;

#[test]
fn complete_resident_execution_binding_has_frozen_identity() {
    let execution = OllamaSessionExecutionReceipt::new(&preflight(), &response(), 8, 16)
        .expect("execution receipt");
    let receipt = OllamaResidentSessionExecutionReceipt::new(execution, &running(), 12, 16);

    assert_eq!(
        RESIDENT_SESSION_EXECUTION_RECEIPT_COMPLETE_BINDING_DOMAIN,
        b"ollama/retained-session/resident-execution-receipt/complete-binding/v1\0"
    );
    assert_eq!(
        receipt.complete_binding_digest().as_str(),
        "ae96eb05ed1db4c98ad903dec687ad25f70d2a771105aec9b9f4a4cca156c547"
    );
}

#[test]
fn every_resident_execution_receipt_field_changes_the_complete_binding() {
    let execution = OllamaSessionExecutionReceipt::new(&preflight(), &response(), 8, 16)
        .expect("execution receipt");
    let original = OllamaResidentSessionExecutionReceipt::new(execution, &running(), 12, 16);
    let original_digest = original.complete_binding_digest();
    let variants = [
        changed(&original, |value| {
            value.execution.preflight_digest = digest("other preflight");
        }),
        changed(&original, |value| {
            value.execution.request_digest = digest("other request");
        }),
        changed(&original, |value| {
            value.execution.response_digest = digest("other response");
        }),
        changed(&original, |value| {
            value.execution.first_response_ordinal += 1;
        }),
        changed(&original, |value| {
            value.execution.last_response_ordinal += 1;
        }),
        changed(&original, |value| {
            value.residency_contract_digest = digest("other residency contract");
        }),
        changed(&original, |value| {
            value.residency_observation_digest = digest("other residency observation");
        }),
        changed(&original, |value| {
            value.runtime_reference_digest = digest("other runtime reference");
        }),
        changed(&original, |value| {
            value.inventory_digest = digest("other inventory");
        }),
        changed(&original, |value| value.byte_size += 1),
        changed(&original, |value| value.accelerator_bytes += 1),
        changed(&original, |value| value.context_tokens += 1),
        changed(&original, |value| value.first_residency_ordinal += 1),
        changed(&original, |value| value.last_residency_ordinal += 1),
    ];

    for (index, variant) in variants.iter().enumerate() {
        assert_ne!(
            variant.complete_binding_digest(),
            original_digest,
            "receipt field variant {index} did not change the complete binding"
        );
    }
}
