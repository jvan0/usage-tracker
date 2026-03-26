// tray.rs — System tray para Windows
//
// En Windows: crea un icono en la barra de tareas (iconos ocultos).
// Click en el menú → mostrar/ocultar el widget
// En Linux/Mac: no implementado aún.

pub fn run_tray() {
    #[cfg(target_os = "windows")]
    {
        run_tray_windows();
    }

    #[cfg(not(target_os = "windows"))]
    {
        println!("System tray solo soportado en Windows por ahora.");
        println!("Usá 'usage-tracker widget' directamente.");
    }
}

#[cfg(target_os = "windows")]
fn run_tray_windows() {
    use std::sync::{Arc, Mutex};
    use tao::event::Event;
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tray_icon::{
        menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
        TrayIconBuilder,
    };

    let exe = std::env::current_exe().expect("No se pudo obtener la ruta del ejecutable");

    let widget_process: Arc<Mutex<Option<std::process::Child>>> = Arc::new(Mutex::new(None));
    let widget_proc = widget_process.clone();

    // Crear menú
    let tray_menu = Menu::new();
    let open_item = MenuItem::new("Open Widget", true, None);
    let quit_item = MenuItem::new("Quit", true, None);
    tray_menu.append(&open_item).unwrap();
    tray_menu.append(&PredefinedMenuItem::separator()).unwrap();
    tray_menu.append(&quit_item).unwrap();

    // Crear icono
    let icon = create_tray_icon();

    // Crear tray
    let _tray = TrayIconBuilder::new()
        .with_tooltip("Usage Tracker")
        .with_icon(icon)
        .with_menu(Box::new(tray_menu))
        .build()
        .expect("Error creando tray icon");

    // Auto-abrir widget
    launch_widget(&exe, &widget_process);

    let event_loop = EventLoopBuilder::new().build();
    let menu_channel = MenuEvent::receiver();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::MainEventsCleared = event {
            if let Ok(event) = menu_channel.try_recv() {
                if event.id == open_item.id() {
                    // Abrir o toggle widget
                    let mut proc = widget_proc.lock().unwrap();
                    let needs_launch = match *proc {
                        Some(ref mut child) => {
                            matches!(child.try_wait(), Ok(Some(_)) | Err(_))
                        }
                        None => true,
                    };
                    if needs_launch {
                        *proc = None;
                        drop(proc);
                        launch_widget(&exe, &widget_proc);
                    }
                } else if event.id == quit_item.id() {
                    let mut proc = widget_proc.lock().unwrap();
                    if let Some(ref mut child) = *proc {
                        let _ = child.kill();
                    }
                    *control_flow = ControlFlow::Exit;
                }
            }
        }
    });
}

#[cfg(target_os = "windows")]
fn launch_widget(
    exe: &std::path::Path,
    process: &std::sync::Arc<std::sync::Mutex<Option<std::process::Child>>>,
) {
    let mut proc = process.lock().unwrap();
    if proc.is_none() {
        if let Ok(child) = std::process::Command::new(exe).arg("widget").spawn() {
            *proc = Some(child);
        }
    }
}

#[cfg(target_os = "windows")]
fn create_tray_icon() -> tray_icon::Icon {
    // Generar icono 16x16 en runtime
    let mut rgba = Vec::with_capacity(16 * 16 * 4);
    for y in 0..16u32 {
        for x in 0..16u32 {
            let dist = ((x as f32 - 8.0).powi(2) + (y as f32 - 8.0).powi(2)).sqrt();
            if dist < 4.0 {
                let i = (1.0 - dist / 4.0).max(0.0);
                rgba.extend_from_slice(&[
                    (50.0 * i) as u8,
                    (200.0 * i) as u8,
                    (100.0 * i) as u8,
                    (255.0 * i) as u8,
                ]);
            } else if dist < 7.0 {
                rgba.extend_from_slice(&[60, 60, 60, 200]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    tray_icon::Icon::from_rgba(rgba, 16, 16).expect("Error creando icono")
}
