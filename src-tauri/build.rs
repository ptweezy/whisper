fn main() {
    // Register the app's own commands with Tauri's permission system so the
    // capability file can allow them explicitly (see capabilities/default.json).
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new()
                .commands(&["send_request", "cancel_request", "copy_text", "save_file"]),
        ),
    )
    .expect("failed to run tauri-build");
}
