mod commands;
mod models;
mod platform;
mod services;

#[cfg(target_os = "macos")]
fn setup_macos_menu(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{AboutMetadata, Menu, MenuItemBuilder, SubmenuBuilder};
    use tauri::Emitter;
    #[cfg(debug_assertions)]
    use tauri::Manager;
    use tauri_plugin_opener::OpenerExt;

    let handle = app.handle();

    // 1. Zen Profile Menu
    let app_menu = SubmenuBuilder::new(handle, "Zen Profile")
        .about(Some(AboutMetadata {
            name: Some("Zen Profile".to_string()),
            version: Some(app.package_info().version.to_string()),
            ..Default::default()
        }))
        .item(
            &MenuItemBuilder::with_id("settings", "Settings…")
                .accelerator("CmdOrCtrl+,")
                .build(handle)?,
        )
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    // 2. File Menu
    let file_menu = SubmenuBuilder::new(handle, "File")
        .item(
            &MenuItemBuilder::with_id("new_profile", "New Profile")
                .accelerator("CmdOrCtrl+N")
                .build(handle)?,
        )
        .separator()
        .close_window()
        .build()?;

    // 3. Edit Menu
    let edit_menu = SubmenuBuilder::new(handle, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    // 4. View Menu
    #[allow(unused_mut)]
    let mut view_menu_builder = SubmenuBuilder::new(handle, "View").fullscreen();

    #[cfg(debug_assertions)]
    {
        view_menu_builder = view_menu_builder.separator().item(
            &MenuItemBuilder::with_id("dev_tools", "Developer Tools")
                .accelerator("CmdOrCtrl+Alt+I")
                .build(handle)?,
        );
    }

    let view_menu = view_menu_builder.build()?;

    // 5. Window Menu
    let window_menu = SubmenuBuilder::new(handle, "Window")
        .minimize()
        .maximize()
        .separator()
        .bring_all_to_front()
        .build()?;

    // 6. Help Menu
    let help_menu = SubmenuBuilder::new(handle, "Help")
        .item(&MenuItemBuilder::with_id("github", "GitHub").build(handle)?)
        .item(&MenuItemBuilder::with_id("report_issue", "Report an Issue").build(handle)?)
        .build()?;

    let menu = Menu::new(handle)?;
    menu.append(&app_menu)?;
    menu.append(&file_menu)?;
    menu.append(&edit_menu)?;
    menu.append(&view_menu)?;
    menu.append(&window_menu)?;
    menu.append(&help_menu)?;

    app.set_menu(menu)?;

    // Menu event listener
    app.on_menu_event(move |app_handle, event| match event.id().0.as_str() {
        "settings" => {
            let _ = app_handle.emit("menu-settings", ());
        }
        "new_profile" => {
            let _ = app_handle.emit("menu-new-profile", ());
        }
        "github" => {
            let _ = app_handle.opener().open_url(
                "https://github.com/dhanushonchain/Zen-profile",
                None::<&str>,
            );
        }
        "report_issue" => {
            let _ = app_handle.opener().open_url(
                "https://github.com/dhanushonchain/Zen-profile/issues/new",
                None::<&str>,
            );
        }
        #[cfg(debug_assertions)]
        "dev_tools" => {
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.open_devtools();
            }
        }
        _ => {}
    });

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                setup_macos_menu(app)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::profile_commands::get_profiles,
            commands::profile_commands::launch_profile,
            commands::profile_commands::create_profile,
            commands::avatar_commands::select_avatar,
            commands::avatar_commands::load_avatar,
            commands::avatar_commands::rename_profile,
            commands::avatar_commands::remove_avatar,
            commands::settings_commands::get_app_config,
            commands::settings_commands::set_theme,
            commands::settings_commands::set_launch_at_login,
            commands::settings_commands::get_launch_at_login,
            commands::settings_commands::pick_zen_path,
            commands::settings_commands::clear_zen_path,
            commands::settings_commands::get_version_info,
            commands::settings_commands::open_settings_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
