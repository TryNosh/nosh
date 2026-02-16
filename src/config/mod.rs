mod schema;

pub use schema::Config;

/// Default cloud URL (compile-time or fallback)
const DEFAULT_CLOUD_URL: &str = "https://noshell.dev/api";

/// Get the cloud API URL.
/// - Release builds: compile-time only (set via NOSH_CLOUD_URL env during build)
/// - Debug builds: allows runtime override for development
pub fn cloud_url() -> String {
    // Compile-time URL from build environment
    let compile_time_url = option_env!("NOSH_CLOUD_URL");

    #[cfg(not(debug_assertions))]
    {
        // Release: use compile-time URL only
        compile_time_url
            .unwrap_or(DEFAULT_CLOUD_URL)
            .to_string()
    }

    #[cfg(debug_assertions)]
    {
        // Debug: allow runtime override for development
        std::env::var("NOSH_CLOUD_URL").unwrap_or_else(|_| {
            compile_time_url
                .unwrap_or(DEFAULT_CLOUD_URL)
                .to_string()
        })
    }
}
