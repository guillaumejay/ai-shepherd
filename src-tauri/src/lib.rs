pub mod commands;
pub mod platform;
pub mod terminal;
pub mod usage;

use std::{
    process,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WindowEvent,
};

pub fn run() {
    #[cfg(target_os = "windows")]
    std::env::set_var(
        "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
        "--disable-logging --log-level=3",
    );

    let intentional_quit = Arc::new(AtomicBool::new(false));
    let tray_quit = Arc::clone(&intentional_quit);

    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::list_panes,
            commands::focus_pane,
            commands::send_to_pane,
            commands::get_usage_summary,
            commands::get_usage_history,
            commands::get_usage_diagnostics,
            commands::get_pending_prompts,
            commands::update_settings,
        ])
        .setup(move |app| {
            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            TrayIconBuilder::with_id("main-tray")
                .tooltip("AI Shepherd")
                .icon(
                    app.default_window_icon()
                        .cloned()
                        .expect("missing app icon"),
                )
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        tray_quit.store(true, Ordering::SeqCst);
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            let app_handle = app.handle().clone();
            thread::spawn(move || {
                let emit_usage = |handle: &tauri::AppHandle| {
                    if let Ok(summary) = crate::usage::current_usage_summaries() {
                        let _ = handle.emit("usage:updated", summary);
                    }
                };

                thread::sleep(Duration::from_secs(2));
                emit_usage(&app_handle);

                loop {
                    thread::sleep(Duration::from_secs(10));
                    emit_usage(&app_handle);
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    #[allow(deprecated)]
    let exit_code = app.run_return(|_, _| {});
    if intentional_quit.load(Ordering::SeqCst) {
        process::exit(0);
    }
    process::exit(exit_code);
}
