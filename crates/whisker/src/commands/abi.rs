use clawless::prelude::*;

use crate::custom_lints::AbiTag;

/// Print the tag that names the prebuilt lints this whisker can load
#[derive(Debug, Args)]
pub struct AbiArgs {}

#[command]
pub async fn abi(_args: AbiArgs, context: Context) -> CommandResult {
    context
        .output()
        .artifact(AbiTag::host().to_string())
        .await
        .context("failed to report the ABI tag")?;

    Ok(())
}
