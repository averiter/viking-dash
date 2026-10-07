//! Viking Dash — La Route de Valhalla
//!
//! Un jeu d'adresse : saute de bouclier en bouclier en évitant les créatures
//! qui tournent autour. Inspiré du "Cochon d'Halloween", version viking.

mod audio;
#[cfg(not(target_arch = "wasm32"))]
mod autotest;
mod fx;
mod player;
mod ui;
mod world;

use bevy::prelude::*;
use bevy::text::FontSource;
use bevy::window::WindowResolution;

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

/// Police du jeu : Luminari (système) sur macOS, Fira Sans embarquée ailleurs et sur le web.
#[derive(Resource)]
pub struct GameFont(pub FontSource);

/// Taille de référence : la vue fait toujours au moins 720 unités de haut.
pub const VIEW_H: f32 = 720.0;
/// Largeur minimale garantie (un bouclier au bord avec son second anneau reste visible).
pub const VIEW_W_MIN: f32 = 780.0;


fn main() {
    // `VIKING_WINDOW=390x844` force une taille de fenêtre (tests du mode portrait).
    let (w, h) = std::env::var("VIKING_WINDOW")
        .ok()
        .and_then(|v| {
            let (a, b) = v.split_once('x')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .unwrap_or((960, 720));
    let mut app = App::new();
    app
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            // Pas de fichiers `.meta` : évite des requêtes 404 sur le web.
            meta_check: bevy::asset::AssetMetaCheck::Never,
            ..default()
        }).set(WindowPlugin {
            primary_window: Some(Window {
                title: "Viking Dash — La Route de Valhalla".into(),
                resolution: WindowResolution::new(w, h),
                // Web : le canvas remplit la page et le navigateur ne défile pas.
                canvas: Some("#viking".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
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
        ));
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(autotest::AutoTestPlugin);
    app.add_systems(PreStartup, load_font)
        .add_systems(Startup, (setup_camera, load_best, enter_menu))
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        MainCamera,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::AutoMin {
                min_width: VIEW_W_MIN,
                min_height: VIEW_H,
            },
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 120.0, 0.0),
    ));
}

fn load_font(mut commands: Commands, asset_server: Res<AssetServer>) {
    let source = if cfg!(target_os = "macos") {
        FontSource::from("Luminari")
    } else {
        FontSource::Handle(asset_server.load("fonts/FiraSans-Bold.ttf"))
    };
    commands.insert_resource(GameFont(source));
}

fn enter_menu(mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::Menu);
}

#[cfg(not(target_arch = "wasm32"))]
fn best_path() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".viking_dash_best"))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_best() -> Option<String> {
    std::fs::read_to_string(best_path()?).ok()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_best(best: u32) {
    if let Some(p) = best_path() {
        let _ = std::fs::write(p, best.to_string());
    }
}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
fn read_best() -> Option<String> {
    local_storage()?.get_item("viking_dash_best").ok()?
}

#[cfg(target_arch = "wasm32")]
pub fn save_best(best: u32) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item("viking_dash_best", &best.to_string());
    }
}

fn load_best(mut score: ResMut<Score>) {
    if let Some(s) = read_best() {
        score.best = s.trim().parse().unwrap_or(0);
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
