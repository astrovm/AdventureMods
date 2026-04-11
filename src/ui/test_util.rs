use std::sync::Once;

pub fn init_resource_overlay() {
    static INIT: Once = Once::new();

    INIT.call_once(|| unsafe {
        std::env::set_var(
            "G_RESOURCE_OVERLAYS",
            concat!(
                "/io/github/astrovm/AdventureMods=",
                env!("CARGO_MANIFEST_DIR"),
                "/data"
            ),
        );
    });
}
