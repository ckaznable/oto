use std::sync::{
    Arc,
    atomic::{self, AtomicU8},
    mpsc::Sender,
};

use ratatui::crossterm::{
    self,
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
};

use crate::{
    event::{AppCommand, CursorMove, PlayerCommand},
    tui::state::KeyHandleMode,
};

fn handle_app_keypress(
    tx: Sender<AppCommand>,
    player_tx: Sender<PlayerCommand>,
    event: KeyEvent,
    mode: Arc<AtomicU8>,
) {
    let KeyEvent {
        code, modifiers, ..
    } = event;

    match code {
        KeyCode::Char('q') => {
            tx.send(AppCommand::End).ok();
        }
        KeyCode::Char(' ') if modifiers.contains(KeyModifiers::CONTROL) => {
            tx.send(AppCommand::SelectItem).ok();
        }
        KeyCode::Char(' ') => {
            player_tx.send(PlayerCommand::PauseCycle).ok();
        }
        KeyCode::Char('J') => {
            player_tx.send(PlayerCommand::SetRelatedVolume(-3)).ok();
        }
        KeyCode::Char('K') => {
            player_tx.send(PlayerCommand::SetRelatedVolume(3)).ok();
        }
        KeyCode::Char('j') if modifiers.contains(KeyModifiers::CONTROL) => {
            tx.send(AppCommand::MoveCollapseCursor(1)).ok();
        }
        KeyCode::Char('k') if modifiers.contains(KeyModifiers::CONTROL) => {
            tx.send(AppCommand::MoveCollapseCursor(-1)).ok();
        }
        KeyCode::Char('j') => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(1)))
                .ok();
        }
        KeyCode::Char('k') => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(-1)))
                .ok();
        }
        KeyCode::Char('f') if modifiers.contains(KeyModifiers::CONTROL) => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(10)))
                .ok();
        }
        KeyCode::Char('b') if modifiers.contains(KeyModifiers::CONTROL) => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(-10)))
                .ok();
        }
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(5)))
                .ok();
        }
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(-5)))
                .ok();
        }
        KeyCode::Char('g') => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Start)).ok();
        }
        KeyCode::Char('G') => {
            tx.send(AppCommand::MoveListCursor(CursorMove::End)).ok();
        }
        KeyCode::Char('l') => {
            tx.send(AppCommand::MoveSubCollapseCursor(1)).ok();
        }
        KeyCode::Char('h') => {
            tx.send(AppCommand::MoveSubCollapseCursor(-1)).ok();
        }
        KeyCode::Char('L') => {
            player_tx.send(PlayerCommand::NextSong).ok();
        }
        KeyCode::Char('H') => {
            player_tx.send(PlayerCommand::PrevSong).ok();
        }
        KeyCode::Enter => {
            tx.send(AppCommand::SubmitItem).ok();
        }
        KeyCode::Tab => {
            tx.send(AppCommand::SelectItem).ok();
        }
        KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
            tx.send(AppCommand::AppendItem).ok();
        }
        KeyCode::Char('i') => {
            tx.send(AppCommand::InsertItem).ok();
        }
        KeyCode::Char('a') => {
            tx.send(AppCommand::TogglePickerMode).ok();
        }
        KeyCode::Char('/') => {
            mode.store(KeyHandleMode::Edit as u8, atomic::Ordering::Relaxed);
            tx.send(AppCommand::Rerender(true)).ok();
        }
        KeyCode::Char('r') => {
            tx.send(AppCommand::RandomPlaylist).ok();
        }
        KeyCode::Char('x') => {
            tx.send(AppCommand::RemoveFromPicked).ok();
        }
        _ => {}
    }
}

fn handle_edit_keypress(tx: Sender<AppCommand>, event: Event, mode: Arc<AtomicU8>) {
    match event {
        Event::Key(key_event) => {
            let KeyEvent {
                code, modifiers, ..
            } = key_event;

            match code {
                KeyCode::Esc | KeyCode::Enter => {
                    mode.store(KeyHandleMode::App as u8, atomic::Ordering::Relaxed);
                    tx.send(AppCommand::Rerender(true)).ok();
                }
                KeyCode::Char('n') if modifiers.contains(KeyModifiers::CONTROL) => {
                    tx.send(AppCommand::MoveListCursor(CursorMove::Steps(1)))
                        .ok();
                }
                KeyCode::Char('p') if modifiers.contains(KeyModifiers::CONTROL) => {
                    tx.send(AppCommand::MoveListCursor(CursorMove::Steps(-1)))
                        .ok();
                }
                _ => {
                    tx.send(AppCommand::EditEvent(Event::Key(key_event))).ok();
                }
            }
        }
        event => {
            tx.send(AppCommand::EditEvent(event)).ok();
        }
    }
}

pub fn handle_keypress(
    tx: Sender<AppCommand>,
    player_tx: Sender<PlayerCommand>,
    mode: Arc<AtomicU8>,
) {
    loop {
        let handle_mode = KeyHandleMode::from_repr(mode.load(atomic::Ordering::Relaxed));

        match crossterm::event::read() {
            Ok(Event::Resize(_, _)) => {
                tx.send(AppCommand::Resize).ok();
            }
            Ok(Event::Key(event)) if matches!(handle_mode, Some(KeyHandleMode::App)) => {
                handle_app_keypress(tx.clone(), player_tx.clone(), event, mode.clone());
            }
            Ok(event) => {
                handle_edit_keypress(tx.clone(), event, mode.clone());
            }
            Err(_) => {
                tx.send(AppCommand::Err("crossterm event reader error occurred"))
                    .ok();
            }
        }
    }
}
