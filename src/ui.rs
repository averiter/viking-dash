//! Menu, affichage en jeu (un par joueur) et écran de fin.

use crate::audio::{Sfx, SfxBank};
use crate::help::HelpPage;
use crate::input::Inputs;
use crate::player::{Player, NOTICE_DURATION};
use crate::{split_horizontal, AutoStart, GameFont, GameMode, GameState, PlayerCam, Score, VIEW_H};
use bevy::color::Alpha;
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::{ComputedUiRenderTargetInfo, UiGlobalTransform, UiTargetCamera};
use bevy::window::PrimaryWindow;

/// `size` est exprimée pour une vue de 720 de haut ; elle suit la plus petite
/// dimension de l'écran (portrait sur téléphone, paysage sur PC).
pub fn font(game_font: &GameFont, size: f32) -> TextFont {
    TextFont {
        font: game_font.display.clone(),
        font_size: FontSize::VMin(size * 100.0 / VIEW_H),
        ..default()
    }
}

/// Comme `font`, pour les textes courants (consignes, aides, HUD secondaire).
pub fn body_font(game_font: &GameFont, size: f32) -> TextFont {
    TextFont {
        font: game_font.body.clone(),
        ..font(game_font, size)
    }
}

/// Police pour les textes du monde (`Text2d`), en unités monde.
pub fn world_font(game_font: &GameFont, size: f32) -> TextFont {
    TextFont {
        font: game_font.display.clone(),
        font_size: FontSize::Px(size),
        ..default()
    }
}

pub const GOLD: Color = Color::srgb(1.0, 0.85, 0.35);
pub const PALE: Color = Color::srgb(0.9, 0.92, 1.0);
const P1_COLOR: Color = Color::srgb(1.0, 0.55, 0.45);
const P2_COLOR: Color = Color::srgb(0.55, 0.75, 1.0);
const NOTICE_COLOR: Color = Color::srgb(1.0, 0.92, 0.7);
pub const PANEL_BG: Color = Color::srgba(0.02, 0.04, 0.12, 0.84);
pub const BUTTON_BG: Color = Color::srgba(0.10, 0.15, 0.34, 0.95);
const BUTTON_HOVER: Color = Color::srgba(0.24, 0.31, 0.56, 0.95);

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
    Notice,
}

#[derive(Component)]
struct HudText {
    player: u8,
    kind: HudKind,
}

/// Bouton du menu : lance une partie à `n` joueurs.
#[derive(Component)]
struct ModeButton(u8);

/// Bouton du menu : ouvre l'aide.
#[derive(Component)]
struct HelpButton;

/// Bouton qui s'éclaircit sous le curseur de la souris.
#[derive(Component)]
pub struct Hoverable;

/// Aide d'un bouton du menu, adaptée à l'appareil et à l'orientation.
#[derive(Component)]
struct ModeHint(u8);

#[derive(Resource, Default)]
struct GameOverDelay(f32);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameOverDelay>()
            .add_systems(OnEnter(GameState::Menu), spawn_menu)
            .add_systems(Update, (menu_input, menu_taps, menu_hints).run_if(in_state(GameState::Menu)))
            .add_systems(OnEnter(GameState::Playing), (spawn_hud, start_horn))
            .add_systems(Update, (update_hud, escape_to_menu).run_if(in_state(GameState::Playing)))
            .add_systems(OnEnter(GameState::GameOver), (despawn_hud, spawn_game_over))
            .add_systems(Update, game_over_input.run_if(in_state(GameState::GameOver)))
            .add_systems(Update, (blink, scale_shadows, hover_buttons));
    }
}

/// Ombre portée ; son décalage est ajusté à la taille du texte par `scale_shadows`.
pub fn shadow() -> TextShadow {
    TextShadow {
        offset: Vec2::splat(2.0),
        color: Color::srgba(0.0, 0.0, 0.0, 0.75),
    }
}

/// Décalage de l'ombre proportionnel au corps du texte : discret sur les petites
/// lignes, marqué sur les titres.
fn scale_shadows(mut q: Query<(&TextFont, &ComputedUiRenderTargetInfo, &mut TextShadow)>) {
    for (text_font, target, mut shadow) in &mut q {
        let view = target.logical_size();
        let px = match text_font.font_size {
            FontSize::VMin(v) => v / 100.0 * view.min_element(),
            FontSize::Px(p) => p,
            _ => continue,
        };
        if px <= 0.0 {
            continue;
        }
        let offset = Vec2::splat((px * 0.055).clamp(1.0, 5.0));
        if shadow.offset != offset {
            shadow.offset = offset;
        }
    }
}

/// Écran tactile ? (sur le web uniquement : les aides parlent alors de toucher, pas de touches)
pub fn touch_device() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window().is_some_and(|w| w.navigator().max_touch_points() > 0)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}

/// Aide d'un bouton du menu : seulement le cas qui concerne ce joueur-ci.
fn mode_hint(players: u8, touch: bool, horizontal: bool) -> &'static str {
    match (players, touch, horizontal) {
        (1, false, _) => "Espace, clic ou bouton A",
        (1, true, _) => "Touche pour partir",
        (_, false, _) => "Touche 2 ou bouton Y\nJ1 : Espace · J2 : Entrée",
        (_, true, true) => "J1 : moitié gauche\nJ2 : moitié droite",
        (_, true, false) => "J1 : moitié haute\nJ2 : moitié basse",
    }
}

pub fn fullscreen_column() -> Node {
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
    let title_font = font(game_font, 32.0);
    let hint_font = body_font(game_font, 20.0);
    (
        ModeButton(players),
        Hoverable,
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(Val::VMin(3.0), Val::VMin(1.8)),
            border: UiRect::all(Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(14.0)),
            row_gap: Val::VMin(0.6),
            ..default()
        },
        BorderColor::all(GOLD),
        BackgroundColor(BUTTON_BG),
        Children::spawn((
            Spawn((Text::new(title), title_font, TextColor(GOLD), shadow())),
            Spawn((
                Text::new(hint),
                hint_font,
                TextColor(PALE),
                TextLayout::default().with_justify(Justify::Center),
                shadow(),
                ModeHint(players),
            )),
        )),
    )
}

fn spawn_menu(
    mut commands: Commands,
    score: Res<Score>,
    auto: Res<AutoStart>,
    game_font: Res<GameFont>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    if auto.0 {
        return;
    }
    let touch = touch_device();
    let horizontal = split_horizontal(window.size());
    // Les textes occupent le haut de l'écran : le viking et son bouclier restent
    // visibles dessous, sans rien derrière les lignes à lire.
    let mut root = fullscreen_column();
    root.padding = UiRect::bottom(Val::Vh(40.0));
    root.row_gap = Val::VMin(0.6);
    commands.spawn((
        root,
        DespawnOnExit(GameState::Menu),
        children![
            (Text::new("VIKING DASH"), font(&game_font, 88.0), TextColor(GOLD), shadow()),
            (
                Text::new("La Route de Valhalla"),
                font(&game_font, 34.0),
                TextColor(PALE),
                shadow(),
                Node { margin: UiRect::bottom(Val::VMin(1.8)), ..default() },
            ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    max_width: Val::VMin(94.0),
                    padding: UiRect::axes(Val::VMin(3.5), Val::VMin(2.4)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(18.0)),
                    row_gap: Val::VMin(1.8),
                    ..default()
                },
                BorderColor::all(GOLD.with_alpha(0.45)),
                BackgroundColor(PANEL_BG),
                children![
                    (
                        Text::new("Saute de bouclier en bouclier, évite les créatures."),
                        body_font(&game_font, 22.0),
                        TextColor(PALE),
                        TextLayout::default().with_justify(Justify::Center),
                    ),
                    (
                        Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::VMin(3.0),
                            ..default()
                        },
                        children![
                            mode_button(&game_font, 1, "1 JOUEUR", mode_hint(1, touch, horizontal)),
                            mode_button(&game_font, 2, "2 JOUEURS", mode_hint(2, touch, horizontal)),
                        ],
                    ),
                    (
                        Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::VMin(4.0),
                            ..default()
                        },
                        children![
                            (
                                HelpButton,
                                Hoverable,
                                Node {
                                    flex_direction: FlexDirection::Row,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::axes(Val::VMin(2.4), Val::VMin(1.0)),
                                    border: UiRect::all(Val::Px(2.0)),
                                    border_radius: BorderRadius::all(Val::Px(12.0)),
                                    column_gap: Val::VMin(1.6),
                                    ..default()
                                },
                                BorderColor::all(GOLD),
                                BackgroundColor(BUTTON_BG),
                                children![
                                    (Text::new("AIDE"), font(&game_font, 26.0), TextColor(GOLD), shadow()),
                                    (
                                        Text::new(if touch { "Comment jouer" } else { "Touche H" }),
                                        body_font(&game_font, 20.0),
                                        TextColor(PALE),
                                        shadow(),
                                    ),
                                ],
                            ),
                            (Text::new(format!("Record : {}", score.best)), font(&game_font, 26.0), TextColor(GOLD)),
                        ],
                    ),
                ],
            ),
        ],
    ));
}

/// Tient les aides des boutons à jour quand l'écran change d'orientation.
fn menu_hints(window: Single<&Window, With<PrimaryWindow>>, mut hints: Query<(&ModeHint, &mut Text)>) {
    let touch = touch_device();
    let horizontal = split_horizontal(window.size());
    for (hint, mut text) in &mut hints {
        let wanted = mode_hint(hint.0, touch, horizontal);
        if text.0 != wanted {
            text.0 = wanted.to_string();
        }
    }
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
    mut page: ResMut<HelpPage>,
) {
    if auto.0 {
        auto.0 = false;
        let players = mode.players;
        start_game(players, &mut mode, &mut next);
    } else if inputs.help {
        *page = HelpPage::open(0);
        next.set(GameState::Help);
    } else if inputs.choose_duo {
        start_game(2, &mut mode, &mut next);
    } else if inputs.choose_solo {
        start_game(1, &mut mode, &mut next);
    }
}

/// Survol à la souris : éclaircit le bouton sous le curseur. Les clics passent par
/// les contacts de `Inputs`, ce qui évite de dépendre du picking de Bevy.
fn hover_buttons(
    window: Single<&Window, With<PrimaryWindow>>,
    mut buttons: Query<(&ComputedNode, &UiGlobalTransform, &mut BackgroundColor), With<Hoverable>>,
) {
    let cursor = window.cursor_position().map(|p| p * window.scale_factor());
    for (node, transform, mut bg) in &mut buttons {
        let hovered = cursor.is_some_and(|p| node.contains_point(*transform, p));
        let color = if hovered { BUTTON_HOVER } else { BUTTON_BG };
        if bg.0 != color {
            bg.0 = color;
        }
    }
}

/// Tactile : un doigt posé sur un bouton du menu le déclenche, même si le contact
/// commence et finit dans la même frame (fréquent sur un téléphone lent). Sert aussi
/// au clic de souris.
fn menu_taps(
    inputs: Res<Inputs>,
    window: Single<&Window, With<PrimaryWindow>>,
    buttons: Query<(&ComputedNode, &UiGlobalTransform, &ModeButton)>,
    help: Query<(&ComputedNode, &UiGlobalTransform), With<HelpButton>>,
    mut mode: ResMut<GameMode>,
    mut next: ResMut<NextState<GameState>>,
    mut page: ResMut<HelpPage>,
) {
    if inputs.taps.is_empty() {
        return;
    }
    let scale = window.scale_factor();
    for (node, transform) in &help {
        if inputs.taps.iter().any(|tap| node.contains_point(*transform, *tap * scale)) {
            *page = HelpPage::open(0);
            next.set(GameState::Help);
            return;
        }
    }
    for (node, transform, button) in &buttons {
        if inputs.taps.iter().any(|tap| node.contains_point(*transform, *tap * scale)) {
            start_game(button.0, &mut mode, &mut next);
            return;
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
                // Bandeaux sombres dégradés : le HUD reste lisible sur l'aurore et les boucliers.
                hud_strip(true),
                hud_strip(false),
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
                        (Text::new("Or : 0"), body_font(&game_font, 22.0), TextColor(GOLD), shadow(), HudText { player: id, kind: HudKind::Gold }),
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
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::VMin(0.6),
                        ..default()
                    },
                    children![
                        (Text::new(""), font(&game_font, 28.0), TextColor(NOTICE_COLOR), shadow(), HudText { player: id, kind: HudKind::Notice }),
                        (Text::new(""), body_font(&game_font, 22.0), TextColor(Color::srgb(0.6, 0.9, 1.0)), shadow(), HudText { player: id, kind: HudKind::Power }),
                    ],
                ),
            ],
        ));
    }
}

/// Bandeau dégradé en haut ou en bas d'une vue, derrière les textes du HUD.
fn hud_strip(top: bool) -> impl Bundle {
    let dark = Color::srgba(0.0, 0.0, 0.04, if top { 0.7 } else { 0.5 });
    let clear = dark.with_alpha(0.0);
    let (height, stops) = if top {
        (Val::VMin(19.0), vec![ColorStop::auto(dark), ColorStop::auto(clear)])
    } else {
        (Val::VMin(15.0), vec![ColorStop::auto(clear), ColorStop::auto(dark)])
    };
    let mut node = Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height,
        ..default()
    };
    if top {
        node.top = Val::Px(0.0);
    } else {
        node.bottom = Val::Px(0.0);
    }
    (node, BackgroundGradient::from(LinearGradient::to_bottom(stops)))
}

fn update_hud(
    score: Res<Score>,
    mode: Res<GameMode>,
    players: Query<&Player>,
    mut texts: Query<(&HudText, &mut Text, &mut TextColor)>,
) {
    let duo = mode.players == 2;
    for (hud, mut text, mut color) in &mut texts {
        let Some(player) = players.iter().find(|p| p.id == hud.player) else { continue };
        let label = if duo { format!("J{} · ", player.id + 1) } else { String::new() };
        let wanted = match hud.kind {
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
            HudKind::Notice => {
                // Nouvelle créature droit devant : apparaît, tient, puis s'efface.
                let (name, left) = player.notice.filter(|_| player.alive()).unwrap_or(("", 0.0));
                let alpha = (left / 0.6).min((NOTICE_DURATION - left) / 0.2).clamp(0.0, 1.0);
                if color.0.alpha() != alpha {
                    color.0.set_alpha(alpha);
                }
                name.to_string()
            }
        };
        // Ne réécrit le texte que s'il change : sinon il serait remis en page à chaque frame.
        if text.0 != wanted {
            text.0 = wanted;
        }
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
        BackgroundColor(Color::srgba(0.0, 0.0, 0.03, 0.72)),
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
                body_font(&game_font, 22.0),
                TextColor(Color::srgb(0.6, 1.0, 0.7)),
                shadow(),
                Node { margin: UiRect::top(Val::Px(10.0)), ..default() },
            ),
            (
                Text::new(if touch_device() { "Touche pour rejouer" } else { "Clique pour rejouer · Échap : menu" }),
                body_font(&game_font, 24.0),
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
    // Pulsation légère : le texte reste toujours lisible.
    let a = 0.85 + 0.15 * (time.elapsed_secs() * 4.0).sin();
    for mut c in &mut q {
        c.0.set_alpha(a);
    }
}

