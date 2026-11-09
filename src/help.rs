//! Aide : quelques pages illustrées avec les dessins mêmes du jeu.
//!
//! Les textes sont en UI (nets à toute taille, retour à la ligne automatique). Les
//! dessins sont les maillages du jeu, posés devant une caméra à part qui passe après
//! l'UI : chaque illustration suit la case que la mise en page lui a réservée.

use crate::art::Art;
use crate::input::Inputs;
use crate::ui::{self, body_font, font, Hoverable, BUTTON_BG, GOLD, PALE, PANEL_BG};
use crate::world::{bonus_visual, enemy_visual, Bob, BonusKind, Draw, EnemyKind, Palette, Shapes, PLATFORM_R};
use crate::{GameFont, GameState};
use bevy::camera::{CameraOutputMode, ClearColorConfig};
use bevy::prelude::*;
use bevy::render::render_resource::BlendState;
use bevy::ui::UiGlobalTransform;
use bevy::window::PrimaryWindow;
use std::f32::consts::PI;

/// Les illustrations vivent loin sous le fjord : seule la caméra de l'aide les voit.
const STAGE: Vec2 = Vec2::new(0.0, -50_000.0);

/// Couleur des phrases sur la mythologie (parchemin), distincte des règles.
const LORE: Color = Color::srgb(0.84, 0.74, 0.54);

/// Page affichée ; `built` passe à vrai une fois la page construite.
#[derive(Resource, Default)]
pub struct HelpPage {
    page: usize,
    built: bool,
}

impl HelpPage {
    pub fn open(page: usize) -> Self {
        Self { page: page.min(PAGES.len() - 1), built: false }
    }
}

/// Tout ce qui appartient à la page courante (reconstruit à chaque changement de page).
#[derive(Component)]
struct HelpEntity;

/// Case réservée dans la mise en page pour une illustration.
#[derive(Component)]
struct IconSlot {
    art: Entity,
    /// Demi-étendue du dessin, en unités du jeu.
    extent: f32,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Nav {
    Prev,
    Next,
    Close,
}

#[derive(Clone, Copy)]
enum Icon {
    Viking(usize),
    /// Bouclier ; `true` ajoute l'anneau doré qui désigne la prochaine cible.
    Shield(u32, bool),
    Enemy(EnemyKind),
    Bonus(BonusKind),
    Serpent,
}

struct Entry {
    icon: Icon,
    name: &'static str,
    /// Règle du jeu.
    text: &'static str,
    /// Origine mythologique, en une phrase.
    lore: &'static str,
}

struct Page {
    title: &'static str,
    intro: &'static str,
    entries: &'static [Entry],
}

const PAGES: &[Page] = &[
    Page {
        title: "Comment jouer",
        intro: "Grimpe le plus haut possible sur la route de Valhalla. Ton record est conservé.",
        entries: &[
            Entry {
                icon: Icon::Viking(0),
                name: "Un seul geste : sauter",
                text: "Espace, clic, toucher ou bouton de manette : le viking bondit vers le bouclier suivant.",
                lore: "Le Valhalla est la halle d'Odin, où festoient les guerriers tombés au combat.",
            },
            Entry {
                icon: Icon::Shield(3, true),
                name: "De bouclier en bouclier",
                text: "Un anneau doré désigne ta cible. Posé sur un bouclier, tu ne risques rien.",
                lore: "On dit le toit du Valhalla couvert de boucliers dorés.",
            },
            Entry {
                icon: Icon::Enemy(EnemyKind::Raven),
                name: "Choisis ton moment",
                text: "Les créatures tournent autour des boucliers. En toucher une en plein saut, et les Valkyries t'emportent.",
                lore: "Les Valkyries, messagères d'Odin, choisissent les braves qui rejoindront sa halle.",
            },
        ],
    },
    Page {
        title: "Les pouvoirs",
        intro: "Ils flottent parfois au centre d'un bouclier : atterris dessus pour les prendre. Chacun rapporte aussi 5 pièces d'or.",
        entries: &[
            Entry {
                icon: Icon::Bonus(BonusKind::Hammer),
                name: "Mjölnir",
                text: "Tes 5 prochains sauts sont foudroyants : presque deux fois plus rapides.",
                lore: "Le marteau de Thor, dieu du tonnerre, forgé par les nains Brokk et Sindri : il ne manque jamais sa cible.",
            },
            Entry {
                icon: Icon::Bonus(BonusKind::Shield),
                name: "Bouclier runique",
                text: "Encaisse un coup à ta place et pulvérise la créature touchée.",
                lore: "Odin arracha le secret des runes en restant pendu neuf nuits à l'arbre-monde Yggdrasil.",
            },
            Entry {
                icon: Icon::Bonus(BonusKind::Mead),
                name: "Hydromel",
                text: "Pendant 7 secondes, toutes les créatures tournent au ralenti.",
                lore: "Au Valhalla, la chèvre Heidrun donne chaque jour l'hydromel que boivent les guerriers d'Odin.",
            },
        ],
    },
    Page {
        title: "Or et exploits",
        intro: "L'or s'amasse en prenant des pouvoirs et, surtout, en enchaînant les sauts sans hésiter.",
        entries: &[
            Entry {
                icon: Icon::Viking(0),
                name: "Berserk",
                text: "Repars moins de 0,7 s après l'atterrissage : dès deux sauts enchaînés, chacun rapporte autant d'or que la série (2, 3, 4…).",
                lore: "Les berserkir, guerriers voués à Odin, combattaient dans une fureur qui les rendait insensibles aux coups.",
            },
            Entry {
                icon: Icon::Shield(10, false),
                name: "Ragnarök",
                text: "Tous les dix boucliers, le ciel s'embrase et le serpent-monde monte la garde.",
                lore: "Le crépuscule des dieux : l'ultime bataille, où périssent Odin, Thor et le monde ancien.",
            },
            Entry {
                icon: Icon::Viking(1),
                name: "À deux",
                text: "Chacun sa moitié d'écran. Un pouvoir revient au premier arrivé ; qui monte le plus haut l'emporte.",
                lore: "Les Einherjar, guerriers du Valhalla, s'affrontent chaque jour et festoient ensemble chaque soir.",
            },
        ],
    },
    Page {
        title: "Les créatures",
        intro: "Toutes tuent au moindre contact, toujours plus vite et plus nombreuses.",
        entries: &[
            Entry {
                icon: Icon::Enemy(EnemyKind::Raven),
                name: "Corbeaux d'Odin",
                text: "Dès le départ. Plus haut, ils patrouillent aussi sur un second anneau, à contresens.",
                lore: "Hugin et Munin, « Pensée » et « Mémoire », survolent le monde chaque jour et rapportent tout à Odin.",
            },
            Entry {
                icon: Icon::Enemy(EnemyKind::Draugr),
                name: "Draugr",
                text: "Dès le bouclier 8. À partir du 12, les créatures vont par deux, parfois en sens opposés.",
                lore: "Dans les sagas, un mort à la force surhumaine qui sort de son tertre pour garder son trésor.",
            },
            Entry {
                icon: Icon::Enemy(EnemyKind::Wolf),
                name: "Loups de Fenrir",
                text: "Dès le bouclier 16. Vers le 22, leur allure devient irrégulière.",
                lore: "Fenrir, loup géant fils de Loki, fut enchaîné par les dieux ; ses fils Sköll et Hati pourchassent le soleil et la lune.",
            },
        ],
    },
    Page {
        title: "Les créatures",
        intro: "Les anciennes créatures reviennent parfois parmi les nouvelles.",
        entries: &[
            Entry {
                icon: Icon::Enemy(EnemyKind::Kraken),
                name: "Kraken",
                text: "Dès le bouclier 26 : les créatures sont désormais trois par bouclier.",
                lore: "Monstre du folklore scandinave, si vaste que les marins le prenaient pour une île.",
            },
            Entry {
                icon: Icon::Enemy(EnemyKind::Jotunn),
                name: "Jötunn de glace",
                text: "Dès le bouclier 39 : les plus rapides, souvent escortés de deux corbeaux.",
                lore: "Les Jötnar, géants ennemis des dieux ; ceux du givre descendent d'Ymir, né des glaces primordiales.",
            },
            Entry {
                icon: Icon::Serpent,
                name: "Jörmungandr",
                text: "Il garde un bouclier sur dix. Son long corps ne laisse qu'une brèche : attends-la.",
                lore: "Fils de Loki jeté à la mer par Odin, il a grandi jusqu'à encercler le monde et se mordre la queue.",
            },
        ],
    },
];

pub struct HelpPlugin;

impl Plugin for HelpPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HelpPage>()
            .add_systems(OnEnter(GameState::Help), spawn_stage)
            .add_systems(OnExit(GameState::Help), clear_page)
            .add_systems(Update, (help_input, build_page, place_icons).chain().run_if(in_state(GameState::Help)));
    }
}

/// Caméra des illustrations : rendue après l'UI, donc par-dessus le voile et les cartes.
/// Elle dessine sur fond transparent puis se fond dans l'image (l'UI de la caméra de jeu
/// est déjà à l'écran : une recopie simple l'effacerait). Une unité vaut un pixel
/// logique, ce qui rend le placement sur les cases de l'UI direct.
fn spawn_stage(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 5,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            output_mode: CameraOutputMode::Write {
                blend_state: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            },
            ..default()
        },
        Transform::from_translation(STAGE.extend(0.0)),
        DespawnOnExit(GameState::Help),
    ));
}

fn clear_page(mut commands: Commands, olds: Query<Entity, With<HelpEntity>>) {
    for e in &olds {
        commands.entity(e).despawn();
    }
}

/// Bouton de navigation ; inactif (première ou dernière page), il reste en place mais s'efface.
fn nav_button(p: &mut ChildSpawnerCommands, game_font: &GameFont, nav: Nav, label: &str, enabled: bool) {
    let alpha = if enabled { 1.0 } else { 0.25 };
    let mut button = p.spawn((
        nav,
        Node {
            padding: UiRect::axes(Val::VMin(2.4), Val::VMin(0.8)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(12.0)),
            ..default()
        },
        BorderColor::all(GOLD.with_alpha(alpha)),
        BackgroundColor(BUTTON_BG.with_alpha(0.95 * alpha)),
        children![(Text::new(label.to_string()), font(game_font, 24.0), TextColor(GOLD.with_alpha(alpha)))],
    ));
    if enabled {
        button.insert(Hoverable);
    }
}

/// Construit la page demandée : dessins sur la scène, puis textes et boutons en UI.
fn build_page(
    mut commands: Commands,
    mut page: ResMut<HelpPage>,
    game_font: Res<GameFont>,
    shapes: Res<Shapes>,
    art: Res<Art>,
    mut palette: ResMut<Palette>,
    mut mats: ResMut<Assets<ColorMaterial>>,
    olds: Query<Entity, With<HelpEntity>>,
) {
    if page.built {
        return;
    }
    page.built = true;
    for e in &olds {
        commands.entity(e).despawn();
    }
    let mut draw = Draw { shapes: &shapes, palette: &mut palette, mats: &mut mats };
    let index = page.page;
    let content = &PAGES[index];
    let last = index + 1 == PAGES.len();

    let mut root = ui::fullscreen_column();
    root.row_gap = Val::VMin(1.0);
    root.padding = UiRect::all(Val::VMin(2.0));
    let root = commands
        .spawn((
            HelpEntity,
            root,
            BackgroundColor(Color::srgba(0.0, 0.01, 0.05, 0.88)),
            children![
                (Text::new(content.title), font(&game_font, 38.0), TextColor(GOLD), ui::shadow()),
                (
                    Text::new(content.intro),
                    body_font(&game_font, 21.0),
                    TextColor(PALE),
                    TextLayout::default().with_justify(Justify::Center),
                    Node { max_width: Val::VMin(92.0), margin: UiRect::bottom(Val::VMin(0.4)), ..default() },
                ),
            ],
        ))
        .id();

    for entry in content.entries {
        let (art_entity, extent) = spawn_icon(&mut commands, &mut draw, &art, entry.icon);
        commands.entity(root).with_child((
            Node {
                width: Val::VMin(94.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::VMin(2.4), Val::VMin(0.9)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                column_gap: Val::VMin(2.8),
                ..default()
            },
            BorderColor::all(GOLD.with_alpha(0.35)),
            BackgroundColor(PANEL_BG),
            children![
                (
                    IconSlot { art: art_entity, extent },
                    Node {
                        width: Val::VMin(13.0),
                        height: Val::VMin(13.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ),
                (
                    Node {
                        flex_direction: FlexDirection::Column,
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        min_width: Val::Px(0.0),
                        row_gap: Val::VMin(0.3),
                        ..default()
                    },
                    children![
                        (Text::new(entry.name), font(&game_font, 27.0), TextColor(GOLD), ui::shadow()),
                        (Text::new(entry.text), body_font(&game_font, 20.0), TextColor(PALE)),
                        (Text::new(entry.lore), body_font(&game_font, 20.0), TextColor(LORE)),
                    ],
                ),
            ],
        ));
    }

    let hint = if ui::touch_device() {
        "Touche l'écran pour tourner la page"
    } else {
        "← → : pages · Échap : menu"
    };
    commands.entity(root).with_children(|p| {
        p.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::VMin(2.4),
            margin: UiRect::top(Val::VMin(0.4)),
            ..default()
        })
        .with_children(|p| {
            nav_button(p, &game_font, Nav::Close, "Menu", true);
            nav_button(p, &game_font, Nav::Prev, "Précédent", index > 0);
            p.spawn((
                Text::new(format!("{} / {}", index + 1, PAGES.len())),
                font(&game_font, 24.0),
                TextColor(PALE),
                ui::shadow(),
            ));
            nav_button(p, &game_font, Nav::Next, "Suivant", !last);
        });
        p.spawn((Text::new(hint), body_font(&game_font, 20.0), TextColor(PALE.with_alpha(0.75))));
    });
}

/// Pose une illustration sur la scène (cachée tant que sa case n'est pas mise en page).
/// Renvoie l'entité et la demi-étendue du dessin.
fn spawn_icon(commands: &mut Commands, d: &mut Draw, art: &Art, icon: Icon) -> (Entity, f32) {
    let mut root = commands.spawn((HelpEntity, Transform::from_translation(STAGE.extend(0.0)), Visibility::Hidden));
    let extent = match icon {
        Icon::Viking(id) => {
            root.with_child(art.at(&art.viking[id % 2], Vec2::ZERO, 0.0));
            27.0
        }
        Icon::Shield(index, target) => {
            root.with_child(art.at(art.shield(index), Vec2::ZERO, 0.0));
            if target {
                let r = (PLATFORM_R + 13.0) / 0.86;
                root.with_child((
                    Sprite {
                        image: d.shapes.ring_tex.clone(),
                        color: Color::srgba(1.0, 0.9, 0.55, 0.65),
                        custom_size: Some(Vec2::splat(2.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, 1.0).with_scale(Vec3::new(r, r, 1.0)),
                ));
                PLATFORM_R + 22.0
            } else {
                PLATFORM_R + 4.0
            }
        }
        Icon::Enemy(kind) => {
            root.with_children(|q| enemy_visual(q, d, art, kind, 0));
            30.0
        }
        Icon::Bonus(kind) => {
            // Même flottement que sur les boucliers.
            root.with_children(|q| {
                q.spawn((Bob { amp: 3.0, phase: 0.0, base_y: 0.0 }, Transform::default(), Visibility::default()))
                    .with_children(|q| bonus_visual(q, d, art, kind));
            });
            25.0
        }
        Icon::Serpent => {
            // Tête et corps sur un arc d'orbite, comme autour d'un bouclier.
            const R: f32 = 100.0;
            root.with_children(|q| {
                for seg in 0..4 {
                    let kind = if seg == 0 { EnemyKind::SerpentHead } else { EnemyKind::SerpentBody };
                    let a = PI / 2.0 - 0.45 + seg as f32 * 0.3;
                    q.spawn((
                        Transform {
                            translation: (Vec2::from_angle(a) * R - Vec2::new(0.0, R * 0.95)).extend(0.0),
                            rotation: Quat::from_rotation_z(a - PI / 2.0),
                            scale: Vec3::new(1.0, -1.0, 1.0),
                        },
                        Visibility::default(),
                    ))
                    .with_children(|q| enemy_visual(q, d, art, kind, seg));
                }
            });
            56.0
        }
    };
    (root.id(), extent)
}

/// Amène chaque illustration sur sa case et l'ajuste à sa taille.
fn place_icons(
    window: Single<&Window, With<PrimaryWindow>>,
    slots: Query<(&IconSlot, &ComputedNode, &UiGlobalTransform)>,
    mut arts: Query<(&mut Transform, &mut Visibility)>,
) {
    let size = window.size();
    let px = 1.0 / window.scale_factor();
    for (slot, node, transform) in &slots {
        let Ok((mut tf, mut vis)) = arts.get_mut(slot.art) else { continue };
        let side = node.size().min_element() * px;
        if side <= 0.0 {
            continue;
        }
        let center = transform.translation * px;
        let pos = STAGE + Vec2::new(center.x - size.x / 2.0, size.y / 2.0 - center.y);
        let scale = side / (2.0 * slot.extent);
        tf.translation = pos.extend(0.0);
        tf.scale = Vec3::new(scale, scale, 1.0);
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
    }
}

fn help_input(
    inputs: Res<Inputs>,
    window: Single<&Window, With<PrimaryWindow>>,
    buttons: Query<(&ComputedNode, &UiGlobalTransform, &Nav)>,
    mut page: ResMut<HelpPage>,
    mut next: ResMut<NextState<GameState>>,
) {
    // Un bouton touché l'emporte ; un contact ailleurs tourne la page.
    let scale = window.scale_factor();
    let tapped = buttons
        .iter()
        .find(|(node, transform, _)| inputs.taps.iter().any(|tap| node.contains_point(**transform, *tap * scale)))
        .map(|(_, _, nav)| *nav);
    let last = page.page + 1 == PAGES.len();
    let nav = if let Some(nav) = tapped {
        nav
    } else if inputs.back {
        Nav::Close
    } else if inputs.prev {
        Nav::Prev
    } else if inputs.next || !inputs.taps.is_empty() {
        if last { Nav::Close } else { Nav::Next }
    } else {
        return;
    };
    match nav {
        Nav::Close => next.set(GameState::Menu),
        Nav::Prev if page.page > 0 => *page = HelpPage::open(page.page - 1),
        Nav::Next if !last => *page = HelpPage::open(page.page + 1),
        _ => {}
    }
}
