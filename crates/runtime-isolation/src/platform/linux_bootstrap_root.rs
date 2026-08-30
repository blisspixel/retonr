use rewrite_types::Digest;

use super::{
    linux_bootstrap_protocol::BootstrapRequest, linux_build_protocol::InputDeclaration,
    linux_helper_setup::HelperFailure,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootPreparationPhase {
    Declared,
    InputSnapshotMeasured,
    AlpineMinirootfsExtracted,
    OfflinePackagesInstalled,
    RustToolchainAssembled,
    ReadOnlyMountsEstablished,
    RootPivoted,
    OldRootDetached,
    CanariesPassed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RootPreparationStep {
    MeasureInputSnapshot,
    ExtractAlpineMinirootfs,
    InstallOfflinePackages,
    AssembleRustToolchain,
    EstablishReadOnlyMounts,
    PivotRoot,
    DetachOldRoot,
    PassCanaries,
}

const REQUIRED_STEPS: [RootPreparationStep; 8] = [
    RootPreparationStep::MeasureInputSnapshot,
    RootPreparationStep::ExtractAlpineMinirootfs,
    RootPreparationStep::InstallOfflinePackages,
    RootPreparationStep::AssembleRustToolchain,
    RootPreparationStep::EstablishReadOnlyMounts,
    RootPreparationStep::PivotRoot,
    RootPreparationStep::DetachOldRoot,
    RootPreparationStep::PassCanaries,
];

#[derive(Debug)]
pub(super) struct RootPreparation {
    phase: RootPreparationPhase,
    authenticated_busybox_executable: Digest,
}

impl RootPreparation {
    pub(super) fn begin(
        request: &BootstrapRequest,
        declarations: &[InputDeclaration],
    ) -> Result<Self, HelperFailure> {
        if !request.joins(declarations) {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        request
            .busybox_package()
            .ok_or(HelperFailure::BootstrapRootVerification)?;
        Ok(Self {
            phase: RootPreparationPhase::Declared,
            authenticated_busybox_executable: request.busybox_executable().digest.clone(),
        })
    }

    pub(super) fn advance(&mut self, step: RootPreparationStep) -> Result<(), HelperFailure> {
        if REQUIRED_STEPS.get(self.completed_steps()).copied() != Some(step) {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        self.phase = match step {
            RootPreparationStep::MeasureInputSnapshot => {
                RootPreparationPhase::InputSnapshotMeasured
            }
            RootPreparationStep::ExtractAlpineMinirootfs => {
                RootPreparationPhase::AlpineMinirootfsExtracted
            }
            RootPreparationStep::InstallOfflinePackages => {
                RootPreparationPhase::OfflinePackagesInstalled
            }
            RootPreparationStep::AssembleRustToolchain => {
                RootPreparationPhase::RustToolchainAssembled
            }
            RootPreparationStep::EstablishReadOnlyMounts => {
                RootPreparationPhase::ReadOnlyMountsEstablished
            }
            RootPreparationStep::PivotRoot => RootPreparationPhase::RootPivoted,
            RootPreparationStep::DetachOldRoot => RootPreparationPhase::OldRootDetached,
            RootPreparationStep::PassCanaries => RootPreparationPhase::CanariesPassed,
        };
        Ok(())
    }

    pub(super) fn measure_input_snapshot(
        &mut self,
        busybox_executable: &Digest,
    ) -> Result<(), HelperFailure> {
        if &self.authenticated_busybox_executable != busybox_executable {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        self.advance(RootPreparationStep::MeasureInputSnapshot)
    }

    const fn completed_steps(&self) -> usize {
        match self.phase {
            RootPreparationPhase::Declared => 0,
            RootPreparationPhase::InputSnapshotMeasured => 1,
            RootPreparationPhase::AlpineMinirootfsExtracted => 2,
            RootPreparationPhase::OfflinePackagesInstalled => 3,
            RootPreparationPhase::RustToolchainAssembled => 4,
            RootPreparationPhase::ReadOnlyMountsEstablished => 5,
            RootPreparationPhase::RootPivoted => 6,
            RootPreparationPhase::OldRootDetached => 7,
            RootPreparationPhase::CanariesPassed => 8,
        }
    }

    #[cfg(test)]
    fn is_complete(&self) -> bool {
        self.phase == RootPreparationPhase::CanariesPassed
    }
}

pub(super) fn execute(
    request: &BootstrapRequest,
    declarations: &[InputDeclaration],
    helper: &std::fs::File,
) -> Result<crate::contract::RetainedProgramBootstrapRootObservation, HelperFailure> {
    let mut preparation = RootPreparation::begin(request, declarations)?;
    preparation.measure_input_snapshot(&request.busybox_executable().digest)?;
    super::linux_bootstrap_root_native::execute(&mut preparation, helper)
}

#[cfg(test)]
mod tests {
    use crate::{
        RetainedProgramBootstrapAttempt, RetainedProgramBootstrapInputKind,
        platform::linux_bootstrap_protocol::{BootstrapExecutable, BootstrapSignedInput},
    };

    use super::*;

    fn request_and_declarations() -> (BootstrapRequest, Vec<InputDeclaration>) {
        let signed_inputs = RetainedProgramBootstrapInputKind::ALL
            .into_iter()
            .map(|kind| BootstrapSignedInput {
                kind,
                digest: Digest::sha256(kind.relative_path().as_bytes()),
                byte_size: 1,
            })
            .collect::<Vec<_>>();
        let mut declarations = vec![InputDeclaration {
            relative_path: "lineage/build-recipe-v2.json".to_owned(),
            expected_digest: Digest::sha256(b"recipe"),
            expected_bytes: 6,
        }];
        declarations.extend(signed_inputs.iter().map(|input| InputDeclaration {
            relative_path: input.kind.relative_path().to_owned(),
            expected_digest: input.digest.clone(),
            expected_bytes: input.byte_size,
        }));
        declarations.push(InputDeclaration {
            relative_path: "toolchains/busybox".to_owned(),
            expected_digest: Digest::sha256(b"busybox executable"),
            expected_bytes: 18,
        });
        (
            BootstrapRequest {
                attempt: RetainedProgramBootstrapAttempt::Primary,
                input_count: declarations.len(),
                launch_digest: Digest::sha256(b"launch"),
                recipe_digest: Digest::sha256(b"recipe"),
                recipe_bytes: 6,
                busybox_executable: BootstrapExecutable {
                    digest: Digest::sha256(b"busybox executable"),
                    byte_size: 18,
                },
                signed_inputs,
            },
            declarations,
        )
    }

    #[test]
    fn root_transition_order_is_exact_and_complete() {
        let (request, declarations) = request_and_declarations();
        let mut preparation = RootPreparation::begin(&request, &declarations).expect("begin");
        preparation
            .measure_input_snapshot(&request.busybox_executable().digest)
            .expect("measurement");
        for step in [
            RootPreparationStep::ExtractAlpineMinirootfs,
            RootPreparationStep::InstallOfflinePackages,
            RootPreparationStep::AssembleRustToolchain,
            RootPreparationStep::EstablishReadOnlyMounts,
            RootPreparationStep::PivotRoot,
            RootPreparationStep::DetachOldRoot,
            RootPreparationStep::PassCanaries,
        ] {
            preparation.advance(step).expect("ordered step");
        }
        assert!(preparation.is_complete());
    }

    #[test]
    fn skipped_reordered_and_partial_transitions_fail_closed() {
        let (request, declarations) = request_and_declarations();
        let mut preparation = RootPreparation::begin(&request, &declarations).expect("begin");
        assert_eq!(
            preparation.advance(RootPreparationStep::InstallOfflinePackages),
            Err(HelperFailure::BootstrapRootVerification)
        );
        preparation
            .measure_input_snapshot(&request.busybox_executable().digest)
            .expect("measurement");
        assert!(!preparation.is_complete());
    }

    #[test]
    fn authenticated_busybox_package_is_mandatory() {
        let (mut request, declarations) = request_and_declarations();
        request.signed_inputs.retain(|input| {
            input.kind != RetainedProgramBootstrapInputKind::AlpineBusyboxStaticPackage
        });
        assert_eq!(
            RootPreparation::begin(&request, &declarations).map(|_| ()),
            Err(HelperFailure::BootstrapRootVerification)
        );
    }
}
