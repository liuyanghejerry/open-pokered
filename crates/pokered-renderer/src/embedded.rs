//! Embedded assets for wasm32 targets
//!
//! This module provides access to assets embedded at compile time using `include_bytes!`.
//! For non-wasm32 targets, all functions return None/empty.

// Include the auto-generated embedded assets code
include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_embedded_assets_available() {
        // Should have at least some assets embedded
        let assets = super::list_embedded_assets();
        assert!(!assets.is_empty(), "No assets were embedded");
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_can_find_title_asset() {
        // Try to find a known asset
        let result = super::get_embedded_asset("title/pokemon_logo.png");
        assert!(result.is_some(), "Could not find title/pokemon_logo.png");
    }
}
