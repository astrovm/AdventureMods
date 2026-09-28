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

/// Keep the GSettings that GTK widgets open on their own, such as the file
/// chooser's, in memory. GIO picks the default backend once per process, so
/// this must run before the first one is created.
pub fn use_memory_gsettings_backend() {
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let _env = crate::test_env::lock();
        unsafe { std::env::set_var("GSETTINGS_BACKEND", "memory") };
    });
}
