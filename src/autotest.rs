//! Mode de test automatique (variable d'environnement `VIKING_AUTOTEST=<dossier>`) :
//! joue tout seul et enregistre des captures d'écran, pour vérifier le rendu.

use crate::{AutoJump, GameState};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::PathBuf;

#[derive(Resource)]
struct AutoTest {
    dir: PathBuf,
    shots: u32,
    next_shot: f32,
    next_jump: f32,
    started: bool,
}

pub struct AutoTestPlugin;

impl Plugin for AutoTestPlugin {
    fn build(&self, app: &mut App) {
        if let Some(dir) = std::env::var_os("VIKING_AUTOTEST") {
            app.insert_resource(AutoTest {
                dir: PathBuf::from(dir),
                shots: 0,
                next_shot: 1.0,
                next_jump: 2.0,
                started: false,
            })
            .add_systems(Update, drive);
        }
    }
}

fn drive(
    mut commands: Commands,
    time: Res<Time>,
    mut at: ResMut<AutoTest>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut jump: ResMut<AutoJump>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs();
    if t > 1.5 && !at.started && *state.get() == GameState::Menu && std::env::var_os("VIKING_AUTOTEST_NOPLAY").is_none() {
        at.started = true;
        next.set(GameState::Playing);
    }
    if t > at.next_jump && *state.get() == GameState::Playing {
        jump.0 = true;
        at.next_jump = t + 1.1;
    }
    if t > at.next_shot {
        at.shots += 1;
        at.next_shot = t + 0.4;
        info!("capture {} ({:?})", at.shots, state.get());
        let path = at.dir.join(format!("shot_{:02}.png", at.shots));
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if t > 12.0 {
        exit.write(AppExit::Success);
    }
}
