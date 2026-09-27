//! System tray icon, context menu, and the event loop that drives them.
//!
//! Menu layout:
//!   [x] Enabled
//!   Volume >
//!       ( ) 25%
//!       ( ) 50%
//!       (o) 75%
//!       ( ) 100%
//!   [ ] Start with Windows
//!   ---
//!   Quit

use crate::autostart::{self, RunKeyStore};
use crate::config::Config;
use crate::state::AppState;
use anyhow::Result;
use std::sync::Arc;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, Submenu};
use tray_icon::{Icon, TrayIconBuilder};
use winit::event_loop::{ControlFlow, EventLoopBuilder};

/// The four supported volume steps.
pub const VOLUME_STEPS: [u32; 4] = [25, 50, 75, 100];

/// Outcome of handling a menu click, used to drive state updates and to make
/// the mapping unit-testable without a real GUI.
#[derive(Debug, PartialEq, Eq)]
pub enum MenuAction {
    ToggleEnabled(bool),
    SetVolume(u32),
    SetAutostart(bool),
    Quit,
    None,
}

/// Identifiers assigned to menu items, kept so we can map click events back to
/// an action. Cloned into the event loop.
struct MenuIds {
    enabled: String,
    volumes: Vec<(u32, String)>,
    autostart: String,
    quit: String,
}

impl MenuIds {
    /// Pure mapping from a clicked menu id to an action, given the current
    /// state. Extracted so it can be tested directly.
    fn action_for(
        &self,
        clicked: &str,
        current_enabled: bool,
        current_autostart: bool,
    ) -> MenuAction {
        if clicked == self.enabled {
            return MenuAction::ToggleEnabled(!current_enabled);
        }
        if clicked == self.autostart {
            return MenuAction::SetAutostart(!current_autostart);
        }
        if clicked == self.quit {
            return MenuAction::Quit;
        }
        for (vol, id) in &self.volumes {
            if clicked == id {
                return MenuAction::SetVolume(*vol);
            }
        }
        MenuAction::None
    }
}

/// Build a simple monochrome tray icon at runtime (a small keyboard-key glyph),
/// so we do not depend on shipping an .ico separately.
fn build_icon() -> Result<Icon> {
    // 32x32 RGBA. Draw a rounded filled square with a lighter inner face to
    // suggest a keycap.
    const S: u32 = 32;
    let mut rgba = vec![0u8; (S * S * 4) as usize];
    for y in 0..S {
        for x in 0..S {
            let idx = ((y * S + x) * 4) as usize;
            let border = 4;
            let inside = x >= border && x < S - border && y >= border && y < S - border;
            let inner_face = x >= border + 3
                && x < S - border - 3
                && y >= border + 3
                && y < S - border - 5;
            if inner_face {
                rgba[idx] = 235;
                rgba[idx + 1] = 235;
                rgba[idx + 2] = 240;
                rgba[idx + 3] = 255;
            } else if inside {
                rgba[idx] = 120;
                rgba[idx + 1] = 120;
                rgba[idx + 2] = 130;
                rgba[idx + 3] = 255;
            } else {
                // transparent background
                rgba[idx + 3] = 0;
            }
        }
    }
    Icon::from_rgba(rgba, S, S).map_err(|e| anyhow::anyhow!("failed to build tray icon: {e}"))
}

/// Run the tray + event loop. Blocks until the user chooses Quit.
///
/// `state` is shared with the hook thread; `store` provides registry access
/// for the autostart toggle. `config` is the loaded config we keep in sync and
/// persist on every change.
pub fn run<S: RunKeyStore + 'static>(
    state: Arc<AppState>,
    store: S,
    mut config: Config,
) -> Result<()> {
    let event_loop = EventLoopBuilder::new().build()?;

    // --- Build the menu ---
    let tray_menu = Menu::new();

    let enabled_item = CheckMenuItem::new("Enabled", true, config.enabled, None);

    let volume_submenu = Submenu::new("Volume", true);
    let mut volume_items = Vec::new();
    for step in VOLUME_STEPS {
        let checked = config.volume_pct == step;
        let item = CheckMenuItem::new(format!("{step}%"), true, checked, None);
        volume_submenu.append(&item)?;
        volume_items.push((step, item));
    }

    let autostart_item =
        CheckMenuItem::new("Start with Windows", true, config.autostart, None);
    let quit_item = MenuItem::new("Quit", true, None);

    tray_menu.append(&enabled_item)?;
    tray_menu.append(&volume_submenu)?;
    tray_menu.append(&autostart_item)?;
    tray_menu.append(&tray_icon::menu::PredefinedMenuItem::separator())?;
    tray_menu.append(&quit_item)?;

    let ids = MenuIds {
        enabled: enabled_item.id().0.clone(),
        volumes: volume_items
            .iter()
            .map(|(v, item)| (*v, item.id().0.clone()))
            .collect(),
        autostart: autostart_item.id().0.clone(),
        quit: quit_item.id().0.clone(),
    };

    // --- Build the tray icon ---
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Mechanical Keyboard Sound")
        .with_icon(build_icon()?)
        .build()?;

    let menu_channel = MenuEvent::receiver();

    event_loop.run(move |_event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

        // Drain any pending menu events.
        while let Ok(ev) = menu_channel.try_recv() {
            let action = ids.action_for(&ev.id.0, config.enabled, config.autostart);
            match action {
                MenuAction::ToggleEnabled(v) => {
                    config.enabled = v;
                    state.set_enabled(v);
                    enabled_item.set_checked(v);
                    let _ = config.save();
                }
                MenuAction::SetVolume(vol) => {
                    config.volume_pct = vol;
                    state.set_volume_pct(vol);
                    // Update the radio-style checkmarks.
                    for (step, item) in &volume_items {
                        item.set_checked(*step == vol);
                    }
                    let _ = config.save();
                }
                MenuAction::SetAutostart(v) => {
                    match autostart::set_autostart(&store, v) {
                        Ok(()) => {
                            config.autostart = v;
                            autostart_item.set_checked(v);
                            let _ = config.save();
                        }
                        Err(_) => {
                            // Revert the checkbox if the registry write failed.
                            autostart_item.set_checked(config.autostart);
                        }
                    }
                }
                MenuAction::Quit => {
                    elwt.exit();
                }
                MenuAction::None => {}
            }
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ids() -> MenuIds {
        MenuIds {
            enabled: "enabled".into(),
            volumes: vec![
                (25, "v25".into()),
                (50, "v50".into()),
                (75, "v75".into()),
                (100, "v100".into()),
            ],
            autostart: "auto".into(),
            quit: "quit".into(),
        }
    }

    #[test]
    fn toggle_enabled_inverts_current() {
        let ids = sample_ids();
        assert_eq!(
            ids.action_for("enabled", true, false),
            MenuAction::ToggleEnabled(false)
        );
        assert_eq!(
            ids.action_for("enabled", false, false),
            MenuAction::ToggleEnabled(true)
        );
    }

    #[test]
    fn volume_ids_map_to_values() {
        let ids = sample_ids();
        assert_eq!(ids.action_for("v25", true, false), MenuAction::SetVolume(25));
        assert_eq!(
            ids.action_for("v100", true, false),
            MenuAction::SetVolume(100)
        );
    }

    #[test]
    fn autostart_and_quit_map() {
        let ids = sample_ids();
        assert_eq!(
            ids.action_for("auto", true, false),
            MenuAction::SetAutostart(true)
        );
        assert_eq!(
            ids.action_for("auto", true, true),
            MenuAction::SetAutostart(false)
        );
        assert_eq!(ids.action_for("quit", true, false), MenuAction::Quit);
    }

    #[test]
    fn unknown_id_is_none() {
        let ids = sample_ids();
        assert_eq!(ids.action_for("bogus", true, false), MenuAction::None);
    }
}
