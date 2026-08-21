use super::{
    linux_helper::{
        Mode, decode_namespace_evidence, encode_namespace_evidence, validate_mode_arguments,
    },
    linux_helper_setup::{NamespaceEvidence, RawNamespaceIdentity},
};

#[test]
fn helper_modes_reject_ambiguous_probe_arguments() {
    assert!(validate_mode_arguments(Mode::Probe, &[]).is_ok());
    assert!(validate_mode_arguments(Mode::Probe, &[std::ffi::OsString::from("extra")]).is_err());
    assert!(validate_mode_arguments(Mode::Launch, &[]).is_ok());
}

#[test]
fn namespace_evidence_codec_is_exact() {
    let evidence = NamespaceEvidence {
        network: RawNamespaceIdentity {
            device: 1,
            inode: 2,
        },
        user: RawNamespaceIdentity {
            device: 3,
            inode: 4,
        },
        process: RawNamespaceIdentity {
            device: 5,
            inode: 6,
        },
    };
    assert_eq!(
        decode_namespace_evidence(&encode_namespace_evidence(evidence)),
        Ok(evidence)
    );
    assert!(decode_namespace_evidence(&[0; 47]).is_err());
}
