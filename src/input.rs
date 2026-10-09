//! Lecture des entrées (clavier, souris, tactile, manettes) et attribution aux joueurs.
//!
//! À deux : le joueur 1 utilise Espace, la première manette ou la moitié gauche
//! (ou haute, en portrait) de l'écran ; le joueur 2 utilise Entrée, la seconde
//! manette ou l'autre moitié.

use crate::{split_horizontal, GameMode, MAX_PLAYERS};
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Entrées de la frame, déjà attribuées aux joueurs.
#[derive(Resource, Default)]
pub struct Inputs {
    /// Saut demandé par chaque joueur.
    pub jump: [bool; MAX_PLAYERS],
    /// N'importe quelle action (écrans de menu).
    pub any: bool,
    /// Clics et contacts tactiles de la frame, en pixels logiques de la fenêtre.
    pub taps: Vec<Vec2>,
    /// Choix du mode dans le menu.
    pub choose_solo: bool,
    pub choose_duo: bool,
    pub escape: bool,
}

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inputs>()
            .add_systems(PreUpdate, read_inputs.after(InputSystems));
    }
}

fn read_inputs(
    mut inputs: ResMut<Inputs>,
    mode: Res<GameMode>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    gamepads: Query<(Entity, &Gamepad)>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    *inputs = Inputs::default();
    let size = window.size();
    let horizontal = split_horizontal(size);
    let side = |p: Vec2| -> usize {
        if horizontal {
            (p.x >= size.x / 2.0) as usize
        } else {
            (p.y >= size.y / 2.0) as usize
        }
    };

    // Clavier.
    let p1_key = keys.any_just_pressed([KeyCode::Space, KeyCode::ArrowUp, KeyCode::KeyW, KeyCode::KeyZ]);
    let p2_key = keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::ShiftRight]);

    // Souris et tactile : position du clic ou du doigt.
    let mut taps: Vec<Vec2> = Vec::new();
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(p) = window.cursor_position() {
            taps.push(p);
        }
    }
    for touch in touches.iter_just_pressed() {
        taps.push(touch.position());
    }

    // Manettes, dans l'ordre de connexion : la première au joueur 1, la seconde au joueur 2.
    let mut pads: Vec<(Entity, &Gamepad)> = gamepads.iter().collect();
    pads.sort_by_key(|(e, _)| *e);
    let pad_pressed: Vec<bool> = pads
        .iter()
        .map(|(_, g)| g.get_just_pressed().next().is_some())
        .collect();

    inputs.any = p1_key || p2_key || !taps.is_empty() || pad_pressed.iter().any(|b| *b);

    if mode.players == 2 {
        inputs.jump[0] = p1_key
            || taps.iter().any(|p| side(*p) == 0)
            || pad_pressed.first().copied().unwrap_or(false);
        inputs.jump[1] = p2_key
            || taps.iter().any(|p| side(*p) == 1)
            || pad_pressed.get(1).copied().unwrap_or(false);
    } else {
        inputs.jump[0] = inputs.any;
    }

    inputs.taps = taps;

    inputs.choose_solo = keys.any_just_pressed([
        KeyCode::Space,
        KeyCode::Enter,
        KeyCode::NumpadEnter,
        KeyCode::Digit1,
        KeyCode::Numpad1,
    ]) || pads.iter().any(|(_, g)| g.just_pressed(GamepadButton::South));
    inputs.choose_duo = keys.any_just_pressed([KeyCode::Digit2, KeyCode::Numpad2])
        || pads
            .iter()
            .any(|(_, g)| g.just_pressed(GamepadButton::North) || g.just_pressed(GamepadButton::Start));
    inputs.escape = keys.just_pressed(KeyCode::Escape);
}
