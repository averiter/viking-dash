//! Le monde : boucliers-plateformes, créatures en orbite, bonus, génération
//! procédurale et difficulté progressive.

use crate::{player, ui, GameEntity, GameState, MainCamera, Powers, Score};
use bevy::color::Luminance;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use rand::Rng;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

pub const PLATFORM_R: f32 = 54.0;
pub const ORBIT_R: f32 = 100.0;
pub const ORBIT_R2: f32 = 138.0;
pub const ENEMY_R: f32 = 17.0;
pub const PLAYER_R: f32 = 15.0;

const SHIELD_COLORS: [Color; 6] = [
    Color::srgb(0.70, 0.18, 0.14),
    Color::srgb(0.16, 0.36, 0.62),
    Color::srgb(0.82, 0.62, 0.18),
    Color::srgb(0.22, 0.50, 0.30),
    Color::srgb(0.55, 0.22, 0.52),
    Color::srgb(0.86, 0.84, 0.76),
];

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Track>()
            .init_resource::<Palette>()
            .init_resource::<Seen>()
            .add_systems(PreStartup, setup_shapes)
            .add_systems(OnEnter(GameState::Menu), reset_world)
            .add_systems(
                Update,
                (extend_track, orbit_enemies, animate_wobble, animate_bob),
            );
    }
}

// ---------- primitives de dessin ----------

#[derive(Resource)]
pub struct Shapes {
    pub circle: Handle<Mesh>,
    pub square: Handle<Mesh>,
    pub tri: Handle<Mesh>,
    pub ring: Handle<Mesh>,
    pub thin_ring: Handle<Mesh>,
    pub hex: Handle<Mesh>,
}

/// Cache de matériaux par couleur, pour ne pas créer un asset par forme.
#[derive(Resource, Default)]
pub struct Palette {
    cache: HashMap<[u8; 4], Handle<ColorMaterial>>,
}

impl Palette {
    pub fn get(&mut self, mats: &mut Assets<ColorMaterial>, color: Color) -> Handle<ColorMaterial> {
        let key = color.to_srgba().to_u8_array();
        self.cache
            .entry(key)
            .or_insert_with(|| mats.add(ColorMaterial::from(color)))
            .clone()
    }
}

fn setup_shapes(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(Shapes {
        circle: meshes.add(Circle::new(1.0)),
        square: meshes.add(Rectangle::new(1.0, 1.0)),
        tri: meshes.add(Triangle2d::new(
            Vec2::new(-0.5, -0.5),
            Vec2::new(0.5, -0.5),
            Vec2::new(0.0, 0.5),
        )),
        ring: meshes.add(Annulus::new(0.88, 1.0)),
        thin_ring: meshes.add(Annulus::new(0.965, 1.0)),
        hex: meshes.add(RegularPolygon::new(1.0, 6)),
    });
}

pub type ShapeBundle = (Mesh2d, MeshMaterial2d<ColorMaterial>, Transform);

/// Petit pinceau : fabrique des bundles de formes colorées.
pub struct Draw<'a> {
    pub shapes: &'a Shapes,
    pub palette: &'a mut Palette,
    pub mats: &'a mut Assets<ColorMaterial>,
}

impl Draw<'_> {
    fn bundle(&mut self, mesh: &Handle<Mesh>, color: Color, pos: Vec2, z: f32, scale: Vec2, rot: f32) -> ShapeBundle {
        (
            Mesh2d(mesh.clone()),
            MeshMaterial2d(self.palette.get(self.mats, color)),
            Transform {
                translation: pos.extend(z),
                rotation: Quat::from_rotation_z(rot),
                scale: scale.extend(1.0),
            },
        )
    }
    pub fn circle(&mut self, color: Color, pos: Vec2, z: f32, r: f32) -> ShapeBundle {
        let m = self.shapes.circle.clone();
        self.bundle(&m, color, pos, z, Vec2::splat(r), 0.0)
    }
    pub fn ellipse(&mut self, color: Color, pos: Vec2, z: f32, rx: f32, ry: f32, rot: f32) -> ShapeBundle {
        let m = self.shapes.circle.clone();
        self.bundle(&m, color, pos, z, Vec2::new(rx, ry), rot)
    }
    pub fn rect(&mut self, color: Color, pos: Vec2, z: f32, size: Vec2, rot: f32) -> ShapeBundle {
        let m = self.shapes.square.clone();
        self.bundle(&m, color, pos, z, size, rot)
    }
    pub fn tri(&mut self, color: Color, pos: Vec2, z: f32, size: Vec2, rot: f32) -> ShapeBundle {
        let m = self.shapes.tri.clone();
        self.bundle(&m, color, pos, z, size, rot)
    }
    pub fn ring(&mut self, color: Color, pos: Vec2, z: f32, r: f32) -> ShapeBundle {
        let m = self.shapes.ring.clone();
        self.bundle(&m, color, pos, z, Vec2::splat(r), 0.0)
    }
    pub fn thin_ring(&mut self, color: Color, pos: Vec2, z: f32, r: f32) -> ShapeBundle {
        let m = self.shapes.thin_ring.clone();
        self.bundle(&m, color, pos, z, Vec2::splat(r), 0.0)
    }
    pub fn hex(&mut self, color: Color, pos: Vec2, z: f32, r: f32) -> ShapeBundle {
        let m = self.shapes.hex.clone();
        self.bundle(&m, color, pos, z, Vec2::splat(r), 0.0)
    }
}

// ---------- composants ----------

#[derive(Component)]
#[allow(dead_code)]
pub struct Platform {
    pub index: u32,
}

#[derive(Component)]
pub struct Orbiter {
    pub platform: u32,
    pub angle: f32,
    pub speed: f32,
    pub radius: f32,
    pub wobble: f32,
    pub phase: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind {
    Raven,
    Draugr,
    Wolf,
    Kraken,
    Jotunn,
    SerpentHead,
    SerpentBody,
}

#[derive(Component)]
pub struct Enemy {
    pub radius: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BonusKind {
    Hammer,
    Shield,
    Mead,
}

#[derive(Component)]
pub struct Bonus {
    pub kind: BonusKind,
    pub platform: u32,
}

/// Oscillation de rotation (ailes, tentacules).
#[derive(Component)]
pub struct Wobble {
    pub amp: f32,
    pub freq: f32,
    pub phase: f32,
    pub base: f32,
}

/// Flottement vertical + rotation lente (bonus).
#[derive(Component)]
pub struct Bob {
    pub amp: f32,
    pub phase: f32,
    pub base_y: f32,
}

// ---------- piste ----------

pub struct TrackEntry {
    pub entity: Entity,
    pub pos: Vec2,
    pub index: u32,
}

#[derive(Resource, Default)]
pub struct Track {
    pub platforms: Vec<TrackEntry>,
    pub next: u32,
    pub last: Vec2,
}

impl Track {
    pub fn pos(&self, index: u32) -> Option<Vec2> {
        self.platforms.iter().find(|p| p.index == index).map(|p| p.pos)
    }
}

/// Créatures déjà rencontrées (masque de bits par `EnemyKind`).
#[derive(Resource, Default)]
pub struct Seen(pub u32);

pub fn kind_name(kind: EnemyKind) -> &'static str {
    match kind {
        EnemyKind::Raven => "Corbeaux d'Odin",
        EnemyKind::Draugr => "Draugr",
        EnemyKind::Wolf => "Loups de Fenrir",
        EnemyKind::Kraken => "Kraken",
        EnemyKind::Jotunn => "Jötunn de glace",
        EnemyKind::SerpentHead | EnemyKind::SerpentBody => "Jörmungandr !",
    }
}

struct Spec {
    count: u32,
    speed: f32,
    wobble: f32,
    second_ring: u32,
    serpent: bool,
    mixed_dirs: bool,
    kind: EnemyKind,
    bonus: Option<BonusKind>,
}

fn difficulty(n: u32, rng: &mut impl Rng) -> Spec {
    let nf = n as f32;
    let tier = match n {
        0..=7 => 0,
        8..=15 => 1,
        16..=25 => 2,
        26..=38 => 3,
        _ => 4,
    };
    let tier = if tier > 0 && rng.random_bool(0.3) {
        rng.random_range(0..tier)
    } else {
        tier
    };
    let kind = [
        EnemyKind::Raven,
        EnemyKind::Draugr,
        EnemyKind::Wolf,
        EnemyKind::Kraken,
        EnemyKind::Jotunn,
    ][tier];
    let serpent = n > 0 && n % 10 == 0;
    let bonus = if n >= 3 && !serpent && rng.random_bool(0.17) {
        Some(match rng.random_range(0..10) {
            0..=3 => BonusKind::Hammer,
            4..=6 => BonusKind::Shield,
            _ => BonusKind::Mead,
        })
    } else {
        None
    };
    Spec {
        count: if n == 0 { 0 } else { (1 + n / 12).min(3) },
        speed: (1.3 + nf * 0.045).min(3.8) * rng.random_range(0.85..1.15),
        wobble: if n >= 22 { 0.45 } else { 0.0 },
        second_ring: if n >= 15 && rng.random_bool(0.35) {
            if n >= 35 && rng.random_bool(0.5) {
                2
            } else {
                1
            }
        } else {
            0
        },
        serpent,
        mixed_dirs: n >= 12 && rng.random_bool(0.5),
        kind,
        bonus,
    }
}

fn reset_world(
    mut commands: Commands,
    olds: Query<Entity, With<GameEntity>>,
    mut track: ResMut<Track>,
    mut seen: ResMut<Seen>,
    mut score: ResMut<Score>,
    mut powers: ResMut<Powers>,
    shapes: Res<Shapes>,
    mut palette: ResMut<Palette>,
    mut mats: ResMut<Assets<ColorMaterial>>,
    mut cam: Single<&mut Transform, With<MainCamera>>,
) {
    for e in &olds {
        commands.entity(e).despawn();
    }
    *track = Track::default();
    seen.0 = 0;
    score.level = 0;
    score.gold = 0;
    score.combo = 0;
    score.new_record = false;
    *powers = Powers::default();
    cam.translation = Vec3::new(0.0, 120.0, 0.0);
    let mut draw = Draw {
        shapes: &shapes,
        palette: &mut palette,
        mats: &mut mats,
    };
    // `VIKING_START=n` permet de démarrer directement au niveau n (tests).
    let start = std::env::var("VIKING_START").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    score.level = start;
    spawn_platform(&mut commands, &mut draw, &mut track, &mut seen, start, Vec2::ZERO);
    player::spawn_player(&mut commands, &mut draw, Vec2::ZERO, start);
}

fn extend_track(
    mut commands: Commands,
    mut track: ResMut<Track>,
    mut seen: ResMut<Seen>,
    shapes: Res<Shapes>,
    mut palette: ResMut<Palette>,
    mut mats: ResMut<Assets<ColorMaterial>>,
    cam: Single<&Transform, With<MainCamera>>,
) {
    let cam_y = cam.translation.y;
    let mut draw = Draw {
        shapes: &shapes,
        palette: &mut palette,
        mats: &mut mats,
    };
    let mut rng = rand::rng();
    while track.next > 0 && track.last.y < cam_y + 1200.0 {
        let n = track.next;
        let gap = 215.0 + (n.min(30) as f32) * 1.5;
        let lo = (track.last.x - 220.0).max(-230.0);
        let hi = (track.last.x + 220.0).min(230.0);
        let x = rng.random_range(lo..hi);
        let pos = Vec2::new(x, track.last.y + gap);
        spawn_platform(&mut commands, &mut draw, &mut track, &mut seen, n, pos);
    }
    // Nettoyage des plateformes passées.
    let mut i = 0;
    while i < track.platforms.len() {
        if track.platforms[i].pos.y < cam_y - 900.0 {
            let e = track.platforms.remove(i).entity;
            commands.entity(e).despawn();
        } else {
            i += 1;
        }
    }
}

fn spawn_platform(
    commands: &mut Commands,
    draw: &mut Draw,
    track: &mut Track,
    seen: &mut Seen,
    index: u32,
    pos: Vec2,
) {
    let mut rng = rand::rng();
    let spec = difficulty(index, &mut rng);
    let face = SHIELD_COLORS[index as usize % SHIELD_COLORS.len()];
    let dark = face.darker(0.12);
    let iron = Color::srgb(0.42, 0.44, 0.48);
    let iron_light = Color::srgb(0.72, 0.74, 0.78);

    let root = commands
        .spawn((
            Platform { index },
            GameEntity,
            Transform::from_translation(pos.extend(0.0)),
            Visibility::default(),
        ))
        .id();

    let mut label: Option<&'static str> = None;
    let kind_bit = 1 << (if spec.serpent { 5 } else { spec.kind as u32 });
    if spec.count > 0 || spec.serpent {
        if seen.0 & kind_bit == 0 {
            seen.0 |= kind_bit;
            label = Some(kind_name(if spec.serpent { EnemyKind::SerpentHead } else { spec.kind }));
        }
    }

    commands.entity(root).with_children(|p| {
        // Chemins d'orbite.
        let path = Color::srgba(0.8, 0.85, 1.0, 0.13);
        p.spawn(draw.thin_ring(path, Vec2::ZERO, 0.5, ORBIT_R));
        if spec.second_ring > 0 {
            p.spawn(draw.thin_ring(path, Vec2::ZERO, 0.5, ORBIT_R2));
        }
        // Bouclier.
        p.spawn(draw.circle(Color::srgba(0.0, 0.0, 0.05, 0.45), Vec2::new(5.0, -7.0), 0.8, PLATFORM_R));
        p.spawn(draw.circle(face, Vec2::ZERO, 1.0, PLATFORM_R));
        p.spawn(draw.rect(dark, Vec2::ZERO, 1.1, Vec2::new(PLATFORM_R * 1.9, 7.0), 0.0));
        p.spawn(draw.rect(dark, Vec2::ZERO, 1.1, Vec2::new(7.0, PLATFORM_R * 1.9), 0.0));
        p.spawn(draw.ring(iron, Vec2::ZERO, 1.2, PLATFORM_R));
        p.spawn(draw.circle(iron, Vec2::ZERO, 1.3, 11.0));
        p.spawn(draw.circle(iron_light, Vec2::new(-3.0, 3.0), 1.4, 4.0));
        for k in 0..8 {
            let a = k as f32 * TAU / 8.0 + PI / 8.0;
            p.spawn(draw.circle(iron_light, Vec2::from_angle(a) * (PLATFORM_R - 3.5), 1.3, 2.4));
        }

        // Créatures.
        if spec.serpent {
            let dir = if rng.random_bool(0.5) { 1.0 } else { -1.0 };
            let speed = spec.speed * 0.8 * dir;
            let start = rng.random_range(0.0..TAU);
            for seg in 0..6 {
                let kind = if seg == 0 { EnemyKind::SerpentHead } else { EnemyKind::SerpentBody };
                let radius = if seg == 0 { 20.0 } else { 14.0 };
                p.spawn((
                    Orbiter {
                        platform: index,
                        angle: start - seg as f32 * 0.3 * dir,
                        speed,
                        radius: ORBIT_R,
                        wobble: 0.0,
                        phase: seg as f32,
                    },
                    Enemy { radius },
                    Transform::default(),
                    Visibility::default(),
                ))
                .with_children(|q| enemy_visual(q, draw, kind, seg));
            }
        } else {
            let base_dir = if rng.random_bool(0.5) { 1.0 } else { -1.0 };
            let offset = rng.random_range(0.0..TAU);
            for i in 0..spec.count {
                let dir = if spec.mixed_dirs && rng.random_bool(0.5) { -base_dir } else { base_dir };
                p.spawn((
                    Orbiter {
                        platform: index,
                        angle: offset + i as f32 * TAU / spec.count as f32,
                        speed: spec.speed * dir,
                        radius: ORBIT_R,
                        wobble: spec.wobble,
                        phase: rng.random_range(0.0..TAU),
                    },
                    Enemy { radius: ENEMY_R },
                    Transform::default(),
                    Visibility::default(),
                ))
                .with_children(|q| enemy_visual(q, draw, spec.kind, i as usize));
            }
            for i in 0..spec.second_ring {
                p.spawn((
                    Orbiter {
                        platform: index,
                        angle: rng.random_range(0.0..TAU) + i as f32 * PI,
                        speed: spec.speed * 0.7 * -base_dir,
                        radius: ORBIT_R2,
                        wobble: spec.wobble,
                        phase: rng.random_range(0.0..TAU),
                    },
                    Enemy { radius: ENEMY_R },
                    Transform::default(),
                    Visibility::default(),
                ))
                .with_children(|q| enemy_visual(q, draw, EnemyKind::Raven, i as usize + 3));
            }
        }

        // Bonus au centre du bouclier.
        if let Some(kind) = spec.bonus {
            p.spawn((
                Bonus { kind, platform: index },
                Bob { amp: 5.0, phase: rng.random_range(0.0..TAU), base_y: 30.0 },
                Transform::from_xyz(0.0, 30.0, 3.0),
                Visibility::default(),
            ))
            .with_children(|q| bonus_visual(q, draw, kind));
        }

        if let Some(name) = label {
            let (x, anchor) = if pos.x > 0.0 {
                (-ORBIT_R - 14.0, Anchor::CENTER_RIGHT)
            } else {
                (ORBIT_R + 14.0, Anchor::CENTER_LEFT)
            };
            p.spawn((
                Text2d::new(name),
                ui::font(20.0),
                TextColor(Color::srgba(1.0, 0.92, 0.7, 0.85)),
                anchor,
                Transform::from_xyz(x, 0.0, 0.6),
            ));
        }
    });

    track.platforms.push(TrackEntry { entity: root, pos, index });
    track.next = index + 1;
    track.last = pos;
}

// ---------- visuels ----------

fn enemy_visual(q: &mut ChildSpawnerCommands, d: &mut Draw, kind: EnemyKind, seed: usize) {
    let s = seed as f32;
    match kind {
        EnemyKind::Raven => {
            // Plumage bleu nuit cerné de clair pour rester lisible sur le ciel.
            let feather = Color::srgb(0.16, 0.20, 0.34);
            let edge = Color::srgb(0.75, 0.82, 1.0);
            let z = 2.0;
            q.spawn(d.circle(Color::srgba(0.8, 0.9, 1.0, 0.22), Vec2::ZERO, z - 0.3, 27.0));
            q.spawn(d.ellipse(edge, Vec2::ZERO, z - 0.1, 21.0, 14.0, 0.0));
            q.spawn(d.ellipse(feather, Vec2::ZERO, z, 18.5, 11.5, 0.0));
            q.spawn(d.circle(edge, Vec2::new(16.0, 6.0), z + 0.05, 11.0));
            q.spawn(d.circle(feather, Vec2::new(16.0, 6.0), z + 0.1, 9.0));
            q.spawn(d.tri(Color::srgb(1.0, 0.72, 0.15), Vec2::new(28.0, 5.0), z + 0.1, Vec2::new(9.0, 13.0), -PI / 2.0));
            q.spawn(d.circle(Color::WHITE, Vec2::new(18.0, 8.5), z + 0.2, 3.6));
            q.spawn(d.circle(Color::srgb(0.05, 0.05, 0.1), Vec2::new(19.2, 8.5), z + 0.3, 1.8));
            for x in [-4.0, 2.0] {
                q.spawn(d.rect(Color::srgb(1.0, 0.72, 0.15), Vec2::new(x, -13.0), z - 0.05, Vec2::new(2.5, 7.0), 0.0));
            }
            for side in [1.0, -1.0] {
                let base = if side > 0.0 { 0.0 } else { PI };
                q.spawn((
                    d.tri(edge, Vec2::new(-4.0, side * 8.0), z - 0.2, Vec2::new(22.0, 30.0), base),
                    Wobble { amp: 0.55, freq: 9.0, phase: s, base },
                ));
                q.spawn((
                    d.tri(feather, Vec2::new(-4.0, side * 8.0), z - 0.15, Vec2::new(17.0, 25.0), base),
                    Wobble { amp: 0.55, freq: 9.0, phase: s, base },
                ));
            }
        }
        EnemyKind::Draugr => {
            let skin = Color::srgb(0.30, 0.52, 0.30);
            let z = 2.0;
            q.spawn(d.circle(Color::srgba(0.5, 1.0, 0.5, 0.18), Vec2::ZERO, z - 0.2, 24.0));
            q.spawn(d.circle(skin, Vec2::ZERO, z, 16.0));
            q.spawn(d.rect(skin.darker(0.1), Vec2::new(0.0, -10.0), z + 0.1, Vec2::new(18.0, 8.0), 0.0));
            for k in 0..3 {
                q.spawn(d.rect(Color::srgb(0.95, 0.95, 0.85), Vec2::new(-5.0 + k as f32 * 5.0, -8.0), z + 0.2, Vec2::new(3.0, 4.0), 0.0));
            }
            for side in [-1.0, 1.0] {
                q.spawn(d.circle(Color::srgb(0.05, 0.1, 0.05), Vec2::new(side * 6.0, 3.0), z + 0.1, 4.5));
                q.spawn((
                    d.circle(Color::srgb(1.0, 0.2, 0.1), Vec2::new(side * 6.0, 3.0), z + 0.2, 2.2),
                    Wobble { amp: 0.0, freq: 6.0, phase: s, base: 0.0 },
                ));
            }
        }
        EnemyKind::Wolf => {
            let fur = Color::srgb(0.55, 0.57, 0.62);
            let z = 2.0;
            q.spawn(d.circle(fur, Vec2::ZERO, z, 15.0));
            q.spawn(d.ellipse(fur.lighter(0.08), Vec2::new(11.0, -3.0), z + 0.1, 9.0, 6.5, 0.0));
            q.spawn(d.circle(Color::srgb(0.1, 0.1, 0.1), Vec2::new(18.0, -2.0), z + 0.2, 3.0));
            q.spawn(d.tri(fur.darker(0.1), Vec2::new(-7.0, 12.0), z - 0.1, Vec2::new(9.0, 13.0), 0.2));
            q.spawn(d.tri(fur.darker(0.1), Vec2::new(3.0, 13.0), z - 0.1, Vec2::new(9.0, 13.0), -0.2));
            for x in [-3.0, 6.0] {
                q.spawn(d.circle(Color::srgb(1.0, 0.85, 0.2), Vec2::new(x, 4.0), z + 0.2, 2.8));
                q.spawn(d.circle(Color::BLACK, Vec2::new(x + 0.8, 4.0), z + 0.3, 1.3));
            }
            q.spawn(d.tri(Color::WHITE, Vec2::new(12.0, -9.0), z + 0.2, Vec2::new(3.0, 5.0), PI));
        }
        EnemyKind::Kraken => {
            let purple = Color::srgb(0.48, 0.18, 0.55);
            let z = 2.0;
            for (i, a) in [-0.9_f32, -0.3, 0.3, 0.9].iter().enumerate() {
                q.spawn((
                    d.rect(purple.darker(0.05), Vec2::from_angle(*a + PI) * 18.0, z - 0.1, Vec2::new(7.0, 26.0), *a + PI / 2.0),
                    Wobble { amp: 0.35, freq: 4.0 + i as f32, phase: s + i as f32, base: *a + PI / 2.0 },
                ));
            }
            q.spawn(d.circle(purple, Vec2::ZERO, z, 15.0));
            q.spawn(d.ellipse(purple.lighter(0.1), Vec2::new(0.0, 6.0), z + 0.1, 12.0, 8.0, 0.0));
            for x in [-5.0, 5.0] {
                q.spawn(d.circle(Color::srgb(1.0, 0.95, 0.5), Vec2::new(x, 0.0), z + 0.2, 3.5));
                q.spawn(d.ellipse(Color::BLACK, Vec2::new(x, 0.0), z + 0.3, 1.2, 2.8, 0.0));
            }
        }
        EnemyKind::Jotunn => {
            let ice = Color::srgb(0.62, 0.82, 0.95);
            let z = 2.0;
            q.spawn(d.circle(Color::srgba(0.8, 0.95, 1.0, 0.25), Vec2::ZERO, z - 0.2, 24.0));
            q.spawn(d.hex(ice, Vec2::ZERO, z, 18.0));
            q.spawn(d.hex(ice.lighter(0.1), Vec2::new(-2.0, 2.0), z + 0.05, 12.0));
            for (x, h) in [(-9.0, 10.0), (0.0, 15.0), (8.0, 11.0)] {
                q.spawn(d.tri(Color::srgb(0.9, 0.98, 1.0), Vec2::new(x, 16.0 + h * 0.3), z - 0.1, Vec2::new(6.0, h), 0.0));
            }
            for x in [-5.0, 5.0] {
                q.spawn(d.circle(Color::srgb(0.05, 0.15, 0.45), Vec2::new(x, 2.0), z + 0.2, 2.6));
            }
            q.spawn(d.rect(Color::srgb(0.05, 0.15, 0.45), Vec2::new(0.0, -7.0), z + 0.2, Vec2::new(10.0, 2.0), 0.0));
        }
        EnemyKind::SerpentHead => {
            let green = Color::srgb(0.18, 0.55, 0.30);
            let z = 2.5;
            q.spawn(d.ellipse(green, Vec2::ZERO, z, 21.0, 16.0, 0.0));
            q.spawn(d.tri(Color::srgb(0.95, 0.3, 0.2), Vec2::new(26.0, 0.0), z - 0.1, Vec2::new(5.0, 16.0), -PI / 2.0));
            for y in [-6.0, 6.0] {
                q.spawn(d.circle(Color::srgb(1.0, 0.85, 0.1), Vec2::new(8.0, y), z + 0.1, 3.6));
                q.spawn(d.ellipse(Color::BLACK, Vec2::new(8.5, y), z + 0.2, 1.2, 3.0, 0.0));
            }
            for y in [-9.0, 9.0] {
                q.spawn(d.tri(Color::WHITE, Vec2::new(17.0, y), z + 0.1, Vec2::new(3.0, 6.0), -PI / 2.0));
            }
        }
        EnemyKind::SerpentBody => {
            let shade = if seed % 2 == 0 { Color::srgb(0.18, 0.5, 0.28) } else { Color::srgb(0.25, 0.62, 0.34) };
            let z = 2.4 - seed as f32 * 0.02;
            q.spawn(d.circle(shade, Vec2::ZERO, z, 14.0));
            q.spawn(d.ring(shade.darker(0.12), Vec2::ZERO, z + 0.05, 14.0));
        }
    }
}

fn bonus_visual(q: &mut ChildSpawnerCommands, d: &mut Draw, kind: BonusKind) {
    q.spawn(d.circle(Color::srgba(1.0, 0.95, 0.6, 0.22), Vec2::ZERO, 0.0, 22.0));
    match kind {
        BonusKind::Hammer => {
            q.spawn(d.rect(Color::srgb(0.45, 0.28, 0.14), Vec2::new(0.0, -6.0), 0.1, Vec2::new(5.0, 22.0), 0.0));
            q.spawn(d.rect(Color::srgb(0.75, 0.78, 0.85), Vec2::new(0.0, 7.0), 0.2, Vec2::new(22.0, 12.0), 0.0));
            q.spawn(d.rect(Color::srgb(0.95, 0.85, 0.4), Vec2::new(0.0, 7.0), 0.3, Vec2::new(4.0, 8.0), 0.0));
        }
        BonusKind::Shield => {
            q.spawn(d.circle(Color::srgb(0.2, 0.5, 0.95), Vec2::ZERO, 0.1, 13.0));
            q.spawn(d.ring(Color::srgb(0.9, 0.95, 1.0), Vec2::ZERO, 0.2, 13.0));
            q.spawn(d.rect(Color::srgb(0.9, 0.95, 1.0), Vec2::ZERO, 0.3, Vec2::new(3.0, 18.0), 0.0));
            q.spawn(d.rect(Color::srgb(0.9, 0.95, 1.0), Vec2::new(0.0, 3.0), 0.3, Vec2::new(12.0, 3.0), 0.0));
        }
        BonusKind::Mead => {
            q.spawn(d.tri(Color::srgb(0.93, 0.88, 0.75), Vec2::new(0.0, -2.0), 0.1, Vec2::new(16.0, 26.0), PI + 0.35));
            q.spawn(d.ellipse(Color::srgb(0.95, 0.72, 0.2), Vec2::new(-4.0, 9.0), 0.2, 9.0, 4.0, 0.35));
            q.spawn(d.circle(Color::srgb(1.0, 0.95, 0.8), Vec2::new(-6.0, 12.0), 0.3, 3.0));
        }
    }
}

// ---------- animation ----------

fn orbit_enemies(time: Res<Time>, powers: Res<Powers>, mut q: Query<(&mut Orbiter, &mut Transform)>) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let scale = if powers.mead > 0.0 { 0.4 } else { 1.0 };
    for (mut o, mut tf) in &mut q {
        let speed = o.speed * (1.0 + o.wobble * (t * 1.7 + o.phase).sin());
        o.angle += speed * dt * scale;
        let dir = Vec2::from_angle(o.angle);
        tf.translation = (dir * o.radius).extend(2.0);
        // La créature regarde dans le sens de sa course.
        let facing = o.angle + if o.speed > 0.0 { PI / 2.0 } else { -PI / 2.0 };
        tf.rotation = Quat::from_rotation_z(facing);
        if o.speed < 0.0 {
            tf.scale = Vec3::new(1.0, -1.0, 1.0);
        }
    }
}

fn animate_wobble(time: Res<Time>, mut q: Query<(&Wobble, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (w, mut tf) in &mut q {
        tf.rotation = Quat::from_rotation_z(w.base + w.amp * (t * w.freq + w.phase).sin());
        if w.amp == 0.0 {
            // Pulsation d'échelle (yeux).
            let s = 1.0 + 0.4 * (t * w.freq + w.phase).sin();
            tf.scale = Vec3::new(2.2 * s, 2.2 * s, 1.0);
        }
    }
}

fn animate_bob(time: Res<Time>, mut q: Query<(&Bob, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (b, mut tf) in &mut q {
        tf.translation.y = b.base_y + b.amp * (t * 2.5 + b.phase).sin();
        tf.rotation = Quat::from_rotation_z(0.15 * (t * 1.8 + b.phase).sin());
    }
}
