//! Viking Dash — La Route de Valhalla
//!
//! Un jeu d'adresse : saute de bouclier en bouclier en évitant les créatures
//! qui tournent autour. Inspiré du "Cochon d'Halloween", version viking.
//! Seul ou à deux en écran partagé (clavier, souris, tactile ou manettes).

mod audio;
#[cfg(not(target_arch = "wasm32"))]
mod autotest;
mod fx;
mod input;
mod player;
mod ui;
mod world;

use bevy::camera::visibility::RenderLayers;
use bevy::camera::Viewport;
use bevy::prelude::*;
use bevy::text::FontSource;
use bevy::ui::IsDefaultUiCamera;
use bevy::window::{PrimaryWindow, WindowResolution};

/// États principaux du jeu.
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    /// Un tour de boucle pour laisser les ressources se créer avant le menu.
    #[default]
    Loading,
    Menu,
    Playing,
    GameOver,
}

/// Record (partagé entre les joueurs).
#[derive(Resource, Default)]
pub struct Score {
    pub best: u32,
    pub new_record: bool,
}

/// Pouvoirs actifs d'un viking.
#[derive(Default, Clone)]
pub struct Powers {
    /// Bouclier runique : encaisse un coup.
    pub shield: bool,
    /// Mjölnir : nombre de sauts rapides restants.
    pub hammer_jumps: u32,
    /// Hydromel : secondes de ralentissement des monstres restantes.
    pub mead: f32,
}

pub const MAX_PLAYERS: usize = 2;

/// Nombre de joueurs de la partie (1 ou 2).
#[derive(Resource)]
pub struct GameMode {
    pub players: u8,
}

impl Default for GameMode {
    fn default() -> Self {
        Self { players: 1 }
    }
}

/// Demande au menu de relancer la partie immédiatement (après un game over).
#[derive(Resource, Default)]
pub struct AutoStart(pub bool);

/// Sauts demandés par le mode de test automatique.
#[derive(Resource, Default)]
pub struct AutoJump(pub [bool; MAX_PLAYERS]);

/// Marque tout ce qui doit disparaître quand une partie se réinitialise.
#[derive(Component)]
pub struct GameEntity;

/// Caméra d'un joueur (0 ou 1). En solo, seule la caméra 0 est active.
#[derive(Component)]
pub struct PlayerCam(pub u8);

/// Intensité de secousse d'une caméra (décroît toute seule).
#[derive(Component, Default)]
pub struct CamShake(pub f32);

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
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                // Pas de fichiers `.meta` : évite des requêtes 404 sur le web.
                meta_check: bevy::asset::AssetMetaCheck::Never,
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Viking Dash — La Route de Valhalla".into(),
                    resolution: WindowResolution::new(w, h),
                    // Web : le canvas remplit la page et le navigateur ne défile pas.
                    canvas: Some("#viking".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: true,
                    // En mode test, la fenêtre reste au premier plan : macOS cesse de
                    // rendre une fenêtre entièrement recouverte, ce qui noircit les captures.
                    window_level: if std::env::var_os("VIKING_AUTOTEST").is_some() {
                        bevy::window::WindowLevel::AlwaysOnTop
                    } else {
                        bevy::window::WindowLevel::Normal
                    },
                    ..default()
                }),
                ..default()
            }),
    )
    .init_state::<GameState>()
    .insert_resource(ClearColor(Color::srgb(0.03, 0.06, 0.14)))
    .init_resource::<Score>()
    .init_resource::<GameMode>()
    .init_resource::<AutoStart>()
    .init_resource::<AutoJump>()
    .add_plugins((
        audio::SfxPlugin,
        input::InputPlugin,
        world::WorldPlugin,
        player::PlayerPlugin,
        fx::FxPlugin,
        ui::UiPlugin,
    ));
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(autotest::AutoTestPlugin);
    app.add_systems(PreStartup, load_font)
        .add_systems(Startup, (setup_cameras, load_best, enter_menu))
        .add_systems(Update, layout_cameras)
        .run();
}

fn setup_cameras(mut commands: Commands) {
    for id in 0..MAX_PLAYERS as u8 {
        let mut cam = commands.spawn((
            Camera2d,
            PlayerCam(id),
            CamShake::default(),
            Camera {
                order: id as isize,
                is_active: id == 0,
                ..default()
            },
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: bevy::camera::ScalingMode::AutoMin {
                    min_width: VIEW_W_MIN,
                    min_height: VIEW_H,
                },
                ..OrthographicProjection::default_2d()
            }),
            // Couche 0 : le monde partagé ; couche id+1 : le ciel propre à cette caméra.
            RenderLayers::from_layers(&[0, id as usize + 1]),
            Transform::from_xyz(0.0, 120.0, 0.0),
        ));
        if id == 0 {
            cam.insert(IsDefaultUiCamera);
        }
    }
}

/// Écran partagé : côte à côte en paysage, l'un au-dessus de l'autre en portrait.
pub fn split_horizontal(size: Vec2) -> bool {
    size.x >= size.y
}

/// Attribue à chaque caméra sa moitié d'écran (ou tout l'écran en solo et hors partie).
fn layout_cameras(
    window: Single<&Window, With<PrimaryWindow>>,
    mode: Res<GameMode>,
    state: Res<State<GameState>>,
    mut cams: Query<(&PlayerCam, &mut Camera)>,
) {
    let size = window.physical_size();
    let split = mode.players == 2 && *state.get() == GameState::Playing;
    let horizontal = split_horizontal(size.as_vec2());
    for (cam, mut camera) in &mut cams {
        let id = cam.0 as u32;
        if !split {
            if camera.is_active != (id == 0) {
                camera.is_active = id == 0;
            }
            if camera.viewport.is_some() {
                camera.viewport = None;
            }
            continue;
        }
        let (pos, vsize) = if horizontal {
            let w = size.x / 2;
            (UVec2::new(id * w, 0), UVec2::new(if id == 0 { w } else { size.x - w }, size.y))
        } else {
            let h = size.y / 2;
            (UVec2::new(0, id * h), UVec2::new(size.x, if id == 0 { h } else { size.y - h }))
        };
        if vsize.x == 0 || vsize.y == 0 {
            continue;
        }
        if !camera.is_active {
            camera.is_active = true;
        }
        let same = camera
            .viewport
            .as_ref()
            .is_some_and(|v| v.physical_position == pos && v.physical_size == vsize);
        if !same {
            camera.viewport = Some(Viewport {
                physical_position: pos,
                physical_size: vsize,
                ..default()
            });
        }
    }
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
