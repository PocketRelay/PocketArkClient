fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();

        if cfg!(debug_assertions) {
            res.set_manifest(include_str!("./Debug.Manifest.xml"));
        } else {
            res.set_manifest(include_str!("./Manifest.xml"));
        }
        res.set_icon("src/resources/icon.ico");
        res.compile().unwrap();
    }

    #[cfg(all(not(target_os = "windows"), feature = "native"))]
    {
        compile_error!("Cannot compile the windows native variant for non windows targets");
    }
}
