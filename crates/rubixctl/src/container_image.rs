use crate::contract::InstallOptions;
use rubix_assets::{ArchiveLimits, AssetId, DecodeSession, LayerDecodeLimits};

pub const DEFAULT_CONTAINER_IMAGE: &str = "portainer/kubesolo";

/// Resolves the container image to use based on the offline bundle or provided overrides.
///
/// If `--image` is explicitly provided, it takes precedence.
/// Otherwise, if an offline installation bundle is provided, we verify its layers
/// and extract the manifest tags to determine the exact image reference.
/// Finally, if neither is provided, we default to the well-known image and version.
pub fn select_container_image(
    options: &InstallOptions,
    offline_bundle: Option<(&mut DecodeSession<'_>, &[u8])>,
) -> Result<String, String> {
    if let Some(img) = &options.container_image
        && !img.is_empty()
    {
        return Ok(img.clone());
    }

    if let Some((session, bytes)) = offline_bundle {
        let observation = session
            .verify_crane_image_layer_digests(
                AssetId::ImageKubesolo,
                bytes,
                ArchiveLimits::default(),
                LayerDecodeLimits::default(),
            )
            .map_err(|e| format!("image payload verification failed: {e}"))?;

        let tags = observation.archive().repo_tags();
        if let Some(tag) = tags.first() {
            return Ok(tag.clone());
        }
        return Err("offline bundle does not contain any image repo tags".to_string());
    }

    Ok(format!("{}:{}", DEFAULT_CONTAINER_IMAGE, options.version))
}
