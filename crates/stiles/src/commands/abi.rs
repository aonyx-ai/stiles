use clawless::prelude::*;

use crate::custom_lints::AbiTag;

/// Print the tag that names the prebuilt lints this stiles can load
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AbiArgs>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AbiArgs>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AbiArgs>();
    }
}
