use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Subcommand};
use rewrite_app::{
    ArtifactImportLimits, ArtifactInstallationKey, ArtifactInventoryLimits,
    ArtifactReconciliationLimits, ArtifactRemovalLimits, ArtifactRepository,
    OfflineArtifactImportRequest,
};
use rewrite_types::CancellationToken;

use crate::contract::{
    ArtifactIdArgument, CommandName, InstallationGeneration, MAX_MANIFEST_BYTES,
    read_manifest_bounded,
};

mod error;
mod output;

pub(crate) use error::ModelFailure;
pub(crate) use output::ModelOutput;

const MAXIMUM_ARTIFACT_BYTES: u64 = 256 * 1024 * 1024 * 1024;
const MAXIMUM_STORAGE_ENTRIES: usize = 4_096;
const MAXIMUM_STATE_ENTRIES: usize = 4_096;
const MAXIMUM_TOTAL_VERIFICATION_BYTES: u64 = 512 * 1024 * 1024 * 1024;

/// Offline managed-model operations.
#[derive(Debug, Subcommand)]
pub(crate) enum ModelCommand {
    /// Import one exact local artifact file without activation.
    Import(ImportArgs),
    /// Inspect managed artifact state and bytes without mutation.
    Inventory(InventoryArgs),
    /// List exact interrupted operations without reading model bytes.
    PendingOperations,
    /// Register one exact already-managed orphan without changing bytes.
    Reconcile(ReconcileArgs),
    /// Remove one exact inactive installation generation.
    Remove(RemoveArgs),
    /// Forward-complete one exact prepared removal.
    RecoverRemoval(RecoveryArgs),
}

/// Inputs for one offline artifact import.
#[derive(Debug, Args)]
pub(crate) struct ImportArgs {
    /// Local regular artifact file opened read-only and never modified.
    #[arg(value_name = "SOURCE")]
    source: PathBuf,
    /// Strict JSON manifest with exact artifact digest, size, and metadata.
    #[arg(long, value_name = "MANIFEST_JSON")]
    manifest: PathBuf,
}

/// Inputs for read-only managed artifact inventory.
#[derive(Debug, Args)]
pub(crate) struct InventoryArgs {
    /// Return exit code 3 after emitting the complete report when findings exist.
    #[arg(long)]
    fail_on_findings: bool,
}

/// Inputs for selected orphan state reconciliation.
#[derive(Debug, Args)]
pub(crate) struct ReconcileArgs {
    /// Strict JSON manifest selecting the exact canonical managed artifact.
    #[arg(long, value_name = "MANIFEST_JSON")]
    manifest: PathBuf,
}

/// Inputs for a confirmed exact installation removal.
#[derive(Debug, Args)]
pub(crate) struct RemoveArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    /// Confirm deletion of the selected inactive managed artifact bytes.
    #[arg(long)]
    yes: bool,
}

/// Inputs for confirmed forward recovery of one exact prepared removal.
#[derive(Debug, Args)]
pub(crate) struct RecoveryArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    /// Confirm forward deletion or absence confirmation for the prepared bytes.
    #[arg(long)]
    yes: bool,
}

/// Exact content identity and installation generation.
#[derive(Debug, Args)]
pub(crate) struct SelectionArgs {
    /// Canonical lowercase SHA-256 artifact identity.
    #[arg(long, value_name = "ARTIFACT_ID")]
    artifact_id: ArtifactIdArgument,
    /// Positive installation generation returned by import, inventory, or reconcile.
    #[arg(long, value_name = "GENERATION")]
    installation_generation: InstallationGeneration,
}

/// Complete successful model command result.
pub(crate) struct ModelSuccess {
    pub(crate) output: ModelOutput,
    pub(crate) exit_code: ExitCode,
}

pub(crate) fn run(
    command: ModelCommand,
    data_directory: PathBuf,
    cancellation: &CancellationToken,
) -> Result<ModelSuccess, ModelFailure> {
    let command_name = command.name();
    let repository = ArtifactRepository::new(data_directory)
        .map_err(|error| ModelFailure::repository(command_name, &error))?;
    match command {
        ModelCommand::Import(args) => import(&repository, args, cancellation),
        ModelCommand::Inventory(args) => inventory(&repository, &args, cancellation),
        ModelCommand::PendingOperations => pending_operations(&repository, cancellation),
        ModelCommand::Reconcile(args) => reconcile(&repository, &args, cancellation),
        ModelCommand::Remove(args) => remove(&repository, &args, cancellation),
        ModelCommand::RecoverRemoval(args) => recover_removal(&repository, &args),
    }
}

impl ModelCommand {
    pub(crate) const fn name(&self) -> CommandName {
        match self {
            Self::Import(_) => CommandName::ModelImport,
            Self::Inventory(_) => CommandName::ModelInventory,
            Self::PendingOperations => CommandName::ModelPendingOperations,
            Self::Reconcile(_) => CommandName::ModelReconcile,
            Self::Remove(_) => CommandName::ModelRemove,
            Self::RecoverRemoval(_) => CommandName::ModelRecoverRemoval,
        }
    }
}

fn pending_operations(
    repository: &ArtifactRepository,
    cancellation: &CancellationToken,
) -> Result<ModelSuccess, ModelFailure> {
    let result = repository
        .pending_operations(MAXIMUM_STATE_ENTRIES, cancellation)
        .map_err(|error| ModelFailure::repository(CommandName::ModelPendingOperations, &error))?;
    Ok(ModelSuccess {
        output: ModelOutput::pending_operations(&result),
        exit_code: ExitCode::SUCCESS,
    })
}

fn import(
    repository: &ArtifactRepository,
    args: ImportArgs,
    cancellation: &CancellationToken,
) -> Result<ModelSuccess, ModelFailure> {
    let manifest = read_manifest_bounded(&args.manifest, MAX_MANIFEST_BYTES)
        .map_err(|error| ModelFailure::manifest(CommandName::ModelImport, error))?;
    let result = repository
        .import(
            &OfflineArtifactImportRequest {
                source: args.source,
                manifest,
            },
            import_limits(),
            cancellation,
        )
        .map_err(|error| ModelFailure::repository(CommandName::ModelImport, &error))?;
    Ok(ModelSuccess {
        output: ModelOutput::import(&result),
        exit_code: ExitCode::SUCCESS,
    })
}

fn inventory(
    repository: &ArtifactRepository,
    args: &InventoryArgs,
    cancellation: &CancellationToken,
) -> Result<ModelSuccess, ModelFailure> {
    let report = repository
        .inventory(inventory_limits(), cancellation)
        .map_err(|error| ModelFailure::repository(CommandName::ModelInventory, &error))?;
    let output = ModelOutput::inventory(report);
    let exit_code = if args.fail_on_findings && output.has_findings() {
        ExitCode::from(crate::contract::EXIT_POLICY)
    } else {
        ExitCode::SUCCESS
    };
    Ok(ModelSuccess { output, exit_code })
}

fn reconcile(
    repository: &ArtifactRepository,
    args: &ReconcileArgs,
    cancellation: &CancellationToken,
) -> Result<ModelSuccess, ModelFailure> {
    let manifest = read_manifest_bounded(&args.manifest, MAX_MANIFEST_BYTES)
        .map_err(|error| ModelFailure::manifest(CommandName::ModelReconcile, error))?;
    let result = repository
        .reconcile(manifest, reconciliation_limits(), cancellation)
        .map_err(|error| ModelFailure::repository(CommandName::ModelReconcile, &error))?;
    Ok(ModelSuccess {
        output: ModelOutput::reconcile(&result),
        exit_code: ExitCode::SUCCESS,
    })
}

fn remove(
    repository: &ArtifactRepository,
    args: &RemoveArgs,
    cancellation: &CancellationToken,
) -> Result<ModelSuccess, ModelFailure> {
    if !args.yes {
        return Err(ModelFailure::confirmation_required());
    }
    let key = args.selection.to_key(CommandName::ModelRemove)?;
    let result = repository
        .remove(&key, removal_limits(), cancellation)
        .map_err(|error| ModelFailure::repository(CommandName::ModelRemove, &error))?;
    Ok(ModelSuccess {
        output: ModelOutput::remove(&result),
        exit_code: ExitCode::SUCCESS,
    })
}

fn recover_removal(
    repository: &ArtifactRepository,
    args: &RecoveryArgs,
) -> Result<ModelSuccess, ModelFailure> {
    if !args.yes {
        return Err(ModelFailure::recovery_confirmation_required());
    }
    let key = args.selection.to_key(CommandName::ModelRecoverRemoval)?;
    let result = repository
        .recover_removal(&key, removal_limits())
        .map_err(|error| ModelFailure::repository(CommandName::ModelRecoverRemoval, &error))?;
    Ok(ModelSuccess {
        output: ModelOutput::remove(&result),
        exit_code: ExitCode::SUCCESS,
    })
}

impl SelectionArgs {
    fn to_key(&self, command: CommandName) -> Result<ArtifactInstallationKey, ModelFailure> {
        ArtifactInstallationKey::new(
            self.artifact_id.to_artifact_id(),
            self.installation_generation.get(),
        )
        .map_err(|error| ModelFailure::repository(command, &error))
    }
}

const fn import_limits() -> ArtifactImportLimits {
    ArtifactImportLimits {
        maximum_artifact_bytes: MAXIMUM_ARTIFACT_BYTES,
        maximum_storage_entries: MAXIMUM_STORAGE_ENTRIES,
    }
}

const fn reconciliation_limits() -> ArtifactReconciliationLimits {
    ArtifactReconciliationLimits {
        maximum_artifact_bytes: MAXIMUM_ARTIFACT_BYTES,
        maximum_storage_entries: MAXIMUM_STORAGE_ENTRIES,
    }
}

const fn removal_limits() -> ArtifactRemovalLimits {
    ArtifactRemovalLimits {
        maximum_artifact_bytes: MAXIMUM_ARTIFACT_BYTES,
        maximum_storage_entries: MAXIMUM_STORAGE_ENTRIES,
    }
}

const fn inventory_limits() -> ArtifactInventoryLimits {
    ArtifactInventoryLimits {
        maximum_state_entries: MAXIMUM_STATE_ENTRIES,
        maximum_storage_entries: MAXIMUM_STORAGE_ENTRIES,
        maximum_artifact_bytes: MAXIMUM_ARTIFACT_BYTES,
        maximum_total_verification_bytes: MAXIMUM_TOTAL_VERIFICATION_BYTES,
    }
}
