//! Le monde : boucliers-plateformes, créatures en orbite, bonus, génération
//! procédurale et difficulté progressive.

use crate::art::Art;
use crate::player::{self, Player};
use crate::{GameEntity, GameMode, GameState, PlayerCam, Score};
use bevy::asset::RenderAssetUsages;
use bevy::color::Luminance;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::RngExt;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

pub const PLATFORM_R: f32 = 54.0;
pub const ORBIT_R: f32 = 100.0;
pub const ORBIT_R2: f32 = 138.0;
pub const ENEMY_R: f32 = 17.0;
pub const PLAYER_R: f32 = 15.0;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Track>()
            .init_resource::<Palette>()
            .init_resource::<Seen>()
            .add_systems(PreStartup, setup_shapes)
            .add_systems(OnEnter(GameState::Menu), reset_world)
            .add_systems(OnEnter(GameState::Playing), spawn_missing_players)
            .add_systems(
                Update,
                (extend_track, orbit_enemies, animate_wobble, animate_bob, animate_bounce),
            );
    }
}

// ---------- primitives de dessin ----------

#[derive(Resource)]
pub struct Shapes {
    pub circle: Handle<Mesh>,
    pub square: Handle<Mesh>,
    pub ring: Handle<Mesh>,
    /// Halo doux : disque blanc dont l'opacité s'éteint vers le bord (texture créée en mémoire).
    pub glow: Handle<Image>,
    /// Anneau doux (ondes de choc, repère du prochain bouclier).
    pub ring_tex: Handle<Image>,
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

/// Texture blanche carrée dont l'opacité suit `alpha(distance au centre)` (1 = bord).
fn radial_image(n: usize, alpha: impl Fn(f32) -> f32) -> Image {
    let mut data = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let p = (Vec2::new(x as f32, y as f32) + 0.5) / (n as f32 / 2.0) - 1.0;
            data.extend_from_slice(&[255, 255, 255, (alpha(p.length()).clamp(0.0, 1.0) * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d { width: n as u32, height: n as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn setup_shapes(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(Shapes {
        // Halo : opacité en cloche du centre vers le bord.
        glow: images.add(radial_image(64, |d| (1.0 - d).max(0.0).powi(2))),
        // Anneau : trait doux à 86 % du rayon.
        ring_tex: images.add(radial_image(128, |d| 1.0 - ((d - 0.86) / 0.07).powi(2))),
        circle: meshes.add(Circle::new(1.0)),
        square: meshes.add(Rectangle::new(1.0, 1.0)),
        ring: meshes.add(Annulus::new(0.88, 1.0)),
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
    pub fn rect(&mut self, color: Color, pos: Vec2, z: f32, size: Vec2, rot: f32) -> ShapeBundle {
        let m = self.shapes.square.clone();
        self.bundle(&m, color, pos, z, size, rot)
    }
    pub fn ring(&mut self, color: Color, pos: Vec2, z: f32, r: f32) -> ShapeBundle {
        let m = self.shapes.ring.clone();
        self.bundle(&m, color, pos, z, Vec2::splat(r), 0.0)
    }
    /// Halo lumineux de rayon `r` (un sprite : tous les halos partent dans le même lot).
    pub fn glow(&self, color: Color, pos: Vec2, z: f32, r: f32) -> (Sprite, Transform) {
        (
            Sprite {
                image: self.shapes.glow.clone(),
                color,
                custom_size: Some(Vec2::splat(2.0)),
                ..default()
            },
            Transform::from_translation(pos.extend(z)).with_scale(Vec3::new(r, r, 1.0)),
        )
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

/// Dessin du bouclier d'une plateforme (s'écrase brièvement quand on y atterrit).
#[derive(Component)]
pub struct Bounce(pub f32);

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
    /// Entité du dessin du bouclier.
    pub visual: Entity,
    pub pos: Vec2,
    pub index: u32,
    /// Nom de la créature si elle apparaît ici pour la première fois (annoncé par le HUD).
    pub announce: Option<&'static str>,
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
    pub fn visual(&self, index: u32) -> Option<Entity> {
        self.platforms.iter().find(|p| p.index == index).map(|p| p.visual)
    }
    pub fn announce(&self, index: u32) -> Option<&'static str> {
        self.platforms.iter().find(|p| p.index == index).and_then(|p| p.announce)
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

fn difficulty(n: u32, rng: &mut impl RngExt) -> Spec {
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
    shapes: Res<Shapes>,
    art: Res<Art>,
    mut palette: ResMut<Palette>,
    mut mats: ResMut<Assets<ColorMaterial>>,
    mut cams: Query<&mut Transform, With<PlayerCam>>,
) {
    for e in &olds {
        commands.entity(e).despawn();
    }
    *track = Track::default();
    seen.0 = 0;
    score.new_record = false;
    for mut cam in &mut cams {
        cam.translation = Vec3::new(0.0, 120.0, 0.0);
    }
    let mut draw = Draw {
        shapes: &shapes,
        palette: &mut palette,
        mats: &mut mats,
    };
    // `VIKING_START=n` permet de démarrer directement au niveau n (tests).
    let start = std::env::var("VIKING_START").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    spawn_platform(&mut commands, &mut draw, &art, &mut track, &mut seen, start, Vec2::ZERO);
    // Le joueur 1 est visible dès le menu ; le joueur 2 arrive au lancement d'une partie à deux.
    player::spawn_player(&mut commands, &mut draw, &art, Vec2::ZERO, start, 0);
}

/// Au lancement d'une partie à deux, ajoute le second viking et écarte les deux sur le bouclier.
fn spawn_missing_players(
    mut commands: Commands,
    mode: Res<GameMode>,
    shapes: Res<Shapes>,
    art: Res<Art>,
    mut palette: ResMut<Palette>,
    mut mats: ResMut<Assets<ColorMaterial>>,
    mut players: Query<(&Player, &mut Transform)>,
) {
    let mut draw = Draw {
        shapes: &shapes,
        palette: &mut palette,
        mats: &mut mats,
    };
    let mut existing = 0;
    let mut start = 0;
    for (p, mut tf) in &mut players {
        existing += 1;
        start = p.on;
        let offset = player::stand_offset(p.id, mode.players);
        tf.translation.x = offset.x;
    }
    for id in existing..mode.players {
        let pos = player::stand_offset(id, mode.players);
        player::spawn_player(&mut commands, &mut draw, &art, pos, start, id);
    }
}

fn extend_track(
    mut commands: Commands,
    mut track: ResMut<Track>,
    mut seen: ResMut<Seen>,
    shapes: Res<Shapes>,
    art: Res<Art>,
    mut palette: ResMut<Palette>,
    mut mats: ResMut<Assets<ColorMaterial>>,
    cams: Query<(&Transform, &Projection, &Camera), With<PlayerCam>>,
) {
    // Zone à couvrir : du bas de la caméra la plus basse au haut de la plus haute
    // (la hauteur visible est bien plus grande en portrait sur téléphone).
    let mut top = f32::MIN;
    let mut bottom = f32::MAX;
    for (tf, projection, camera) in &cams {
        if !camera.is_active {
            continue;
        }
        let half_h = match projection {
            Projection::Orthographic(o) => o.area.height() / 2.0,
            _ => crate::VIEW_H / 2.0,
        }
        .max(crate::VIEW_H / 2.0);
        top = top.max(tf.translation.y + half_h);
        bottom = bottom.min(tf.translation.y - half_h);
    }
    if top == f32::MIN {
        return;
    }
    let mut draw = Draw {
        shapes: &shapes,
        palette: &mut palette,
        mats: &mut mats,
    };
    let mut rng = rand::rng();
    while track.next > 0 && track.last.y < top + 700.0 {
        let n = track.next;
        let gap = 215.0 + (n.min(30) as f32) * 1.5;
        let lo = (track.last.x - 220.0).max(-230.0);
        let hi = (track.last.x + 220.0).min(230.0);
        let x = rng.random_range(lo..hi);
        let pos = Vec2::new(x, track.last.y + gap);
        spawn_platform(&mut commands, &mut draw, &art, &mut track, &mut seen, n, pos);
    }
    // Nettoyage des plateformes passées.
    let mut i = 0;
    while i < track.platforms.len() {
        if track.platforms[i].pos.y < bottom - 400.0 {
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
    art: &Art,
    track: &mut Track,
    seen: &mut Seen,
    index: u32,
    pos: Vec2,
) {
    let mut rng = rand::rng();
    let spec = difficulty(index, &mut rng);

    let root = commands
        .spawn((
            Platform { index },
            GameEntity,
            Transform::from_translation(pos.extend(0.0)),
            Visibility::default(),
        ))
        .id();

    // Bouclier : un seul maillage (planches, motif peint, cerclage, rivets).
    let visual = commands.spawn(art.at(art.shield(index), Vec2::ZERO, 1.0)).id();
    commands.entity(root).add_child(visual);

    let mut label: Option<&'static str> = None;
    let kind_bit = 1 << (if spec.serpent { 5 } else { spec.kind as u32 });
    if spec.count > 0 || spec.serpent {
        if seen.0 & kind_bit == 0 {
            seen.0 |= kind_bit;
            label = Some(kind_name(if spec.serpent { EnemyKind::SerpentHead } else { spec.kind }));
        }
    }

    commands.entity(root).with_children(|p| {
        // Chemins d'orbite en pointillés.
        p.spawn(art.scaled(&art.orbit, Vec2::ZERO, 0.5, ORBIT_R));
        if spec.second_ring > 0 {
            p.spawn(art.scaled(&art.orbit, Vec2::ZERO, 0.5, ORBIT_R2));
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
                .with_children(|q| enemy_visual(q, draw, art, kind, seg));
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
                .with_children(|q| enemy_visual(q, draw, art, spec.kind, i as usize));
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
                .with_children(|q| enemy_visual(q, draw, art, EnemyKind::Raven, i as usize + 3));
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
            .with_children(|q| bonus_visual(q, draw, art, kind));
        }
    });

    track.platforms.push(TrackEntry { entity: root, visual, pos, index, announce: label });
    track.next = index + 1;
    track.last = pos;
}

// ---------- visuels ----------

/// Une créature : corps cuit en un maillage, plus les seuls morceaux qui bougent ou brillent.
pub fn enemy_visual(q: &mut ChildSpawnerCommands, d: &mut Draw, art: &Art, kind: EnemyKind, seed: usize) {
    let s = seed as f32;
    let z = 2.0;
    let o = Vec2::ZERO;
    match kind {
        EnemyKind::Raven => {
            q.spawn(d.glow(Color::srgba(0.7, 0.85, 1.0, 0.6), o, z - 0.3, 46.0));
            for side in [1.0, -1.0] {
                let base = if side > 0.0 { 0.0 } else { PI };
                let mut wing = art.at(&art.raven_wing, Vec2::new(-4.0, side * 8.0), z - 0.2);
                wing.2.rotation = Quat::from_rotation_z(base);
                q.spawn((wing, Wobble { amp: 0.55, freq: 9.0, phase: s, base }));
            }
            q.spawn(art.at(&art.raven_body, o, z));
        }
        EnemyKind::Draugr => {
            q.spawn(d.glow(Color::srgba(0.45, 1.0, 0.45, 0.5), o, z - 0.2, 42.0));
            q.spawn(art.at(&art.draugr, o, z));
            for side in [-1.0, 1.0] {
                q.spawn((
                    d.circle(Color::srgb(1.0, 0.2, 0.1), Vec2::new(side * 6.0, 3.0), z + 0.2, 2.2),
                    Wobble { amp: 0.0, freq: 3.5, phase: s, base: 0.0 },
                ));
            }
        }
        EnemyKind::Wolf => {
            q.spawn(art.at(&art.wolf, o, z));
        }
        EnemyKind::Kraken => {
            let purple = Color::srgb(0.48, 0.18, 0.55);
            q.spawn(d.glow(Color::srgba(0.85, 0.5, 1.0, 0.4), o, z - 0.2, 40.0));
            for (i, a) in [-0.9_f32, -0.3, 0.3, 0.9].iter().enumerate() {
                q.spawn((
                    d.rect(purple.darker(0.05), Vec2::from_angle(*a + PI) * 18.0, z - 0.1, Vec2::new(7.0, 26.0), *a + PI / 2.0),
                    Wobble { amp: 0.35, freq: 4.0 + i as f32, phase: s + i as f32, base: *a + PI / 2.0 },
                ));
            }
            q.spawn(art.at(&art.kraken, o, z));
        }
        EnemyKind::Jotunn => {
            q.spawn(d.glow(Color::srgba(0.75, 0.93, 1.0, 0.6), o, z - 0.2, 44.0));
            q.spawn(art.at(&art.jotunn, o, z));
        }
        EnemyKind::SerpentHead => {
            q.spawn(d.glow(Color::srgba(0.4, 1.0, 0.5, 0.4), o, 2.3, 44.0));
            q.spawn(art.at(&art.serpent_head, o, 2.5));
        }
        EnemyKind::SerpentBody => {
            q.spawn(art.at(&art.serpent_body[seed % 2], o, 2.4 - s * 0.02));
        }
    }
}

pub fn bonus_visual(q: &mut ChildSpawnerCommands, d: &mut Draw, art: &Art, kind: BonusKind) {
    q.spawn(d.glow(Color::srgba(1.0, 0.92, 0.55, 0.8), Vec2::ZERO, 0.0, 42.0));
    q.spawn(art.at(art.bonus(kind), Vec2::ZERO, 0.1));
}

// ---------- animation ----------

fn orbit_enemies(time: Res<Time>, players: Query<&Player>, mut q: Query<(&mut Orbiter, &mut Transform)>) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    // L'hydromel d'un joueur ralentit les créatures pour tout le monde.
    let mead = players.iter().any(|p| p.alive() && p.powers.mead > 0.0);
    let scale = if mead { 0.4 } else { 1.0 };
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
            let s = 1.0 + 0.2 * (t * w.freq + w.phase).sin();
            tf.scale = Vec3::new(2.2 * s, 2.2 * s, 1.0);
        }
    }
}

/// Le bouclier s'enfonce et revient quand un viking s'y pose.
fn animate_bounce(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Bounce, &mut Transform)>) {
    for (e, mut bounce, mut tf) in &mut q {
        bounce.0 += time.delta_secs();
        let k = bounce.0 / 0.3;
        if k >= 1.0 {
            tf.scale = Vec3::ONE;
            commands.entity(e).remove::<Bounce>();
        } else {
            let s = 1.0 - 0.1 * (k * PI).sin() * (1.0 - k);
            tf.scale = Vec3::new(s, s, 1.0);
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
