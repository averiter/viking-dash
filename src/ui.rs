//! Menu, affichage en jeu et écran de fin.

use crate::audio::{Sfx, SfxBank};
use crate::{action_pressed, AutoStart, GameFont, GameState, Powers, Score, VIEW_H};
use bevy::color::Alpha;
use bevy::prelude::*;
use bevy::text::FontSize;

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

#[derive(Component)]
struct Blink;

#[derive(Component)]
struct Hud;

#[derive(Component)]
struct LevelText;

#[derive(Component)]
struct GoldText;

#[derive(Component)]
struct BestText;

#[derive(Component)]
struct PowerText;

#[derive(Resource, Default)]
struct GameOverDelay(f32);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameOverDelay>()
            .add_systems(OnEnter(GameState::Menu), spawn_menu)
            .add_systems(Update, menu_input.run_if(in_state(GameState::Menu)))
            .add_systems(OnEnter(GameState::Playing), (spawn_hud, start_horn))
            .add_systems(Update, update_hud.run_if(in_state(GameState::Playing)))
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
                Node { margin: UiRect::top(Val::Px(-120.0)), ..default() },
            ),
            (
                Text::new("La Route de Valhalla"),
                font(&game_font, 34.0),
                TextColor(PALE),
                shadow(),
                Node { margin: UiRect::bottom(Val::Px(30.0)), ..default() },
            ),
            (
                Text::new("Clique ou ESPACE pour bondir de bouclier en bouclier.\nÉvite les créatures qui rôdent autour.\nRamasse Mjölnir, le bouclier runique et l'hydromel.\nEnchaîne les sauts vite : la fureur berserk rapporte de l'or."),
                font(&game_font, 20.0),
                TextColor(Color::srgb(0.8, 0.85, 0.95)),
                TextLayout::default().with_justify(Justify::Center),
                shadow(),
            ),
            (
                Text::new(format!("Record : {}", score.best)),
                font(&game_font, 24.0),
                TextColor(GOLD),
                shadow(),
                Node { margin: UiRect::top(Val::Px(30.0)), ..default() },
            ),
            (
                Text::new("- Clique pour partir à l'aventure -"),
                font(&game_font, 26.0),
                TextColor(PALE),
                shadow(),
                Blink,
            ),
        ],
    ));
}

fn menu_input(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut auto: ResMut<AutoStart>,
    mut next: ResMut<NextState<GameState>>,
) {
    if auto.0 {
        auto.0 = false;
        next.set(GameState::Playing);
    } else if action_pressed(&mouse, &keys, &touches) {
        next.set(GameState::Playing);
    }
}

fn spawn_hud(mut commands: Commands, game_font: Res<GameFont>) {
    commands.spawn((
        Hud,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(14.0),
            left: Val::Px(18.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        children![
            (Text::new("Niveau 0"), font(&game_font, 36.0), TextColor(PALE), shadow(), LevelText),
            (Text::new("Or : 0"), font(&game_font, 22.0), TextColor(GOLD), shadow(), GoldText),
        ],
    ));
    commands.spawn((
        Hud,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(14.0),
            right: Val::Px(18.0),
            ..default()
        },
        children![(Text::new("Record"), font(&game_font, 24.0), TextColor(GOLD), shadow(), BestText)],
    ));
    commands.spawn((
        Hud,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(16.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(Text::new(""), font(&game_font, 22.0), TextColor(Color::srgb(0.6, 0.9, 1.0)), shadow(), PowerText)],
    ));
}

fn update_hud(
    score: Res<Score>,
    powers: Res<Powers>,
    mut level: Query<&mut Text, (With<LevelText>, Without<GoldText>, Without<BestText>, Without<PowerText>)>,
    mut gold: Query<&mut Text, (With<GoldText>, Without<LevelText>, Without<BestText>, Without<PowerText>)>,
    mut best: Query<&mut Text, (With<BestText>, Without<LevelText>, Without<GoldText>, Without<PowerText>)>,
    mut power: Query<&mut Text, (With<PowerText>, Without<LevelText>, Without<GoldText>, Without<BestText>)>,
) {
    for mut t in &mut level {
        t.0 = format!("Niveau {}", score.level);
    }
    for mut t in &mut gold {
        t.0 = if score.combo >= 2 {
            format!("Or : {}   Berserk x{}", score.gold, score.combo)
        } else {
            format!("Or : {}", score.gold)
        };
    }
    for mut t in &mut best {
        t.0 = if score.new_record {
            format!("Nouveau record : {}", score.best)
        } else {
            format!("Record : {}", score.best)
        };
    }
    for mut t in &mut power {
        let mut parts = Vec::new();
        if powers.hammer_jumps > 0 {
            parts.push(format!("Mjölnir x{}", powers.hammer_jumps));
        }
        if powers.shield {
            parts.push("Bouclier runique".to_string());
        }
        if powers.mead > 0.0 {
            parts.push(format!("Hydromel {:.0}s", powers.mead.ceil()));
        }
        t.0 = parts.join("     ");
    }
}

fn start_horn(mut commands: Commands, sfx: Res<SfxBank>) {
    sfx.play(&mut commands, Sfx::Horn, 0.5);
}

fn despawn_hud(mut commands: Commands, hud: Query<Entity, With<Hud>>) {
    for e in &hud {
        commands.entity(e).despawn();
    }
}

fn spawn_game_over(mut commands: Commands, score: Res<Score>, mut delay: ResMut<GameOverDelay>, game_font: Res<GameFont>) {
    delay.0 = 0.0;
    let record_line = if score.new_record {
        "NOUVEAU RECORD ! Les skalds chanteront ton nom."
    } else {
        ""
    };
    commands.spawn((
        fullscreen_column(),
        DespawnOnExit(GameState::GameOver),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.02, 0.55)),
        children![
            (Text::new("TOMBÉ AU COMBAT"), font(&game_font, 70.0), TextColor(Color::srgb(0.95, 0.25, 0.15)), shadow()),
            (
                Text::new("Les Valkyries t'emportent vers Valhalla..."),
                font(&game_font, 26.0),
                TextColor(PALE),
                shadow(),
                Node { margin: UiRect::bottom(Val::Px(24.0)), ..default() },
            ),
            (Text::new(format!("Boucliers franchis : {}", score.level)), font(&game_font, 34.0), TextColor(PALE), shadow()),
            (Text::new(format!("Or amassé : {}", score.gold)), font(&game_font, 26.0), TextColor(GOLD), shadow()),
            (Text::new(format!("Record : {}", score.best)), font(&game_font, 26.0), TextColor(GOLD), shadow()),
            (
                Text::new(record_line),
                font(&game_font, 22.0),
                TextColor(Color::srgb(0.6, 1.0, 0.7)),
                shadow(),
                Node { margin: UiRect::top(Val::Px(10.0)), ..default() },
            ),
            (
                Text::new("- Clique pour renaître -"),
                font(&game_font, 28.0),
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
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut auto: ResMut<AutoStart>,
    mut next: ResMut<NextState<GameState>>,
) {
    delay.0 += time.delta_secs();
    if delay.0 > 0.7 && action_pressed(&mouse, &keys, &touches) {
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
