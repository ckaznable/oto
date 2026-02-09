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
    match event {
        KeyEvent {
            code: KeyCode::Char('q'),
            ..
        } => {
            tx.send(AppCommand::End).ok();
        }
        KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::SelectItem).ok();
        }
        KeyEvent {
            code: KeyCode::Char(' '),
            ..
        } => {
            player_tx.send(PlayerCommand::PauseCycle).ok();
        }
        KeyEvent {
            code: KeyCode::Char('J'),
            ..
        } => {
            player_tx.send(PlayerCommand::SetRelatedVolume(-5)).ok();
        }
        KeyEvent {
            code: KeyCode::Char('K'),
            ..
        } => {
            player_tx.send(PlayerCommand::SetRelatedVolume(5)).ok();
        }
        KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::MoveCollapseCursor(1)).ok();
        }
        KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::MoveCollapseCursor(-1)).ok();
        }
        KeyEvent {
            code: KeyCode::Char('j'),
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(1)))
                .ok();
        }
        KeyEvent {
            code: KeyCode::Char('k'),
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(-1)))
                .ok();
        }
        KeyEvent {
            code: KeyCode::Char('f'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(10)))
                .ok();
        }
        KeyEvent {
            code: KeyCode::Char('b'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(-10)))
                .ok();
        }
        KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(5)))
                .ok();
        }
        KeyEvent {
            code: KeyCode::Char('u'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(-5)))
                .ok();
        }
        KeyEvent {
            code: KeyCode::Char('g'),
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Start)).ok();
        }
        KeyEvent {
            code: KeyCode::Char('G'),
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::End)).ok();
        }
        KeyEvent {
            code: KeyCode::Char('l'),
            ..
        } => {
            tx.send(AppCommand::MoveSubCollapseCursor(1)).ok();
        }
        KeyEvent {
            code: KeyCode::Char('h'),
            ..
        } => {
            tx.send(AppCommand::MoveSubCollapseCursor(-1)).ok();
        }
        KeyEvent {
            code: KeyCode::Char('L'),
            ..
        } => {
            player_tx.send(PlayerCommand::NextSong).ok();
        }
        KeyEvent {
            code: KeyCode::Char('H'),
            ..
        } => {
            player_tx.send(PlayerCommand::PrevSong).ok();
        }
        KeyEvent {
            code: KeyCode::Enter,
            ..
        } => {
            tx.send(AppCommand::SubmitItem).ok();
        }
        KeyEvent {
            code: KeyCode::Tab, ..
        } => {
            tx.send(AppCommand::SelectItem).ok();
        }
        KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::AppendItem).ok();
        }
        KeyEvent {
            code: KeyCode::Char('i'),
            ..
        } => {
            tx.send(AppCommand::InsertItem).ok();
        }
        KeyEvent {
            code: KeyCode::Char('a'),
            ..
        } => {
            tx.send(AppCommand::TogglePickerMode).ok();
        }
        KeyEvent {
            code: KeyCode::Char('/'),
            ..
        } => {
            mode.store(KeyHandleMode::Edit as u8, atomic::Ordering::Relaxed);
            tx.send(AppCommand::Rerender(true)).ok();
        }
        KeyEvent {
            code: KeyCode::Char('r'),
            ..
        } => {
            tx.send(AppCommand::RandomPlaylist).ok();
        }
        _ => {}
    }
}

fn handle_edit_keypress(tx: Sender<AppCommand>, event: Event, mode: Arc<AtomicU8>) {
    match event {
        Event::Key(event) => match event {
            KeyEvent {
                code: KeyCode::Esc | KeyCode::Enter,
                ..
            } => {
                mode.store(KeyHandleMode::App as u8, atomic::Ordering::Relaxed);
                tx.send(AppCommand::Rerender(true)).ok();
            }
            KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                tx.send(AppCommand::MoveListCursor(CursorMove::Steps(1)))
                    .ok();
            }
            KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                tx.send(AppCommand::MoveListCursor(CursorMove::Steps(-1)))
                    .ok();
            }
            event => {
                tx.send(AppCommand::EditEvent(Event::Key(event))).ok();
            }
        },
        event => {
            tx.send(AppCommand::EditEvent(event)).ok();
        }
    }
}

pub fn handle_keypress(tx: Sender<AppCommand>, player_tx: Sender<PlayerCommand>, mode: Arc<AtomicU8>) {
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
