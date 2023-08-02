/// Constant storing the application version
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The host address to redirect in the hosts file
pub const HOST_KEY: &str = "winter15.gosredirector.ea.com";
/// Host address target (Localhost)
pub const HOST_VALUE: &str = "127.0.0.1";
/// The path to the system hosts file
pub const HOSTS_PATH: &str = "C:/Windows/System32/drivers/etc/hosts";

/// Window icon bytes
pub const ICON_BYTES: &[u8] = include_bytes!("resources/assets/icon.ico");

// AnselSDK patch
pub const ANSEL_SDK64_BAK: &[u8] = include_bytes!("resources/embed/AnselSDK64.bak");
pub const ANSEL_SDK64_DLL: &[u8] = include_bytes!("resources/embed/AnselSDK64.dll");
// VerifyCertificate hook
pub const HOOK_ASI: &[u8] = include_bytes!("resources/embed/pocket_ark_hooks.asi");

/// The local redirector server port
pub const REDIRECTOR_PORT: u16 = 42230;
/// The local proxy main server port
pub const MAIN_PORT: u16 = 42231;
/// The local qos server port
pub const QOS_PORT: u16 = 42232;
