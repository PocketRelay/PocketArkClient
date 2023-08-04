use std::{
    fs::{copy, remove_file, write},
    io,
    path::PathBuf,
};

use native_dialog::FileDialog;
use thiserror::Error;

use crate::constants::{ANSEL_SDK64_BAK, ANSEL_SDK64_DLL};

/// Errors that can occur while patching the game
#[derive(Debug, Error)]
pub enum PatchError {
    /// The file picker failed to pick a file
    #[error("Failed to get picked file. Make sure this program is running as administrator")]
    PickFileFailed,
    /// The picked path was missing the game exe
    #[error("The path given doesn't contains the MassEffect.exe executable")]
    MissingGame,
    /// Failed to delete the bink232
    #[error("Failed to delete binkw32.dll you will have to manually unpatch your game: {0}")]
    FailedDelete(io::Error),
    /// Fialed to replace the files
    #[error("Failed to replace binkw32.dll with origin binkw23.ddl: {0}")]
    FailedReplaceOriginal(io::Error),
    /// Failed to write the patch files
    #[error("Failed to write patch file dlls (binkw32.dll and binkw32.dll): {0}")]
    FailedWritingPatchFiles(io::Error),
}

/// Attempt to use the system file picker to pick the path to the
/// Mass Effect 3 executable
fn try_pick_game_path() -> Result<Option<PathBuf>, PatchError> {
    FileDialog::new()
        .set_filename("MassEffectAndromeda.exe")
        .add_filter("Mass Effect Andromeda Executable", &["exe"])
        .show_open_single_file()
        .map_err(|_| PatchError::PickFileFailed)
}

/// Attempts to remove the patch from the provided Mass Effect
/// installation by swapping the binkw32 ddl with binkw23 and
/// deleting the old DLL
pub fn try_remove_patch() -> Result<bool, PatchError> {
    let path = match try_pick_game_path()? {
        Some(value) => value,
        None => return Ok(false),
    };
    if !path.exists() {
        return Err(PatchError::MissingGame);
    }

    let parent = path.parent().ok_or(PatchError::MissingGame)?;

    let ansel_bak = parent.join("AnselSDK64.bak");
    let ansel = parent.join("AnselSDK64.dll");

    if ansel.exists() {
        remove_file(&ansel).map_err(PatchError::FailedDelete)?;
    }

    if ansel_bak.exists() {
        copy(&ansel_bak, &ansel).map_err(PatchError::FailedReplaceOriginal)?;
        let _ = remove_file(&ansel_bak);
    } else {
        write(&ansel, ANSEL_SDK64_BAK).map_err(PatchError::FailedReplaceOriginal)?;
    }

    Ok(true)
}

/// Attempts to patch the Mass Effect installation at the provided game
/// path. Writes the two embedded DLLs to the game directory.
pub fn try_patch_game() -> Result<bool, PatchError> {
    let path = match try_pick_game_path()? {
        Some(value) => value,
        None => return Ok(false),
    };
    if !path.exists() {
        return Err(PatchError::MissingGame);
    }
    let parent = path.parent().ok_or(PatchError::MissingGame)?;

    let ansel_bak = parent.join("AnselSDK64.bak");
    let ansel = parent.join("AnselSDK64.dll");

    write(ansel_bak, ANSEL_SDK64_BAK).map_err(PatchError::FailedWritingPatchFiles)?;
    write(ansel, ANSEL_SDK64_DLL).map_err(PatchError::FailedWritingPatchFiles)?;
    Ok(true)
}
