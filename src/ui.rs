//! Menu, affichage en jeu (un par joueur) et écran de fin.

use crate::audio::{Sfx, SfxBank};
use crate::input::Inputs;
use crate::player::Player;
use crate::{split_horizontal, AutoStart, GameFont, GameMode, GameState, PlayerCam, Score, VIEW_H};
use bevy::color::Alpha;
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::UiTargetCamera;
use bevy::window::PrimaryWindow;

/// `size` est exprimée pour une vue de 720 de haut ; elle suit la plus petite
/// dimension de l'écran (portrait sur téléphone, paysage sur PC).
pub fn font(game_font: &GameFont, size: f32) -> TextFont {
    TextFont {
        font: game_font.0.clone(),
        font_size: FontSize::VMin(size * 100.0 / VIEW_H),
        ..default()
    }
}

/// Police pour les textes du monde (`Text2d`), en unités monde.
pub fn world_font(game_font: &GameFont, size: f32) -> TextFont {
    TextFont {
        font: game_font.0.clone(),
        font_size: FontSize::Px(size),
        ..default()
    }
}

const GOLD: Color = Color::srgb(1.0, 0.85, 0.35);
const PALE: Color = Color::srgb(0.9, 0.92, 1.0);
const P1_COLOR: Color = Color::srgb(1.0, 0.55, 0.45);
const P2_COLOR: Color = Color::srgb(0.55, 0.75, 1.0);

#[derive(Component)]
struct Blink;

#[derive(Component)]
struct Hud;

#[derive(Clone, Copy, PartialEq, Eq)]
enum HudKind {
    Level,
    Gold,
    Best,
    Power,
}

#[derive(Component)]
struct HudText {
    player: u8,
    kind: HudKind,
}

/// Bouton du menu : nombre de joueurs.
#[derive(Component)]
struct ModeButton(u8);

#[derive(Resource, Default)]
struct GameOverDelay(f32);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameOverDelay>()
            .add_systems(OnEnter(GameState::Menu), spawn_menu)
            .add_systems(Update, (menu_input, menu_buttons).run_if(in_state(GameState::Menu)))
            .add_systems(OnEnter(GameState::Playing), (spawn_hud, start_horn))
            .add_systems(Update, (update_hud, escape_to_menu).run_if(in_state(GameState::Playing)))
            .add_systems(OnEnter(GameState::GameOver), (despawn_hud, spawn_game_over))
            .add_systems(Update, game_over_input.run_if(in_state(GameState::GameOver)))
            .add_systems(Update, blink);
    }
}

fn shadow() -> TextShadow {
    TextShadow {
        offset: Vec2::new(3.0, 3.0),
        color: Color::srgba(0.0, 0.0, 0.0, 0.7),
    }
}

fn fullscreen_column() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(10.0),
        ..default()
    }
}

fn mode_button(game_font: &GameFont, players: u8, title: &str, hint: &str) -> impl Bundle {
    let title = title.to_string();
    let hint = hint.to_string();
    let title_font = font(game_font, 30.0);
    let hint_font = font(game_font, 16.0);
    (
        Button,
        ModeButton(players),
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::VMin(3.0), Val::VMin(1.5)),
            border: UiRect::all(Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(14.0)),
            row_gap: Val::Px(4.0),
            ..default()
        },
        BorderColor::all(GOLD),
        BackgroundColor(Color::srgba(0.05, 0.08, 0.2, 0.75)),
        Children::spawn((
            Spawn((Text::new(title), title_font, TextColor(GOLD), shadow())),
            Spawn((Text::new(hint), hint_font, TextColor(PALE), shadow())),
        )),
    )
}

fn spawn_menu(mut commands: Commands, score: Res<Score>, auto: Res<AutoStart>, game_font: Res<GameFont>) {
    if auto.0 {
        return;
    }
    commands.spawn((
        fullscreen_column(),
        DespawnOnExit(GameState::Menu),
        children![
            (
                Text::new("VIKING DASH"),
                font(&game_font, 92.0),
                TextColor(GOLD),
                shadow(),
                Node { margin: UiRect::top(Val::Px(-60.0)), ..default() },
            ),
            (
                Text::new("La Route de Valhalla"),
                font(&game_font, 34.0),
                TextColor(PALE),
                shadow(),
                Node { margin: UiRect::bottom(Val::Px(16.0)), ..default() },
            ),
            (
                Text::new("Bondis de bouclier en bouclier en évitant les créatures qui rôdent autour.\nRamasse Mjölnir, le bouclier runique et l'hydromel.\nEnchaîne les sauts vite : la fureur berserk rapporte de l'or."),
                font(&game_font, 19.0),
                TextColor(Color::srgb(0.8, 0.85, 0.95)),
                TextLayout::default().with_justify(Justify::Center),
                shadow(),
            ),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::VMin(4.0),
                    margin: UiRect::vertical(Val::Px(18.0)),
                    ..default()
                },
                children![
                    mode_button(&game_font, 1, "1 JOUEUR", "Espace, clic ou bouton A"),
                    mode_button(&game_font, 2, "2 JOUEURS", "Touche 2 ou bouton Y"),
                ],
            ),
            (
                Text::new("À deux, l'écran se partage : J1 = Espace, manette 1 ou moitié gauche (haute en portrait)\nJ2 = Entrée, manette 2 ou moitié droite (basse en portrait)"),
                font(&game_font, 16.0),
                TextColor(Color::srgb(0.75, 0.8, 0.9)),
                TextLayout::default().with_justify(Justify::Center),
                shadow(),
            ),
            (
                Text::new(format!("Record : {}", score.best)),
                font(&game_font, 24.0),
                TextColor(GOLD),
                shadow(),
                Node { margin: UiRect::top(Val::Px(14.0)), ..default() },
                Blink,
            ),
        ],
    ));
}

fn plural(n: u32, word: &str) -> String {
    if n >= 2 { format!("{word}s") } else { word.to_string() }
}

fn start_game(players: u8, mode: &mut GameMode, next: &mut NextState<GameState>) {
    mode.players = players;
    next.set(GameState::Playing);
}

fn menu_input(
    inputs: Res<Inputs>,
    mut auto: ResMut<AutoStart>,
    mut mode: ResMut<GameMode>,
    mut next: ResMut<NextState<GameState>>,
) {
    if auto.0 {
        auto.0 = false;
        let players = mode.players;
        start_game(players, &mut mode, &mut next);
    } else if inputs.choose_duo {
        start_game(2, &mut mode, &mut next);
    } else if inputs.choose_solo {
        start_game(1, &mut mode, &mut next);
    }
}

fn menu_buttons(
    mut buttons: Query<(&Interaction, &ModeButton, &mut BackgroundColor), Changed<Interaction>>,
    mut mode: ResMut<GameMode>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, button, mut bg) in &mut buttons {
        match interaction {
            Interaction::Pressed => start_game(button.0, &mut mode, &mut next),
            Interaction::Hovered => bg.0 = Color::srgba(0.2, 0.25, 0.45, 0.85),
            Interaction::None => bg.0 = Color::srgba(0.05, 0.08, 0.2, 0.75),
        }
    }
}

fn start_horn(mut commands: Commands, sfx: Res<SfxBank>) {
    sfx.play(&mut commands, Sfx::Horn, 0.5);
}

fn spawn_hud(
    mut commands: Commands,
    mode: Res<GameMode>,
    game_font: Res<GameFont>,
    window: Single<&Window, With<PrimaryWindow>>,
    cams: Query<(Entity, &PlayerCam)>,
) {
    let duo = mode.players == 2;
    let horizontal = split_horizontal(window.size());
    for (cam_entity, cam) in &cams {
        let id = cam.0;
        if id as u8 >= mode.players {
            continue;
        }
        let label = if duo { format!("J{} · ", id + 1) } else { String::new() };
        let color = if duo && id == 1 { P2_COLOR } else if duo { P1_COLOR } else { PALE };
        // Séparateur entre les deux moitiés d'écran.
        let border = match (duo, horizontal, id) {
            (false, _, _) => UiRect::ZERO,
            (true, true, 0) => UiRect::right(Val::Px(2.0)),
            (true, true, _) => UiRect::left(Val::Px(2.0)),
            (true, false, 0) => UiRect::bottom(Val::Px(2.0)),
            (true, false, _) => UiRect::top(Val::Px(2.0)),
        };
        commands.spawn((
            Hud,
            UiTargetCamera(cam_entity),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                border,
                ..default()
            },
            BorderColor::all(Color::srgba(1.0, 0.9, 0.6, 0.6)),
            children![
                (
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(14.0),
                        left: Val::Px(18.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    children![
                        (Text::new(format!("{label}Niveau 0")), font(&game_font, 36.0), TextColor(color), shadow(), HudText { player: id, kind: HudKind::Level }),
                        (Text::new("Or : 0"), font(&game_font, 22.0), TextColor(GOLD), shadow(), HudText { player: id, kind: HudKind::Gold }),
                    ],
                ),
                (
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(14.0),
                        right: Val::Px(18.0),
                        ..default()
                    },
                    children![(Text::new("Record"), font(&game_font, 24.0), TextColor(GOLD), shadow(), HudText { player: id, kind: HudKind::Best })],
                ),
                (
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(16.0),
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    children![(Text::new(""), font(&game_font, 22.0), TextColor(Color::srgb(0.6, 0.9, 1.0)), shadow(), HudText { player: id, kind: HudKind::Power })],
                ),
            ],
        ));
    }
}

fn update_hud(
    score: Res<Score>,
    mode: Res<GameMode>,
    players: Query<&Player>,
    mut texts: Query<(&HudText, &mut Text)>,
) {
    let duo = mode.players == 2;
    for (hud, mut text) in &mut texts {
        let Some(player) = players.iter().find(|p| p.id == hud.player) else { continue };
        let label = if duo { format!("J{} · ", player.id + 1) } else { String::new() };
        text.0 = match hud.kind {
            HudKind::Level => {
                if player.dead || player.dying.is_some() {
                    format!("{label}Niveau {} — au Valhalla", player.level)
                } else {
                    format!("{label}Niveau {}", player.level)
                }
            }
            HudKind::Gold => {
                if player.combo >= 2 {
                    format!("Or : {}   Berserk x{}", player.gold, player.combo)
                } else {
                    format!("Or : {}", player.gold)
                }
            }
            HudKind::Best => {
                if score.new_record {
                    format!("Nouveau record : {}", score.best)
                } else {
                    format!("Record : {}", score.best)
                }
            }
            HudKind::Power => {
                let mut parts = Vec::new();
                if player.powers.hammer_jumps > 0 {
                    parts.push(format!("Mjölnir x{}", player.powers.hammer_jumps));
                }
                if player.powers.shield {
                    parts.push("Bouclier runique".to_string());
                }
                if player.powers.mead > 0.0 {
                    parts.push(format!("Hydromel {:.0}s", player.powers.mead.ceil()));
                }
                parts.join("     ")
            }
        };
    }
}

/// Échap pendant la partie : retour au menu.
fn escape_to_menu(inputs: Res<Inputs>, mut auto: ResMut<AutoStart>, mut next: ResMut<NextState<GameState>>) {
    if inputs.escape {
        auto.0 = false;
        next.set(GameState::Menu);
    }
}

fn despawn_hud(mut commands: Commands, hud: Query<Entity, With<Hud>>) {
    for e in &hud {
        commands.entity(e).despawn();
    }
}

fn spawn_game_over(
    mut commands: Commands,
    score: Res<Score>,
    mode: Res<GameMode>,
    players: Query<&Player>,
    game_font: Res<GameFont>,
    mut delay: ResMut<GameOverDelay>,
) {
    delay.0 = 0.0;
    let mut sorted: Vec<&Player> = players.iter().collect();
    sorted.sort_by_key(|p| p.id);
    let record_line = if score.new_record {
        "NOUVEAU RECORD ! Les skalds chanteront ton nom."
    } else {
        ""
    };

    let (title, title_color, line1, line2) = if mode.players == 2 && sorted.len() == 2 {
        let (a, b) = (sorted[0], sorted[1]);
        let (title, color) = match a.level.cmp(&b.level) {
            std::cmp::Ordering::Greater => ("VICTOIRE DU JOUEUR 1", P1_COLOR),
            std::cmp::Ordering::Less => ("VICTOIRE DU JOUEUR 2", P2_COLOR),
            std::cmp::Ordering::Equal => ("ÉGALITÉ AU VALHALLA", GOLD),
        };
        (
            title,
            color,
            format!("Joueur 1 : {} {}, {} or", a.level, plural(a.level, "bouclier"), a.gold),
            format!("Joueur 2 : {} {}, {} or", b.level, plural(b.level, "bouclier"), b.gold),
        )
    } else {
        let p = sorted.first();
        (
            "TOMBÉ AU COMBAT",
            Color::srgb(0.95, 0.25, 0.15),
            format!("Boucliers franchis : {}", p.map_or(0, |p| p.level)),
            format!("Or amassé : {}", p.map_or(0, |p| p.gold)),
        )
    };

    commands.spawn((
        fullscreen_column(),
        DespawnOnExit(GameState::GameOver),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.02, 0.55)),
        children![
            (Text::new(title), font(&game_font, 64.0), TextColor(title_color), TextLayout::default().with_justify(Justify::Center), shadow()),
            (
                Text::new("Les Valkyries t'emportent vers Valhalla..."),
                font(&game_font, 26.0),
                TextColor(PALE),
                shadow(),
                Node { margin: UiRect::bottom(Val::Px(24.0)), ..default() },
            ),
            (Text::new(line1), font(&game_font, 32.0), TextColor(PALE), shadow()),
            (Text::new(line2), font(&game_font, 26.0), TextColor(GOLD), shadow()),
            (Text::new(format!("Record : {}", score.best)), font(&game_font, 26.0), TextColor(GOLD), shadow()),
            (
                Text::new(record_line),
                font(&game_font, 22.0),
                TextColor(Color::srgb(0.6, 1.0, 0.7)),
                shadow(),
                Node { margin: UiRect::top(Val::Px(10.0)), ..default() },
            ),
            (
                Text::new("- Clique pour rejouer · Échap : menu -"),
                font(&game_font, 26.0),
                TextColor(PALE),
                shadow(),
                Blink,
                Node { margin: UiRect::top(Val::Px(30.0)), ..default() },
            ),
        ],
    ));
}

fn game_over_input(
    time: Res<Time>,
    mut delay: ResMut<GameOverDelay>,
    inputs: Res<Inputs>,
    mut auto: ResMut<AutoStart>,
    mut next: ResMut<NextState<GameState>>,
) {
    delay.0 += time.delta_secs();
    if inputs.escape {
        auto.0 = false;
        next.set(GameState::Menu);
    } else if delay.0 > 0.7 && inputs.any {
        auto.0 = true;
        next.set(GameState::Menu);
    }
}

fn blink(time: Res<Time>, mut q: Query<&mut TextColor, With<Blink>>) {
    let a = 0.55 + 0.45 * (time.elapsed_secs() * 4.0).sin();
    for mut c in &mut q {
        c.0.set_alpha(a);
    }
}

