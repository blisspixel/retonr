mod support;

use std::io::{self, Cursor, Read};

use rewrite_ollama_package::{GgufLimits, ReconstructionError, inspect_gguf_v3};
use rewrite_types::Digest;

use support::{
    MetadataValue, Tensor, base_metadata, base_tensors, build_gguf, package_fixture, tiny_gguf,
};

struct Broken;

impl Read for Broken {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("sensitive stream detail"))
    }
}

fn inspect(bytes: &[u8]) -> Result<rewrite_ollama_package::GgufObservation, ReconstructionError> {
    inspect_gguf_v3(
        &mut Cursor::new(bytes),
        &Digest::sha256(bytes),
        bytes.len() as u64,
        &GgufLimits::default(),
        &mut || false,
    )
}

#[test]
fn tiny_fixture_has_stable_structural_and_component_identities() {
    let bytes = tiny_gguf();
    let observation = inspect(&bytes).expect("tiny GGUF parses");
    assert_eq!(observation.byte_size(), bytes.len() as u64);
    assert_eq!(observation.byte_digest(), &Digest::sha256(&bytes));
    assert_eq!(observation.metadata_count(), 9);
    assert_eq!(observation.tensor_count(), 2);
    assert_eq!(
        observation
            .component_digests()
            .model_configuration()
            .as_str(),
        "cca5ad5b78579ad2fea89b019a34e123d7263efa0a961f8f5838e3a9d4f3ba80"
    );
    assert_eq!(
        observation.component_digests().tokenizer().as_str(),
        "e6fe394d2f2e98932bdde3512c66c64b25e94a6a95402a99d5413217e4bac927"
    );
    assert_eq!(
        observation.component_digests().prompt_template().as_str(),
        "6f28f1c0f2a6b1ed72e9d669e908bd74ecfbc10a365f909d108bca79d9cea45f"
    );
    assert_eq!(
        observation.component_digests().prompt_template(),
        &Digest::sha256(b"{{ .Messages }}")
    );
}

#[test]
fn model_configuration_commits_effective_defaults_and_only_selected_keys() {
    let explicit = base_metadata();
    let mut defaulted = explicit.clone();
    defaulted.retain(|(key, _value)| key != b"general.alignment");
    let mut unrelated = defaulted.clone();
    unrelated.push((
        b"general.name".to_vec(),
        MetadataValue::String(b"display-only".to_vec()),
    ));
    let explicit =
        inspect(&build_gguf(&explicit, &base_tensors(), 64)).expect("explicit alignment parses");
    let defaulted =
        inspect(&build_gguf(&defaulted, &base_tensors(), 64)).expect("default alignment parses");
    let unrelated =
        inspect(&build_gguf(&unrelated, &base_tensors(), 64)).expect("unrelated metadata parses");
    assert_eq!(
        explicit.component_digests().model_configuration(),
        defaulted.component_digests().model_configuration()
    );
    assert_eq!(
        defaulted.component_digests().model_configuration(),
        unrelated.component_digests().model_configuration()
    );
}

#[test]
fn model_configuration_requires_foundational_keys_and_selected_architecture_state() {
    let base = base_metadata();
    let mut changed = base.clone();
    let block_count = changed
        .iter_mut()
        .find(|(key, _value)| key == b"qwen3.block_count")
        .expect("architecture field");
    block_count.1 = MetadataValue::U32(29);
    let first = inspect(&build_gguf(&base, &base_tensors(), 64)).expect("base parses");
    let second = inspect(&build_gguf(&changed, &base_tensors(), 64)).expect("changed parses");
    assert_ne!(
        first.component_digests().model_configuration(),
        second.component_digests().model_configuration()
    );

    for required in [
        b"general.architecture".as_slice(),
        b"general.file_type".as_slice(),
        b"general.quantization_version".as_slice(),
        b"general.parameter_count".as_slice(),
    ] {
        let mut missing = base.clone();
        missing.retain(|(key, _value)| key != required);
        assert_eq!(
            inspect(&build_gguf(&missing, &base_tensors(), 64)),
            Err(ReconstructionError::UnsupportedGguf)
        );
    }
}

#[test]
fn canonical_component_digests_do_not_depend_on_metadata_order() {
    let metadata = base_metadata();
    let mut reversed = metadata.clone();
    reversed.reverse();
    let first = build_gguf(&metadata, &base_tensors(), 64);
    let second = build_gguf(&reversed, &base_tensors(), 64);
    assert_ne!(Digest::sha256(&first), Digest::sha256(&second));
    assert_eq!(
        inspect(&first)
            .expect("first order parses")
            .component_digests(),
        inspect(&second)
            .expect("second order parses")
            .component_digests()
    );
}

#[test]
fn all_supported_scalar_metadata_types_are_consumed_structurally() {
    let mut metadata = base_metadata();
    for (index, (kind, bytes)) in [
        (0, vec![1]),
        (1, vec![255]),
        (2, 2_u16.to_le_bytes().to_vec()),
        (3, (-2_i16).to_le_bytes().to_vec()),
        (5, (-3_i32).to_le_bytes().to_vec()),
        (6, 1.5_f32.to_le_bytes().to_vec()),
        (7, vec![1]),
        (10, 4_u64.to_le_bytes().to_vec()),
        (11, (-4_i64).to_le_bytes().to_vec()),
        (12, 2.5_f64.to_le_bytes().to_vec()),
    ]
    .into_iter()
    .enumerate()
    {
        metadata.push((
            format!("fixture.scalar{index}").into_bytes(),
            MetadataValue::Raw { kind, bytes },
        ));
    }
    assert!(inspect(&build_gguf(&metadata, &base_tensors(), 64)).is_ok());
}

#[test]
fn every_selected_truncation_is_rejected_without_panicking() {
    let bytes = tiny_gguf();
    for length in [0, 3, 8, 23, 64, bytes.len() - 1] {
        let truncated = &bytes[..length];
        assert_eq!(
            inspect_gguf_v3(
                &mut Cursor::new(truncated),
                &Digest::sha256(&bytes),
                bytes.len() as u64,
                &GgufLimits::default(),
                &mut || false
            ),
            Err(ReconstructionError::BlobSizeMismatch),
            "accepted prefix length {length}"
        );
    }
    let mut extended = bytes.clone();
    extended.push(0);
    assert_eq!(
        inspect_gguf_v3(
            &mut Cursor::new(&extended),
            &Digest::sha256(&bytes),
            bytes.len() as u64,
            &GgufLimits::default(),
            &mut || false
        ),
        Err(ReconstructionError::BlobSizeMismatch)
    );
}

#[test]
fn package_fixture_keeps_all_exact_blob_bytes_available() {
    let fixture = package_fixture();
    assert_eq!(fixture.blobs.len(), 5);
    assert!(!fixture.manifest.is_empty());
}

#[test]
fn count_string_array_and_table_limits_fail_closed() {
    let bytes = tiny_gguf();
    for limits in [
        GgufLimits {
            metadata_entries: 4,
            ..GgufLimits::default()
        },
        GgufLimits {
            tensors: 1,
            ..GgufLimits::default()
        },
        GgufLimits {
            name_bytes: 3,
            ..GgufLimits::default()
        },
        GgufLimits {
            string_bytes: 2,
            ..GgufLimits::default()
        },
        GgufLimits {
            array_elements: 1,
            ..GgufLimits::default()
        },
        GgufLimits {
            total_array_elements: 1,
            ..GgufLimits::default()
        },
        GgufLimits {
            table_bytes: 24,
            ..GgufLimits::default()
        },
    ] {
        assert_eq!(
            inspect_gguf_v3(
                &mut Cursor::new(&bytes),
                &Digest::sha256(&bytes),
                bytes.len() as u64,
                &limits,
                &mut || false
            ),
            Err(ReconstructionError::LimitExceeded)
        );
    }
}

#[test]
fn invalid_utf8_duplicates_types_dimensions_and_overflow_are_rejected() {
    let mut cases = Vec::new();
    let mut metadata = base_metadata();
    metadata[0].0 = vec![0xff];
    cases.push(build_gguf(&metadata, &base_tensors(), 64));

    let mut metadata = base_metadata();
    metadata[0].1 = MetadataValue::String(vec![0xff]);
    cases.push(build_gguf(&metadata, &base_tensors(), 64));

    let mut metadata = base_metadata();
    metadata.push(metadata[0].clone());
    cases.push(build_gguf(&metadata, &base_tensors(), 64));

    let mut metadata = base_metadata();
    metadata[0].1 = MetadataValue::Raw {
        kind: 99,
        bytes: Vec::new(),
    };
    cases.push(build_gguf(&metadata, &base_tensors(), 64));

    let mut metadata = base_metadata();
    metadata[0].1 = MetadataValue::Raw {
        kind: 7,
        bytes: vec![2],
    };
    cases.push(build_gguf(&metadata, &base_tensors(), 64));

    let mut metadata = base_metadata();
    let mut nested_array = Vec::new();
    nested_array.extend_from_slice(&9_u32.to_le_bytes());
    nested_array.extend_from_slice(&0_u64.to_le_bytes());
    metadata[0].1 = MetadataValue::Raw {
        kind: 9,
        bytes: nested_array,
    };
    cases.push(build_gguf(&metadata, &base_tensors(), 64));

    let mut tensors = base_tensors();
    tensors[1].name = tensors[0].name.clone();
    cases.push(build_gguf(&base_metadata(), &tensors, 64));

    let mut tensors = base_tensors();
    tensors[0].dimensions.clear();
    cases.push(build_gguf(&base_metadata(), &tensors, 64));

    let mut tensors = base_tensors();
    tensors[0].kind = 4;
    cases.push(build_gguf(&base_metadata(), &tensors, 64));

    let tensors = vec![Tensor {
        name: b"overflow".to_vec(),
        dimensions: vec![u64::MAX, 2],
        kind: 0,
        offset: 0,
    }];
    cases.push(build_gguf(&base_metadata(), &tensors, 32));

    for bytes in cases {
        assert!(inspect(&bytes).is_err());
    }
}

#[test]
fn absolute_limits_cannot_be_relaxed_by_a_caller() {
    let bytes = tiny_gguf();
    let excessive = GgufLimits {
        tensors: GgufLimits::default().tensors + 1,
        ..GgufLimits::default()
    };
    assert_eq!(
        inspect_gguf_v3(
            &mut Cursor::new(&bytes),
            &Digest::sha256(&bytes),
            bytes.len() as u64,
            &excessive,
            &mut || false
        ),
        Err(ReconstructionError::LimitExceeded)
    );
    assert_eq!(
        inspect_gguf_v3(
            &mut Cursor::new(&bytes),
            &Digest::sha256(&bytes),
            u64::MAX,
            &GgufLimits::default(),
            &mut || false
        ),
        Err(ReconstructionError::LimitExceeded)
    );
}

#[test]
fn cancellation_digest_drift_and_stream_error_are_distinct() {
    let bytes = tiny_gguf();
    assert_eq!(
        inspect_gguf_v3(
            &mut Cursor::new(&bytes),
            &Digest::sha256(&bytes),
            bytes.len() as u64,
            &GgufLimits::default(),
            &mut || true
        ),
        Err(ReconstructionError::Cancelled)
    );
    assert_eq!(
        inspect_gguf_v3(
            &mut Cursor::new(&bytes),
            &Digest::sha256(b"different"),
            bytes.len() as u64,
            &GgufLimits::default(),
            &mut || false
        ),
        Err(ReconstructionError::BlobDigestMismatch)
    );

    assert_eq!(
        inspect_gguf_v3(
            &mut Broken,
            &Digest::sha256(&bytes),
            bytes.len() as u64,
            &GgufLimits::default(),
            &mut || false
        ),
        Err(ReconstructionError::InputRead)
    );
    assert!(
        !ReconstructionError::InputRead
            .to_string()
            .contains("sensitive")
    );
}
