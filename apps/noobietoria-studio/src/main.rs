/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Noobietoria Studio: Roblox-style 3D world builder, inspector and playtester.

mod app;
mod state;
mod ui;

use state::StudioApp;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Noobietoria Studio - Roblox-like IDE and World Editor");
        println!("Usage: noobietoria-studio [OPTIONS]");
        println!();
        println!("Options:");
        println!("  -p, --place <PATH>   Open a .nbw world file directly");
        println!("  --new                Start with a blank Baseplate canvas");
        println!("  --selftest           Run automated engine self-test and exit");
        println!("  -h, --help           Print help information");
        return Ok(());
    }

    if args.iter().any(|a| a == "--selftest") {
        println!("[Noobietoria Studio] Running automated self-test...");
        let mut app = StudioApp::new_headless();
        assert!(
            app.world.len() >= 3,
            "Starter place must have at least 3 instances"
        );
        println!("  [✓] Starter place initialized (Baseplate, SpawnLocation, Audio)");

        // Test Toolbox insertion
        let ids = nb_world::ToolboxCatalog::insert_into_world(
            &mut app.world,
            "lava_brick",
            nb_types::Vector3::new(10.0, 1.0, 10.0),
        )
        .expect("Toolbox insertion must succeed");
        assert_eq!(ids.len(), 2, "Lava brick must insert Part + KillScript");
        println!("  [✓] Toolbox asset insertion verified");

        // Test saving .nbw
        let temp_path = std::env::temp_dir().join("studio_selftest.nbw");
        let file = nb_world::WorldFile::from_world(&app.world, app.metadata, app.settings);
        file.save_to_file(&temp_path)
            .expect("Save .nbw must succeed");
        println!("  [✓] .nbw file serialization verified");
        let _ = std::fs::remove_file(temp_path);

        println!("[Noobietoria Studio] All self-tests passed successfully!");
        return Ok(());
    }

    // Launch GUI
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = egui::ViewportBuilder::default()
        .with_title("Noobietoria Studio - [Baseplate Place]")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([800.0, 500.0]);

    let mut initial_place_path: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        if (args[i] == "--place" || args[i] == "-p") && i + 1 < args.len() {
            initial_place_path = Some(PathBuf::from(&args[i + 1]));
            i += 1;
        }
        i += 1;
    }

    eframe::run_native(
        "Noobietoria Studio",
        native_options,
        Box::new(move |cc| {
            let mut app = StudioApp::new(cc);
            if let Some(path) = initial_place_path {
                if let Ok(file) = nb_world::WorldFile::load_from_file(&path) {
                    app.world = file.to_world();
                    app.metadata = file.metadata;
                    app.settings = file.settings;
                    app.current_path = Some(path.clone());
                    app.log(
                        state::LogLevel::Info,
                        format!("Loaded place: {}", path.display()),
                    );
                }
            }
            Ok(Box::new(app))
        }),
    )
}
