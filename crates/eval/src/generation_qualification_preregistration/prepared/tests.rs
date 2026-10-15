use super::*;

#[test]
fn negative_disposition_has_platform_precedence() {
    assert_eq!(
        disposition_from_evidence(
            GenerationQualificationPlatformStatusV1::Rejected,
            GenerationQualificationLicenseDecisionV1::Rejected,
        ),
        GenerationQualificationPreparationDisposition::PlatformRejected
    );
    assert_eq!(
        disposition_from_evidence(
            GenerationQualificationPlatformStatusV1::Supported,
            GenerationQualificationLicenseDecisionV1::Rejected,
        ),
        GenerationQualificationPreparationDisposition::LicenseRejected
    );
    assert_eq!(
        disposition_from_evidence(
            GenerationQualificationPlatformStatusV1::Supported,
            GenerationQualificationLicenseDecisionV1::LocalUseOnly,
        ),
        GenerationQualificationPreparationDisposition::TrafficEligible
    );
}
