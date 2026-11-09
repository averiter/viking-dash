//! Ambiance : ciel dégradé, lune, aurore boréale, étoiles, neige, fjord, drakkar,
//! particules, vignette et lueur du Ragnarök.

use crate::art::{vcol, Art, Baker};
use crate::world::{Draw, Palette, Shapes};
use crate::{ui, GameEntity, GameFont, GameState, PlayerCam, MAX_PLAYERS};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::color::Alpha;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::sprite_render::AlphaMode2d;
use rand::RngExt;
use std::f32::consts::PI;

/// Élément de ciel attaché à la caméra d'un joueur (chaque caméra a son propre ciel).
#[derive(Component)]
struct SkyOf(u8);

/// Secondes restantes de lueur rouge.
#[derive(Resource, Default)]
pub struct Ragnarok(pub f32);

pub const RAGNAROK_DURATION: f32 = 2.6;

impl Ragnarok {
    /// Intensité de la lueur (0 à 1) : monte en douceur, puis s'éteint lentement. Pas de clignotement.
    fn glow(&self) -> f32 {
        if self.0 <= 0.0 {
            return 0.0;
        }
        let rise = ((RAGNAROK_DURATION - self.0) / 0.4).clamp(0.0, 1.0);
        let fall = (self.0 / RAGNAROK_DURATION).clamp(0.0, 1.0);
        rise * rise * (3.0 - 2.0 * rise) * fall
    }
}

#[derive(Component)]
pub struct Particle {
    vel: Vec2,
    life: f32,
    max: f32,
    gravity: f32,
    spin: f32,
}

impl Particle {
    pub fn new(vel: Vec2, life: f32, gravity: f32) -> Self {
        Self { vel, life, max: life, gravity, spin: 0.0 }
    }
}

#[derive(Component)]
pub struct FloatText {
    vel: Vec2,
    life: f32,
    max: f32,
}

#[derive(Component)]
struct Star {
    phase: f32,
    size: f32,
}

#[derive(Component)]
struct Snow {
    speed: f32,
    drift: f32,
    phase: f32,
}

/// Décor plein écran qui suit la caméra de son joueur.
#[derive(Component)]
enum Backdrop {
    /// Dégradé du ciel, derrière tout le reste.
    Gradient,
    Aurora,
    Moon,
    /// Coins assombris, par-dessus le monde.
    Vignette,
}

/// Maillage du rideau d'aurore, redessiné à chaque frame (partagé par les caméras).
#[derive(Resource)]
struct AuroraMesh(Handle<Mesh>);

#[derive(Component)]
struct Flash;

#[derive(Resource)]
struct FlashMat(Handle<ColorMaterial>);

#[derive(Component)]
struct Wave {
    phase: f32,
    base: Vec2,
}

#[derive(Component)]
struct Ship;

/// Onde de choc : anneau qui s'élargit et s'efface.
#[derive(Component)]
struct Ripple {
    t: f32,
    dur: f32,
    r0: f32,
    r1: f32,
    alpha: f32,
}

/// Emplacement de décor lointain : défile plus lentement que la piste, puis est recyclé plus haut.
#[derive(Component)]
struct Scenery {
    near: bool,
    slot: usize,
    /// Hauteur dans l'espace de son plan (écran = `h - caméra × facteur`).
    h: f32,
    /// Nombre de recyclages : graine de son aspect.
    cycle: u32,
}

/// Hasard reproductible entre 0 et 1 : les deux caméras de l'écran partagé tirent le même
/// décor, puisqu'elles montrent le même ciel.
fn hash01(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65536.0
}

struct Plane {
    factor: f32,
    count: usize,
    spacing: f32,
    start: f32,
    z: f32,
}

/// Plan lointain : le Bifröst. Plan proche : les branches d'Yggdrasil. Un emplacement sur
/// lequel rien ne pousse reste caché. `count × spacing` dépasse la plus haute vue (portrait).
const FAR: Plane = Plane { factor: 0.25, count: 7, spacing: 340.0, start: 260.0, z: -7.5 };
const NEAR: Plane = Plane { factor: 0.5, count: 5, spacing: 480.0, start: 520.0, z: -5.5 };

#[derive(Resource)]
struct SceneryArt {
    branch: Handle<Mesh>,
    bifrost: Handle<Mesh>,
}

/// Étoile filante (une par caméra, réutilisée).
#[derive(Component)]
struct Meteor {
    wait: f32,
    life: f32,
    vel: Vec2,
    /// Position relative à la caméra.
    rel: Vec2,
}

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Ragnarok>()
            .add_systems(Startup, setup_sky)
            .add_systems(OnEnter(GameState::Menu), seed_scenery)
            .add_systems(Update, (ripples, scenery_scroll, meteors, embers))
            .add_systems(
                Update,
                (
                    particles,
                    float_texts,
                    sky_follow,
                    backdrop_follow,
                    aurora_wave,
                    sky_tint,
                    ragnarok_tick,
                    waves_and_ship,
                ),
            );
    }
}

/// Indices d'une grille de `cols` × `rows` cases dont les sommets sont rangés ligne par ligne.
fn grid_indices(cols: usize, rows: usize, offset: usize, out: &mut Vec<u16>) {
    let w = cols + 1;
    for j in 0..rows {
        for i in 0..cols {
            let a = (offset + j * w + i) as u16;
            let (b, c, d) = (a + 1, a + w as u16, a + w as u16 + 1);
            out.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }
}

/// Carré unité centré, découpé en grille, coloré sommet par sommet : un dégradé sans texture.
fn gradient_mesh(cols: usize, rows: usize, color: impl Fn(Vec2) -> Color) -> Mesh {
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for j in 0..=rows {
        for i in 0..=cols {
            let p = Vec2::new(i as f32 / cols as f32 - 0.5, j as f32 / rows as f32 - 0.5);
            positions.push([p.x, p.y, 0.0]);
            colors.push(vcol(color(p)));
        }
    }
    let mut indices = Vec::new();
    grid_indices(cols, rows, 0, &mut indices);
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U16(indices))
}

const AURORA_COLS: usize = 64;
const AURORA_RIBBONS: usize = 2;
const AURORA_HALF_W: f32 = 820.0;

/// Sommets du rideau d'aurore à l'instant `t` : deux rubans ondulants, chacun en quatre
/// rangées (bord inférieur éteint, liseré vif, corps, sommet éteint).
fn aurora_vertices(t: f32) -> (Vec<[f32; 3]>, Vec<[f32; 4]>) {
    let n = AURORA_RIBBONS * 4 * (AURORA_COLS + 1);
    let mut positions = vec![[0.0; 3]; n];
    let mut colors = vec![[0.0; 4]; n];
    for r in 0..AURORA_RIBBONS {
        let rf = r as f32;
        // Le ruban du fond (r = 0) est plus haut, plus pâle et plus lent.
        let strength = if r == 0 { 0.32 } else { 0.55 };
        let speed = if r == 0 { 0.7 } else { 1.0 };
        let ts = t * speed;
        for i in 0..=AURORA_COLS {
            let p = i as f32 * 0.21 + rf * 2.3;
            let x = -AURORA_HALF_W + i as f32 * (2.0 * AURORA_HALF_W / AURORA_COLS as f32) + 14.0 * (ts * 0.3 + p).sin();
            let base = 44.0 * (ts * 0.25 + p * 0.9).sin() + 24.0 * (ts * 0.4 + p * 2.3).sin() + 70.0 - rf * 95.0;
            let height = 190.0 + 70.0 * (ts * 0.5 + p * 1.4).sin() + 40.0 * (ts * 1.1 + p * 3.7).sin();
            // Rayons verticaux : l'intensité varie d'une colonne à l'autre et glisse lentement.
            let rays = 0.62 + 0.38 * (i as f32 * 1.9 + ts * 0.9 + rf * 4.0).sin() * (i as f32 * 0.6 - ts * 0.5).sin();
            let a = strength * rays;
            let hue = 0.5 + 0.5 * (p * 0.8 + ts * 0.12).sin();
            let low = Vec3::new(0.25, 1.0, 0.55).lerp(Vec3::new(0.25, 0.85, 1.0), hue);
            let high = Vec3::new(0.65, 0.42, 1.0);
            let mid = low.lerp(high, 0.55);
            let rows = [
                (base - 60.0, low, 0.0),
                (base, low, a),
                (base + height * 0.42, mid, a * 0.5),
                (base + height, high, 0.0),
            ];
            for (j, (y, c, alpha)) in rows.into_iter().enumerate() {
                let v = (r * 4 + j) * (AURORA_COLS + 1) + i;
                positions[v] = [x, y, 0.0];
                colors[v] = vcol(Color::srgba(c.x, c.y, c.z, alpha));
            }
        }
    }
    (positions, colors)
}

fn aurora_mesh() -> Mesh {
    let (positions, colors) = aurora_vertices(0.0);
    let mut indices = Vec::new();
    for r in 0..AURORA_RIBBONS {
        grid_indices(AURORA_COLS, 3, r * 4 * (AURORA_COLS + 1), &mut indices);
    }
    // Conservé côté jeu (usages par défaut) : ses sommets sont réécrits à chaque frame.
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U16(indices))
}

fn aurora_wave(time: Res<Time>, aurora: Res<AuroraMesh>, mut meshes: ResMut<Assets<Mesh>>) {
    if let Some(mut mesh) = meshes.get_mut(&aurora.0) {
        let (positions, colors) = aurora_vertices(time.elapsed_secs());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }
}

fn setup_sky(
    mut commands: Commands,
    shapes: Res<Shapes>,
    mut palette: ResMut<Palette>,
    mut mats: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    art: Res<Art>,
) {
    let mut rng = rand::rng();

    // Matériau blanc translucide : la couleur vient des sommets.
    let vertex_mat = art.mat.clone();
    // Ciel : sombre en haut, lueur froide vers l'horizon ; translucide, pour laisser
    // passer la teinte du fond qui change avec le niveau et le Ragnarök.
    let gradient = meshes.add(gradient_mesh(1, 8, |p| {
        let v = p.y + 0.5;
        let glow = (1.0 - v / 0.5).clamp(0.0, 1.0);
        let dark = ((v - 0.5) / 0.5).clamp(0.0, 1.0);
        if glow > 0.0 {
            Color::srgba(0.16, 0.42, 0.62, 0.36 * glow * glow)
        } else {
            Color::srgba(0.0, 0.0, 0.05, 0.5 * dark)
        }
    }));
    // Vignette : transparente au centre, sombre dans les coins.
    let vignette = meshes.add(gradient_mesh(12, 12, |p| {
        let d = (p * 2.0).length();
        let k = ((d - 0.62) / 0.8).clamp(0.0, 1.0);
        Color::srgba(0.0, 0.0, 0.04, 0.5 * k * k * (3.0 - 2.0 * k))
    }));
    let aurora = meshes.add(aurora_mesh());
    commands.insert_resource(AuroraMesh(aurora.clone()));
    let mut d = Draw {
        shapes: &shapes,
        palette: &mut palette,
        mats: &mut mats,
    };

    for id in 0..MAX_PLAYERS as u8 {
        let layer = RenderLayers::layer(id as usize + 1);

        // Étoiles.
        for _ in 0..140 {
            let size = rng.random_range(0.7..1.9);
            let pos = Vec2::new(rng.random_range(-720.0..720.0), rng.random_range(-400.0..1100.0));
            commands.spawn((
                d.circle(Color::srgba(1.0, 1.0, 0.95, 0.5), pos, -9.0, size),
                Star { phase: rng.random_range(0.0..6.3), size },
                SkyOf(id),
                layer.clone(),
            ));
        }

        // Dégradé du ciel et vignette : un quadrilatère chacun.
        commands.spawn((
            Mesh2d(gradient.clone()),
            MeshMaterial2d(vertex_mat.clone()),
            Transform::from_xyz(0.0, 0.0, -10.0),
            Backdrop::Gradient,
            SkyOf(id),
            NoFrustumCulling,
            layer.clone(),
        ));
        commands.spawn((
            Mesh2d(vignette.clone()),
            MeshMaterial2d(vertex_mat.clone()),
            Transform::from_xyz(0.0, 0.0, 35.0),
            Backdrop::Vignette,
            SkyOf(id),
            NoFrustumCulling,
            layer.clone(),
        ));

        // Aurore boréale : un seul rideau à bords fondus (au lieu de 48 bandes).
        commands.spawn((
            Mesh2d(aurora.clone()),
            MeshMaterial2d(vertex_mat.clone()),
            Transform::from_xyz(0.0, 300.0, -8.0),
            Backdrop::Aurora,
            SkyOf(id),
            NoFrustumCulling,
            layer.clone(),
        ));

        // Lune, derrière l'aurore. Les couches de rendu ne s'héritent pas : chaque morceau porte la sienne.
        let moon = Color::srgb(0.92, 0.94, 1.0);
        let crater = Color::srgb(0.80, 0.83, 0.94);
        commands
            .spawn((
                Backdrop::Moon,
                SkyOf(id),
                Transform::from_xyz(210.0, 300.0, -8.5),
                Visibility::default(),
                layer.clone(),
            ))
            .with_children(|p| {
                p.spawn((d.glow(Color::srgba(0.7, 0.82, 1.0, 0.5), Vec2::ZERO, -0.1, 130.0), layer.clone()));
                p.spawn((d.circle(moon, Vec2::ZERO, 0.0, 30.0), layer.clone()));
                for (x, y, r) in [(-9.0, 8.0, 7.0), (10.0, -4.0, 5.0), (-3.0, -13.0, 4.0), (13.0, 13.0, 3.0)] {
                    p.spawn((d.circle(crater, Vec2::new(x, y), 0.05, r), layer.clone()));
                }
            });

        // Neige.
        for _ in 0..70 {
            let pos = Vec2::new(rng.random_range(-720.0..720.0), rng.random_range(-380.0..380.0));
            commands.spawn((
                d.circle(Color::srgba(1.0, 1.0, 1.0, 0.4), pos, 30.0, rng.random_range(1.1..2.4)),
                Snow {
                    speed: rng.random_range(25.0..70.0),
                    drift: rng.random_range(10.0..35.0),
                    phase: rng.random_range(0.0..6.3),
                },
                SkyOf(id),
                layer.clone(),
            ));
        }
    }

    // Montagnes : deux plans cuits en un seul maillage, versant éclairé et versant à l'ombre,
    // neige taillée sur le sommet.
    let mut b = Baker::new();
    let snow = (Color::srgb(0.84, 0.89, 0.98), Color::srgb(0.58, 0.65, 0.84));
    let mut x = -800.0;
    while x < 800.0 {
        let w = rng.random_range(240.0..400.0);
        let h = rng.random_range(220.0..360.0);
        mountain(&mut b, 0.0, Vec2::new(x, -170.0 + h), w, h, Color::srgb(0.10, 0.14, 0.29), Color::srgb(0.065, 0.09, 0.21), Some(snow));
        x += w * 0.55;
    }
    let mut x = -750.0;
    while x < 800.0 {
        let w = rng.random_range(260.0..460.0);
        let h = rng.random_range(120.0..220.0);
        mountain(&mut b, 1.0, Vec2::new(x, -180.0 + h), w, h, Color::srgb(0.05, 0.075, 0.17), Color::srgb(0.03, 0.05, 0.12), None);
        x += w * 0.6;
    }
    commands.spawn(art.at(&meshes.add(b.build()), Vec2::ZERO, -6.5));

    // Fjord, reflets de l'aurore et de la lune sur l'eau.
    commands.spawn(d.rect(Color::srgb(0.05, 0.12, 0.26), Vec2::new(0.0, -520.0), -5.0, Vec2::new(4000.0, 700.0), 0.0));
    let reflection = meshes.add(gradient_mesh(1, 2, |p| {
        let k = p.y + 0.5;
        Color::srgba(0.25, 0.85, 0.65, 0.15 * k * k)
    }));
    let mut strip = art.at(&reflection, Vec2::new(0.0, -245.0), -4.8);
    strip.2.scale = Vec3::new(4000.0, 150.0, 1.0);
    commands.spawn(strip);
    let mut moonlight = d.glow(Color::srgba(0.8, 0.88, 1.0, 0.3), Vec2::new(210.0, -300.0), -4.7, 1.0);
    moonlight.1.scale = Vec3::new(34.0, 130.0, 1.0);
    commands.spawn(moonlight);
    for row in 0..7 {
        for k in 0..18 {
            let base = Vec2::new(-800.0 + k as f32 * 95.0 + row as f32 * 23.0, -192.0 - row as f32 * 38.0);
            commands.spawn((
                d.ellipse(Color::srgba(0.45, 0.70, 0.95, 0.35), base, -4.5, 28.0, 3.0, 0.0),
                Wave { phase: rng.random_range(0.0..6.3), base },
            ));
        }
    }

    // Drakkar : un seul maillage.
    commands.spawn((Ship, art.at(&meshes.add(ship_mesh()), Vec2::new(-250.0, -205.0), -4.0)));

    // Décor d'altitude en parallaxe (habillé par `seed_scenery` à chaque retour au menu)
    // et étoile filante : quelques entités recyclées par caméra.
    let scenery = SceneryArt {
        branch: meshes.add(branch_mesh()),
        bifrost: meshes.add(bifrost_mesh()),
    };
    let streak = meshes.add(gradient_mesh(6, 1, |p| {
        let k = p.x + 0.5;
        Color::srgba(1.0, 1.0, 0.95, k * k)
    }));
    for id in 0..MAX_PLAYERS as u8 {
        let layer = RenderLayers::layer(id as usize + 1);
        for (near, count) in [(false, FAR.count), (true, NEAR.count)] {
            for slot in 0..count {
                commands.spawn((
                    art.at(&scenery.branch, Vec2::ZERO, if near { NEAR.z } else { FAR.z }),
                    Scenery { near, slot, h: 0.0, cycle: 0 },
                    Visibility::Hidden,
                    SkyOf(id),
                    NoFrustumCulling,
                    layer.clone(),
                ));
            }
        }
        commands.spawn((
            art.at(&streak, Vec2::ZERO, -8.8),
            Meteor { wait: rng.random_range(3.0..9.0), life: 0.0, vel: Vec2::ZERO, rel: Vec2::ZERO },
            SkyOf(id),
            NoFrustumCulling,
            Visibility::Hidden,
            layer,
        ));
    }
    commands.insert_resource(scenery);

    // Voile de lumière plein écran pour le Ragnarök (un par caméra, même matériau).
    let flash = mats.add(ColorMaterial {
        color: Color::srgba(1.0, 0.1, 0.05, 0.0),
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    });
    for id in 0..MAX_PLAYERS as u8 {
        commands.spawn((
            Flash,
            SkyOf(id),
            RenderLayers::layer(id as usize + 1),
            Mesh2d(shapes.square.clone()),
            MeshMaterial2d(flash.clone()),
            Transform::from_xyz(0.0, 0.0, 40.0).with_scale(Vec3::new(4000.0, 4000.0, 1.0)),
        ));
    }
    commands.insert_resource(FlashMat(flash));
}

/// Montagne à deux versants (éclairé à gauche de l'arête, à l'ombre à droite), avec neige
/// optionnelle épousant le sommet.
fn mountain(b: &mut Baker, z: f32, apex: Vec2, w: f32, h: f32, lit: Color, shade: Color, snow: Option<(Color, Color)>) {
    const RIDGE: f32 = 0.24;
    // Point du flanc à la profondeur `d` sous le sommet ; `side` va de -1 (gauche) à 1 (droite).
    let at = |side: f32, d: f32| apex + Vec2::new(side * w / 2.0 * d / h, -d);
    for (s0, s1, color) in [(-1.0, RIDGE, lit), (RIDGE, 1.0, shade)] {
        b.poly(color, z, &[apex, at(s0, h), at(s1, h)]);
    }
    if let Some((snow_lit, snow_shade)) = snow {
        let d = h * 0.2;
        for (s0, s1, color) in [(-1.0, RIDGE, snow_lit), (RIDGE, 1.0, snow_shade)] {
            b.poly(color, z + 0.01, &[apex, at(s0, d), at(s1, d)]);
            // Langue de neige qui descend le long du versant.
            b.poly(color, z + 0.01, &[at(s0, d), at((s0 + s1) / 2.0, d * 1.5), at(s1, d)]);
        }
    }
}

fn ship_mesh() -> Mesh {
    let mut b = Baker::new();
    let hull = Color::srgb(0.36, 0.22, 0.12);
    let dark = Color::srgb(0.27, 0.16, 0.09);
    b.rect(hull, Vec2::ZERO, 0.0, Vec2::new(120.0, 18.0), 0.0);
    b.rect(dark, Vec2::new(0.0, -5.0), 0.05, Vec2::new(120.0, 3.0), 0.0);
    b.tri(hull, Vec2::new(64.0, 12.0), 0.0, Vec2::new(16.0, 42.0), -0.25);
    b.tri(hull, Vec2::new(-64.0, 12.0), 0.0, Vec2::new(16.0, 42.0), 0.25);
    b.circle(Color::srgb(0.85, 0.2, 0.15), Vec2::new(68.0, 34.0), 0.1, 6.0);
    b.rect(dark, Vec2::new(0.0, 40.0), -0.1, Vec2::new(4.0, 70.0), 0.0);
    b.rect(Color::srgb(0.92, 0.88, 0.78), Vec2::new(0.0, 48.0), -0.2, Vec2::new(64.0, 46.0), 0.0);
    for i in 0..3 {
        b.rect(Color::srgb(0.75, 0.18, 0.15), Vec2::new(-20.0 + i as f32 * 20.0, 48.0), -0.15, Vec2::new(8.0, 46.0), 0.0);
    }
    for i in 0..5 {
        let c = [Color::srgb(0.7, 0.2, 0.15), Color::srgb(0.2, 0.4, 0.65)][i % 2];
        let p = Vec2::new(-40.0 + i as f32 * 20.0, 8.0);
        b.circle(c, p, 0.2, 7.0);
        b.circle(Color::srgb(0.75, 0.75, 0.8), p, 0.3, 2.0);
    }
    b.build()
}

/// Branche d'Yggdrasil : silhouette sombre qui entre par le bord (pointe à l'origine, tronc vers +x).
fn branch_mesh() -> Mesh {
    let mut b = Baker::new();
    let bark = Color::srgb(0.10, 0.085, 0.15);
    let spine = [(0.0, 40.0, 4.0), (120.0, 22.0, 11.0), (260.0, -8.0, 21.0), (430.0, -22.0, 33.0), (760.0, -80.0, 56.0)];
    for w in spine.windows(2) {
        let ((x0, y0, t0), (x1, y1, t1)) = (w[0], w[1]);
        b.poly(bark, 0.0, &[Vec2::new(x0, y0 + t0), Vec2::new(x1, y1 + t1), Vec2::new(x1, y1 - t1), Vec2::new(x0, y0 - t0)]);
    }
    let twigs = [(Vec2::new(125.0, 24.0), Vec2::new(62.0, 112.0)), (Vec2::new(262.0, -10.0), Vec2::new(196.0, -112.0)), (Vec2::new(420.0, -10.0), Vec2::new(372.0, 96.0))];
    for (from, to) in twigs {
        let n = (to - from).normalize().perp();
        b.poly(bark, 0.0, &[from + n * 7.0, to + n * 2.0, to - n * 2.0, from - n * 7.0]);
    }
    // Feuillage en touffes au bout de la branche et des rameaux.
    let leaves = [Color::srgba(0.06, 0.19, 0.17, 0.95), Color::srgba(0.09, 0.26, 0.21, 0.92), Color::srgba(0.13, 0.33, 0.25, 0.9)];
    let tips = [Vec2::new(0.0, 40.0)].into_iter().chain(twigs.iter().map(|t| t.1));
    for (i, tip) in tips.enumerate() {
        for k in 0..6 {
            let a = k as f32 * 1.1 + i as f32 * 0.7;
            let off = Vec2::new(a.cos() * 30.0, (a * 1.7).sin() * 20.0);
            b.ellipse(leaves[k % 3], tip + off, 0.1 + k as f32 * 0.01, 30.0 - k as f32 * 2.0, 12.0, a * 0.9);
        }
    }
    b.build()
}

/// Bifröst : arche de bandes de couleur translucides, très haut dans le ciel.
fn bifrost_mesh() -> Mesh {
    let mut b = Baker::new();
    let center = Vec2::new(0.0, -1320.0);
    let (a0, a1) = (PI / 2.0 - 0.62, PI / 2.0 + 0.62);
    let bands = [(1.0, 0.35, 0.35), (1.0, 0.65, 0.3), (1.0, 0.92, 0.4), (0.4, 0.9, 0.5), (0.35, 0.65, 1.0), (0.65, 0.45, 1.0)];
    b.arc(Color::srgba(1.0, 0.35, 0.35, 0.07), center, 0.0, 1238.0, 1250.0, a0, a1);
    for (i, (r, g, bl)) in bands.into_iter().enumerate() {
        let r0 = 1250.0 + i as f32 * 20.0;
        b.arc(Color::srgba(r, g, bl, 0.2), center, 0.0, r0, r0 + 20.0, a0, a1);
    }
    b.arc(Color::srgba(0.65, 0.45, 1.0, 0.07), center, 0.0, 1370.0, 1382.0, a0, a1);
    b.build()
}

/// Décide ce qui pousse sur un emplacement, selon l'altitude à laquelle il croisera l'écran.
fn dress_scenery(s: &Scenery, tf: &mut Transform, mesh: &mut Mesh2d, vis: &mut Visibility, art: &SceneryArt) {
    let plane = if s.near { &NEAR } else { &FAR };
    // `VIKING_ALTITUDE=8000` décale le décor (tests : voir tout de suite les hauteurs).
    let offset: f32 = std::env::var("VIKING_ALTITUDE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let altitude = s.h / plane.factor + offset;
    let key = s.slot as u32 + if s.near { 100 } else { 0 };
    let unit = |n: u32| hash01(key, s.cycle * 16 + n);
    let flip = if unit(1) < 0.5 { -1.0 } else { 1.0 };
    *vis = Visibility::Inherited;
    if s.near && altitude > 2800.0 && unit(2) < 0.6 {
        // La branche entre par un bord : sa pointe reste hors du couloir central.
        let scale = 0.9 + 0.4 * unit(3);
        mesh.0 = art.branch.clone();
        tf.translation.x = flip * (250.0 + 90.0 * unit(4));
        tf.rotation = Quat::IDENTITY;
        tf.scale = Vec3::new(flip * scale, scale, 1.0);
    } else if !s.near && altitude > 7000.0 && s.slot % 3 == 0 {
        mesh.0 = art.bifrost.clone();
        tf.translation.x = 0.0;
        tf.rotation = Quat::from_rotation_z(flip * 0.12);
        tf.scale = Vec3::ONE;
    } else {
        *vis = Visibility::Hidden;
    }
}

/// Replace tout le décor d'altitude pour une nouvelle ascension.
fn seed_scenery(art: Res<SceneryArt>, mut q: Query<(&mut Scenery, &mut Transform, &mut Mesh2d, &mut Visibility)>) {
    // Une graine par partie : le décor change d'une ascension à l'autre.
    let run = rand::rng().random_range(0..4096u32);
    for (mut s, mut tf, mut mesh, mut vis) in &mut q {
        let plane = if s.near { &NEAR } else { &FAR };
        s.cycle = run * 64;
        let jitter = hash01(s.slot as u32 + if s.near { 100 } else { 0 }, s.cycle) * 120.0 - 60.0;
        s.h = plane.start + s.slot as f32 * plane.spacing + jitter;
        dress_scenery(&s, &mut tf, &mut mesh, &mut vis, &art);
    }
}

fn scenery_scroll(
    art: Res<SceneryArt>,
    cams: Query<(&PlayerCam, &Camera, &Transform, &Projection), Without<Scenery>>,
    mut q: Query<(&mut Scenery, &SkyOf, &mut Transform, &mut Mesh2d, &mut Visibility)>,
) {
    for (cam, camera, cam_tf, projection) in &cams {
        if !camera.is_active {
            continue;
        }
        let cy = cam_tf.translation.y;
        let half_h = match projection {
            Projection::Orthographic(o) => o.area.height().max(crate::VIEW_H) / 2.0,
            _ => crate::VIEW_H / 2.0,
        };
        for (mut s, sky, mut tf, mut mesh, mut vis) in &mut q {
            if sky.0 != cam.0 {
                continue;
            }
            let plane = if s.near { &NEAR } else { &FAR };
            let mut rel = s.h - cy * plane.factor;
            if rel < -half_h - 330.0 {
                // Sorti par le bas : il repart tout en haut de la file, sous un nouvel aspect.
                s.h += plane.count as f32 * plane.spacing;
                s.cycle += 1;
                rel = s.h - cy * plane.factor;
                dress_scenery(&s, &mut tf, &mut mesh, &mut vis, &art);
            }
            tf.translation.y = cy + rel;
        }
    }
}

/// De temps en temps, une étoile filante traverse le ciel.
fn meteors(
    time: Res<Time>,
    cams: Query<(&PlayerCam, &Camera, &Transform, &Projection), Without<Meteor>>,
    mut q: Query<(&mut Meteor, &SkyOf, &mut Transform, &mut Visibility)>,
) {
    const LIFE: f32 = 0.7;
    let dt = time.delta_secs();
    let mut rng = rand::rng();
    for (cam, camera, cam_tf, projection) in &cams {
        if !camera.is_active {
            continue;
        }
        let view_h = match projection {
            Projection::Orthographic(o) => o.area.height().max(crate::VIEW_H),
            _ => crate::VIEW_H,
        };
        for (mut m, sky, mut tf, mut vis) in &mut q {
            if sky.0 != cam.0 {
                continue;
            }
            if m.life <= 0.0 {
                m.wait -= dt;
                if m.wait > 0.0 {
                    continue;
                }
                m.wait = rng.random_range(7.0..16.0);
                m.life = LIFE;
                let dir = Vec2::from_angle(rng.random_range(-0.5..-0.2) + if rng.random_bool(0.5) { 0.0 } else { PI + 0.7 });
                m.vel = dir * rng.random_range(520.0..720.0);
                m.rel = Vec2::new(rng.random_range(-300.0..300.0), view_h * rng.random_range(0.12..0.42));
                tf.rotation = Quat::from_rotation_z(dir.to_angle());
                *vis = Visibility::Inherited;
            }
            m.life -= dt;
            let step = m.vel * dt;
            m.rel += step;
            // La traînée s'allonge puis se résorbe.
            let k = (m.life / LIFE).clamp(0.0, 1.0);
            tf.scale = Vec3::new(150.0 * (k * PI).sin().max(0.02), 2.4, 1.0);
            tf.translation.x = cam_tf.translation.x + m.rel.x;
            tf.translation.y = cam_tf.translation.y + m.rel.y;
            if m.life <= 0.0 {
                *vis = Visibility::Hidden;
            }
        }
    }
}

/// Braises qui montent du bas de l'écran pendant le Ragnarök.
fn embers(
    mut commands: Commands,
    time: Res<Time>,
    ragnarok: Res<Ragnarok>,
    mut budget: Local<f32>,
    cams: Query<(&Camera, &Transform, &Projection), With<PlayerCam>>,
) {
    if ragnarok.0 <= 0.0 {
        *budget = 0.0;
        return;
    }
    *budget += 26.0 * ragnarok.glow() * time.delta_secs();
    let mut rng = rand::rng();
    while *budget >= 1.0 {
        *budget -= 1.0;
        for (camera, tf, projection) in &cams {
            if !camera.is_active {
                continue;
            }
            let area = match projection {
                Projection::Orthographic(o) => o.area.size(),
                _ => Vec2::new(crate::VIEW_W_MIN, crate::VIEW_H),
            };
            let pos = Vec2::new(
                tf.translation.x + rng.random_range(-0.5..0.5) * area.x,
                tf.translation.y - area.y * rng.random_range(0.3..0.5),
            );
            let life = rng.random_range(1.2..2.2);
            let color = Color::srgba(1.0, rng.random_range(0.35..0.7), 0.15, 0.9);
            commands.spawn((
                Sprite::from_color(color, Vec2::splat(rng.random_range(2.5..5.0))),
                Transform::from_translation(pos.extend(9.0)),
                Particle {
                    vel: Vec2::new(rng.random_range(-30.0..30.0), rng.random_range(90.0..190.0)),
                    life,
                    max: life,
                    gravity: -25.0,
                    spin: rng.random_range(-3.0..3.0),
                },
                GameEntity,
            ));
        }
    }
}

/// Onde de choc centrée sur `pos`, de `r0` à `r1` en `dur` secondes.
pub fn ripple(commands: &mut Commands, shapes: &Shapes, pos: Vec2, color: Color, r0: f32, r1: f32, dur: f32) {
    commands.spawn((
        Sprite {
            image: shapes.ring_tex.clone(),
            color,
            custom_size: Some(Vec2::splat(2.0)),
            ..default()
        },
        Transform::from_translation(pos.extend(1.5)).with_scale(Vec3::new(r0, r0, 1.0)),
        Ripple { t: 0.0, dur, r0, r1, alpha: color.alpha() },
        GameEntity,
    ));
}

fn ripples(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Ripple, &mut Transform, &mut Sprite)>) {
    for (e, mut r, mut tf, mut sprite) in &mut q {
        r.t += time.delta_secs();
        let k = r.t / r.dur;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        // Part vite, ralentit en s'effaçant. La texture place le trait à 86 % du rayon.
        let radius = (r.r0 + (r.r1 - r.r0) * (1.0 - (1.0 - k) * (1.0 - k))) / 0.86;
        tf.scale = Vec3::new(radius, radius, 1.0);
        sprite.color.set_alpha(r.alpha * (1.0 - k));
    }
}

/// Jet de particules carrées.
pub fn burst(commands: &mut Commands, pos: Vec2, color: Color, n: usize, speed: f32, size: f32, gravity: f32, life: f32) {
    let mut rng = rand::rng();
    for _ in 0..n {
        let a = rng.random_range(0.0..std::f32::consts::TAU);
        let v = Vec2::from_angle(a) * speed * rng.random_range(0.3..1.0);
        let max = life * rng.random_range(0.6..1.2);
        commands.spawn((
            Sprite::from_color(color, Vec2::splat(size * rng.random_range(0.6..1.3))),
            Transform::from_translation(pos.extend(8.0)).with_rotation(Quat::from_rotation_z(a)),
            Particle { vel: v, life: max, max, gravity, spin: rng.random_range(-8.0..8.0) },
            GameEntity,
        ));
    }
}

/// Texte flottant qui monte et s'efface.
pub fn float_text(commands: &mut Commands, game_font: &GameFont, pos: Vec2, text: &str, color: Color, size: f32) {
    commands.spawn((
        Text2d::new(text),
        ui::world_font(game_font, size),
        TextColor(color),
        Transform::from_translation(pos.extend(20.0)),
        FloatText { vel: Vec2::new(0.0, 55.0), life: 1.4, max: 1.4 },
        GameEntity,
    ));
}

fn particles(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Particle, &mut Transform, &mut Sprite)>) {
    let dt = time.delta_secs();
    for (e, mut p, mut tf, mut sprite) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        p.vel.y -= p.gravity * dt;
        tf.translation += (p.vel * dt).extend(0.0);
        tf.rotate_z(p.spin * dt);
        let k = p.life / p.max;
        sprite.color.set_alpha(k);
        tf.scale = Vec3::splat(0.4 + 0.6 * k);
    }
}

fn float_texts(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut FloatText, &mut Transform, &mut TextColor)>) {
    let dt = time.delta_secs();
    for (e, mut f, mut tf, mut color) in &mut q {
        f.life -= dt;
        if f.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        tf.translation += (f.vel * dt).extend(0.0);
        let k = (f.life / f.max).min(1.0);
        color.0.set_alpha(k.powf(0.6));
        tf.scale = Vec3::splat(1.0 + 0.15 * (1.0 - k));
    }
}

fn sky_follow(
    time: Res<Time>,
    cams: Query<(&PlayerCam, &Transform, &Projection), (Without<Star>, Without<Snow>, Without<Flash>)>,
    mut stars: Query<(&Star, &SkyOf, &mut Transform), (Without<Snow>, Without<Flash>)>,
    mut snow: Query<(&Snow, &SkyOf, &mut Transform), (Without<Star>, Without<Flash>)>,
    mut flash: Query<(&SkyOf, &mut Transform), (With<Flash>, Without<Star>, Without<Snow>)>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let mut rng = rand::rng();

    for (cam, cam_tf, projection) in &cams {
        let id = cam.0;
        let cy = cam_tf.translation.y;
        // Hauteur visible : l'aurore reste dans le tiers supérieur, même en portrait.
        let visible_h = match projection {
            Projection::Orthographic(o) => o.area.height().max(crate::VIEW_H),
            _ => crate::VIEW_H,
        };

        for (s, sky, mut tf) in &mut stars {
            if sky.0 != id {
                continue;
            }
            if tf.translation.y < cy - visible_h * 0.6 {
                tf.translation.y += visible_h * 0.6 + 1100.0;
                tf.translation.x = rng.random_range(-720.0..720.0);
            } else if tf.translation.y > cy + 1100.0 {
                tf.translation.y -= visible_h * 0.6 + 1100.0;
            }
            let tw = 0.7 + 0.3 * (t * 2.0 + s.phase).sin();
            tf.scale = Vec3::splat(s.size * tw);
        }
        for (s, sky, mut tf) in &mut snow {
            if sky.0 != id {
                continue;
            }
            tf.translation.y -= s.speed * dt;
            tf.translation.x += s.drift * (t * 0.8 + s.phase).sin() * dt;
            if tf.translation.y < cy - visible_h * 0.55 {
                tf.translation.y = cy + visible_h * 0.55;
                tf.translation.x = rng.random_range(-720.0..720.0);
            } else if tf.translation.y > cy + visible_h * 0.56 {
                tf.translation.y = cy - visible_h * 0.54;
            }
        }
        for (sky, mut tf) in &mut flash {
            if sky.0 != id {
                continue;
            }
            tf.translation.x = cam_tf.translation.x;
            tf.translation.y = cy;
        }
    }
}

/// Cale le dégradé, la vignette, l'aurore et la lune sur la vue de chaque caméra.
fn backdrop_follow(
    cams: Query<(&PlayerCam, &Transform, &Projection), Without<Backdrop>>,
    mut backdrops: Query<(&Backdrop, &SkyOf, &mut Transform)>,
) {
    for (cam, cam_tf, projection) in &cams {
        let (cx, cy) = (cam_tf.translation.x, cam_tf.translation.y);
        let view = match projection {
            Projection::Orthographic(o) => o.area.size().max(Vec2::new(crate::VIEW_W_MIN, crate::VIEW_H)),
            _ => Vec2::new(crate::VIEW_W_MIN, crate::VIEW_H),
        };
        for (backdrop, sky, mut tf) in &mut backdrops {
            if sky.0 != cam.0 {
                continue;
            }
            match backdrop {
                // Un peu plus grands que la vue : les secousses de caméra n'en montrent pas le bord.
                Backdrop::Gradient | Backdrop::Vignette => {
                    tf.translation.x = cx;
                    tf.translation.y = cy;
                    tf.scale = (view * 1.08).extend(1.0);
                }
                Backdrop::Aurora => tf.translation.y = cy + view.y * 0.20,
                Backdrop::Moon => tf.translation.y = cy + view.y * 0.33,
            }
        }
    }
}

fn sky_tint(players: Query<&crate::player::Player>, ragnarok: Res<Ragnarok>, mut clear: ResMut<ClearColor>) {
    let level = players.iter().map(|p| p.level).max().unwrap_or(0);
    let k = (level as f32 / 45.0).clamp(0.0, 1.0);
    let a = Vec3::new(0.03, 0.06, 0.14);
    let b = Vec3::new(0.10, 0.03, 0.14);
    let mut c = a.lerp(b, k);
    c = c.lerp(Vec3::new(0.22, 0.03, 0.02), ragnarok.glow() * 0.32);
    clear.0 = Color::srgb(c.x, c.y, c.z);
}

fn ragnarok_tick(time: Res<Time>, mut r: ResMut<Ragnarok>, flash: Res<FlashMat>, mut mats: ResMut<Assets<ColorMaterial>>) {
    if r.0 <= 0.0 {
        return;
    }
    r.0 -= time.delta_secs();
    // Voile rouge léger et continu : plus de flashs répétés.
    let alpha = 0.07 * r.glow();
    if let Some(mut m) = mats.get_mut(&flash.0) {
        m.color.set_alpha(alpha);
    }
}

fn waves_and_ship(
    time: Res<Time>,
    mut waves: Query<(&Wave, &mut Transform), Without<Ship>>,
    mut ship: Query<&mut Transform, With<Ship>>,
) {
    let t = time.elapsed_secs();
    for (w, mut tf) in &mut waves {
        tf.translation.x = w.base.x + 14.0 * (t * 0.9 + w.phase).sin();
        tf.translation.y = w.base.y + 2.5 * (t * 1.4 + w.phase).sin();
    }
    for mut tf in &mut ship {
        tf.translation.y = -205.0 + 4.0 * (t * 1.2).sin();
        tf.rotation = Quat::from_rotation_z(0.04 * (t * 0.9 + PI / 3.0).sin());
    }
}
