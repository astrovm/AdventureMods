pub use super::sa2_catalog::RECOMMENDED_MODS;

#[cfg(test)]
mod tests {
    use super::super::common::ModSource;
    use super::*;

    crate::recommended_mods_tests!(12);

    #[test]
    fn test_sonic_new_tricks_uses_sa2_image_folder() {
        let new_tricks = RECOMMENDED_MODS
            .iter()
            .find(|m| m.name == "Sonic: New Tricks")
            .expect("Sonic: New Tricks entry missing");

        assert!(!new_tricks.pictures.is_empty());
        for picture in new_tricks.pictures {
            assert!(
                picture.starts_with(
                    "/io/github/astrovm/AdventureMods/resources/images/sa2/new_tricks/"
                ),
                "Unexpected SA2 New Tricks picture path: {}",
                picture
            );
        }
    }

    #[test]
    fn test_stage_atmosphere_tweaks_not_in_recommended_mods() {
        assert!(
            RECOMMENDED_MODS
                .iter()
                .all(|m| m.name != "Stage Atmosphere Tweaks")
        );
    }

    #[test]
    fn test_render_fix_uses_github_direct_url() {
        let mod_entry = RECOMMENDED_MODS
            .iter()
            .find(|m| m.name == "SA2 Render Fix")
            .expect("SA2 Render Fix entry missing");

        match mod_entry.source {
            ModSource::DirectUrl { url } => assert!(
                url.contains("github.com/shaddatic/sa2b-render-fix"),
                "SA2 Render Fix should use GitHub releases URL, got: {url}"
            ),
            ModSource::GameBananaItem { .. } => {
                panic!("SA2 Render Fix should use DirectUrl, not GameBananaItem")
            }
        }
    }
}
