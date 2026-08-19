use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Subcommand};
use rewrite_app::{
    ArtifactImportLimits, ArtifactInstallationKey, ArtifactInventoryLimits,
    ArtifactReconciliationLimits, ArtifactRemovalLimits, ArtifactRepository,
    ArtifactRepositoryMigrationLimits, ArtifactSetImportLimits, ArtifactSetInventoryLimits,
    ArtifactSetReconciliationLimits, ArtifactSetRemovalLimits, OfflineArtifactImportRequest,
    OfflineArtifactSetImportRequest,
};
use rewrite_model::MAX_ARTIFACT_SET_MEMBERS;
use rewrite_types::CancellationToken;

use crate::contract::{
    ArtifactIdArgument, CommandName, InstallationGeneration, MAX_MANIFEST_BYTES,
    read_manifest_bounded, read_set_manifest_bounded,
};

mod error;
mod output;

pub(crate) use error::ModelFailure;
pub(crate) use output::ModelOutput;

const MAXIMUM_ARTIFACT_BYTES: u64 = 256 * 1024 * 1024 * 1024;
const MAXIMUM_STORAGE_ENTRIES: usize = 4_096;
const MAXIMUM_STATE_ENTRIES: usize = 4_096;
const MAXIMUM_TOTAL_VERIFICATION_BYTES: u64 = 512 * 1024 * 1024 * 1024;
const MAXIMUM_MIGRATION_STATE_BYTES: u64 = 1024 * 1024 * 1024;
const MAXIMUM_MIGRATION_REPOSITORY_ENTRIES: usize = 4_096;
const MAXIMUM_SET_TREE_ENTRIES: usize = 8_192;

/// Offline managed-model operations.
#[derive(Debug, Subcommand)]
pub(crate) enum ModelCommand {
    /// Import one exact local artifact file without activation.
    Import(ImportArgs),
    /// Import one exact local artifact-set folder without activation.
    ImportSet(ImportSetArgs),
    /// Inspect managed artifact state and bytes without mutation.
    Inventory(InventoryArgs),
    /// Inspect managed artifact-set state and trees without mutation.
    InventorySet(InventoryArgs),
    /// List exact interrupted operations without reading model bytes.
    PendingOperations,
    /// Explicitly migrate an existing repository after retaining a verified backup.
    Migrate(MigrationArgs),
    /// Register one exact already-managed orphan without changing bytes.
    Reconcile(ReconcileArgs),
    /// Register one exact already-managed set root without changing bytes.
    ReconcileSet(ReconcileSetArgs),
    /// Remove one exact inactive installation generation.
    Remove(RemoveArgs),
    /// Forward-complete one exact prepared removal.
    RecoverRemoval(RecoveryArgs),
    /// Remove one exact inactive artifact-set installation generation.
    RemoveSet(RemoveSetArgs),
    /// Forward-complete one exact prepared artifact-set removal.
    RecoverSetRemoval(RecoverSetArgs),
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

/// Inputs for one offline artifact-set folder import.
#[derive(Debug, Args)]
pub(crate) struct ImportSetArgs {
    /// Local source directory opened read-only and never modified.
    #[arg(value_name = "SOURCE_ROOT")]
    source_root: PathBuf,
    /// Strict JSON manifest with exact member paths, digests, and sizes.
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

/// Confirmation for an explicit repository schema migration.
#[derive(Debug, Args)]
pub(crate) struct MigrationArgs {
    /// Confirm the forward migration and retained repository backup.
    #[arg(long)]
    yes: bool,
}

/// Inputs for selected orphan state reconciliation.
#[derive(Debug, Args)]
pub(crate) struct ReconcileArgs {
    /// Strict JSON manifest selecting the exact canonical managed artifact.
    #[arg(long, value_name = "MANIFEST_JSON")]
    manifest: PathBuf,
}

/// Inputs for selected set-root state reconciliation.
#[derive(Debug, Args)]
pub(crate) struct ReconcileSetArgs {
    /// Strict JSON set manifest selecting the exact canonical managed set root.
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

/// Inputs for a confirmed exact artifact-set installation removal.
#[derive(Debug, Args)]
pub(crate) struct RemoveSetArgs {
    #[command(flatten)]
    selection: SetSelectionArgs,
    /// Confirm deletion of the selected inactive managed set tree.
    #[arg(long)]
    yes: bool,
}

/// Inputs for confirmed forward recovery of one exact prepared set removal.
#[derive(Debug, Args)]
pub(crate) struct RecoverSetArgs {
    #[command(flatten)]
    selection: SetSelectionArgs,
    /// Confirm forward deletion or absence confirmation for the prepared set tree.
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

/// Exact set identity and installation generation.
#[derive(Debug, Args)]
pub(crate) struct SetSelectionArgs {
    /// Canonical lowercase SHA-256 artifact-set identity.
    #[arg(long, value_name = "ARTIFACT_SET_ID")]
    artifact_set_id: ArtifactIdArgument,
    /// Positive installation generation returned by import-set, inventory-set, or reconcile-set.
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
        ModelCommand::ImportSet(args) => import_set(&repository, args, cancellation),
        ModelCommand::Inventory(args) => inventory(&repository, &args, cancellation),
        ModelCommand::InventorySet(args) => inventory_set(&repository, &args, cancellation),
        ModelCommand::PendingOperations => pending_operations(&repository, cancellation),
        ModelCommand::Migrate(args) => migrate(&repository, &args, cancellation),
        ModelCommand::Reconcile(args) => reconcile(&repository, &args, cancellation),
        ModelCommand::ReconcileSet(args) => reconcile_set(&repository, &args, cancellation),
        ModelCommand::Remove(args) => remove(&repository, &args, cancellation),
        ModelCommand::RecoverRemoval(args) => recover_removal(&repository, &args),
        ModelCommand::RemoveSet(args) => remove_set(&repository, &args, cancellation),
        ModelCommand::RecoverSetRemoval(args) => recover_set_removal(&repository, &args),
    }
}

impl ModelCommand {
    pub(crate) const fn name(&self) -> CommandName {
        match self {
            Self::Import(_) => CommandName::ModelImport,
            Self::ImportSet(_) => CommandName::ModelImportSet,
            Self::Inventory(_) => CommandName::ModelInventory,
            Self::InventorySet(_) => CommandName::ModelInventorySet,
            Self::PendingOperations => CommandName::ModelPendingOperations,
            Self::Migrate(_) => CommandName::ModelMigrate,
            Self::Reconcile(_) => CommandName::ModelReconcile,
            Self::ReconcileSet(_) => CommandName::ModelReconcileSet,
            Self::Remove(_) => CommandName::ModelRemove,
            Self::RecoverRemoval(_) => CommandName::ModelRecoverRemoval,
            Self::RemoveSet(_) => CommandName::ModelRemoveSet,
            Self::RecoverSetRemoval(_) => CommandName::ModelRecoverSetRemoval,
        }
    }
}

fn migrate(
    repository: &ArtifactRepository,
    args: &MigrationArgs,
    cancellation: &CancellationToken,
) -> Result<ModelSuccess, ModelFailure> {
    if !args.yes {
        return Err(ModelFailure::migration_confirmation_required());
    }
    let result = repository
        .migrate(migration_limits(), cancellation)
        .map_err(|error| ModelFailure::repository(CommandName::ModelMigrate, &error))?;
    Ok(ModelSuccess {
        output: ModelOutput::migration(&result),
        exit_code: ExitCode::SUCCESS,
    })
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

fn import_set(
    repository: &ArtifactRepository,
    args: ImportSetArgs,
    cancellation: &CancellationToken,
) -> Result<ModelSuccess, ModelFailure> {
    let manifest = read_set_manifest_bounded(&args.manifest, MAX_MANIFEST_BYTES)
        .map_err(|error| ModelFailure::manifest(CommandName::ModelImportSet, error))?;
    let result = repository
        .import_set(
            &OfflineArtifactSetImportRequest {
                source_root: args.source_root,
                manifest,
            },
            set_import_limits(),
            cancellation,
        )
        .map_err(|error| ModelFailure::repository(CommandName::ModelImportSet, &error))?;
    Ok(ModelSuccess {
        output: ModelOutput::set_import(&result),
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

fn inventory_set(
    repository: &ArtifactRepository,
    args: &InventoryArgs,
    cancellation: &CancellationToken,
) -> Result<ModelSuccess, ModelFailure> {
    let report = repository
        .inventory_set(set_inventory_limits(), cancellation)
        .map_err(|error| ModelFailure::repository(CommandName::ModelInventorySet, &error))?;
    let output = ModelOutput::set_inventory(report);
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

fn reconcile_set(
    repository: &ArtifactRepository,
    args: &ReconcileSetArgs,
    cancellation: &CancellationToken,
) -> Result<ModelSuccess, ModelFailure> {
    let manifest = read_set_manifest_bounded(&args.manifest, MAX_MANIFEST_BYTES)
        .map_err(|error| ModelFailure::manifest(CommandName::ModelReconcileSet, error))?;
    let result = repository
        .reconcile_set(manifest, set_reconciliation_limits(), cancellation)
        .map_err(|error| ModelFailure::repository(CommandName::ModelReconcileSet, &error))?;
    Ok(ModelSuccess {
        output: ModelOutput::set_reconcile(&result),
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

fn remove_set(
    repository: &ArtifactRepository,
    args: &RemoveSetArgs,
    cancellation: &CancellationToken,
) -> Result<ModelSuccess, ModelFailure> {
    if !args.yes {
        return Err(ModelFailure::confirmation_required_for(
            CommandName::ModelRemoveSet,
        ));
    }
    let key = args.selection.to_key(CommandName::ModelRemoveSet)?;
    let result = repository
        .remove_set(&key, set_removal_limits(), cancellation)
        .map_err(|error| ModelFailure::repository(CommandName::ModelRemoveSet, &error))?;
    Ok(ModelSuccess {
        output: ModelOutput::set_remove(&result),
        exit_code: ExitCode::SUCCESS,
    })
}

fn recover_set_removal(
    repository: &ArtifactRepository,
    args: &RecoverSetArgs,
) -> Result<ModelSuccess, ModelFailure> {
    if !args.yes {
        return Err(ModelFailure::recovery_confirmation_required_for(
            CommandName::ModelRecoverSetRemoval,
        ));
    }
    let key = args.selection.to_key(CommandName::ModelRecoverSetRemoval)?;
    let result = repository
        .recover_set_removal(&key, set_removal_limits())
        .map_err(|error| ModelFailure::repository(CommandName::ModelRecoverSetRemoval, &error))?;
    Ok(ModelSuccess {
        output: ModelOutput::set_remove(&result),
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

impl SetSelectionArgs {
    fn to_key(
        &self,
        command: CommandName,
    ) -> Result<rewrite_app::ArtifactSetInstallationKey, ModelFailure> {
        rewrite_app::ArtifactSetInstallationKey::new(
            self.artifact_set_id.to_artifact_set_id(),
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

const fn set_import_limits() -> ArtifactSetImportLimits {
    ArtifactSetImportLimits {
        maximum_members: MAX_ARTIFACT_SET_MEMBERS,
        maximum_member_bytes: MAXIMUM_ARTIFACT_BYTES,
        maximum_total_bytes: MAXIMUM_TOTAL_VERIFICATION_BYTES,
        maximum_tree_entries: MAXIMUM_SET_TREE_ENTRIES,
        maximum_storage_entries: MAXIMUM_STORAGE_ENTRIES,
        maximum_staging_entries: MAXIMUM_STORAGE_ENTRIES,
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

const fn set_removal_limits() -> ArtifactSetRemovalLimits {
    ArtifactSetRemovalLimits {
        maximum_members: MAX_ARTIFACT_SET_MEMBERS,
        maximum_member_bytes: MAXIMUM_ARTIFACT_BYTES,
        maximum_total_bytes: MAXIMUM_TOTAL_VERIFICATION_BYTES,
        maximum_tree_entries: MAXIMUM_SET_TREE_ENTRIES,
        maximum_storage_entries: MAXIMUM_STORAGE_ENTRIES,
    }
}

const fn set_reconciliation_limits() -> ArtifactSetReconciliationLimits {
    ArtifactSetReconciliationLimits {
        maximum_members: MAX_ARTIFACT_SET_MEMBERS,
        maximum_member_bytes: MAXIMUM_ARTIFACT_BYTES,
        maximum_total_bytes: MAXIMUM_TOTAL_VERIFICATION_BYTES,
        maximum_tree_entries: MAXIMUM_SET_TREE_ENTRIES,
        maximum_storage_entries: MAXIMUM_STORAGE_ENTRIES,
    }
}

const fn set_inventory_limits() -> ArtifactSetInventoryLimits {
    ArtifactSetInventoryLimits {
        maximum_state_entries: MAXIMUM_STATE_ENTRIES,
        maximum_storage_entries: MAXIMUM_STORAGE_ENTRIES,
        maximum_members: MAX_ARTIFACT_SET_MEMBERS,
        maximum_member_bytes: MAXIMUM_ARTIFACT_BYTES,
        maximum_tree_entries: MAXIMUM_SET_TREE_ENTRIES,
        maximum_total_verification_bytes: MAXIMUM_TOTAL_VERIFICATION_BYTES,
    }
}

const fn migration_limits() -> ArtifactRepositoryMigrationLimits {
    ArtifactRepositoryMigrationLimits {
        maximum_state_bytes: MAXIMUM_MIGRATION_STATE_BYTES,
        maximum_repository_entries: MAXIMUM_MIGRATION_REPOSITORY_ENTRIES,
    }
}
