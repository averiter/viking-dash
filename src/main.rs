//! Viking Dash — La Route de Valhalla
//!
//! Un jeu d'adresse : saute de bouclier en bouclier en évitant les créatures
//! qui tournent autour. Inspiré du "Cochon d'Halloween", version viking.

mod audio;
mod autotest;
mod fx;
mod player;
mod ui;
mod world;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use std::path::PathBuf;

/// États principaux du jeu.
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    /// Un tour de boucle pour laisser les ressources se créer avant le menu.
    #[default]
    Loading,
    Menu,
    Playing,
    Dying,
    GameOver,
}

/// Score courant et record.
#[derive(Resource, Default)]
pub struct Score {
    pub level: u32,
    pub gold: u32,
    pub best: u32,
    pub combo: u32,
    pub new_record: bool,
}

/// Pouvoirs actifs du viking.
#[derive(Resource, Default)]
pub struct Powers {
    /// Bouclier runique : encaisse un coup.
    pub shield: bool,
    /// Mjölnir : nombre de sauts rapides restants.
    pub hammer_jumps: u32,
    /// Hydromel : secondes de ralentissement des monstres restantes.
    pub mead: f32,
}

/// Demande au menu de lancer la partie immédiatement (après un game over).
#[derive(Resource, Default)]
pub struct AutoStart(pub bool);

/// Saut demandé par le mode de test automatique.
#[derive(Resource, Default)]
pub struct AutoJump(pub bool);

/// Marque tout ce qui doit disparaître quand une partie se réinitialise.
#[derive(Component)]
pub struct GameEntity;

#[derive(Component)]
pub struct MainCamera;

/// Hauteur logique de la vue (la largeur s'adapte à la fenêtre).
pub const VIEW_H: f32 = 720.0;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Viking Dash — La Route de Valhalla".into(),
                resolution: WindowResolution::new(960, 720),
                ..default()
            }),
            ..default()
        }))
        .init_state::<GameState>()
        .insert_resource(ClearColor(Color::srgb(0.03, 0.06, 0.14)))
        .init_resource::<Score>()
        .init_resource::<Powers>()
        .init_resource::<AutoStart>()
        .init_resource::<AutoJump>()
        .add_plugins((
            audio::SfxPlugin,
            world::WorldPlugin,
            player::PlayerPlugin,
            fx::FxPlugin,
            ui::UiPlugin,
            autotest::AutoTestPlugin,
        ))
        .add_systems(Startup, (setup_camera, load_best, enter_menu))
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        MainCamera,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: VIEW_H,
            },
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 120.0, 0.0),
    ));
}

fn enter_menu(mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::Menu);
}

fn best_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".viking_dash_best"))
}

fn load_best(mut score: ResMut<Score>) {
    if let Some(p) = best_path() {
        if let Ok(s) = std::fs::read_to_string(p) {
            score.best = s.trim().parse().unwrap_or(0);
        }
    }
}

pub fn save_best(best: u32) {
    if let Some(p) = best_path() {
        let _ = std::fs::write(p, best.to_string());
    }
}

/// Entrée "sauter / valider" : clic gauche, espace, entrée ou toucher.
pub fn action_pressed(
    mouse: &ButtonInput<MouseButton>,
    keys: &ButtonInput<KeyCode>,
    touches: &Touches,
) -> bool {
    mouse.just_pressed(MouseButton::Left)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::ArrowUp)
        || touches.any_just_pressed()
}
