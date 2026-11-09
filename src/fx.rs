//! Ambiance : ciel dégradé, lune, aurore boréale, étoiles, neige, fjord, drakkar,
//! particules, vignette et lueur du Ragnarök.

use crate::world::{Draw, Palette, Shapes};
use crate::{ui, GameEntity, GameFont, PlayerCam, MAX_PLAYERS};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::color::{Alpha, Luminance};
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

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Ragnarok>()
            .add_systems(Startup, setup_sky)
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

/// Couleur d'un sommet (les couleurs de sommets sont en RVB linéaire).
fn vcol(c: Color) -> [f32; 4] {
    c.to_linear().to_f32_array()
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
) {
    let mut rng = rand::rng();

    // Matériau blanc translucide : la couleur vient des sommets.
    let vertex_mat = mats.add(ColorMaterial {
        color: Color::WHITE,
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    });
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
            let size = rng.random_range(0.8..2.4);
            let pos = Vec2::new(rng.random_range(-720.0..720.0), rng.random_range(-400.0..1100.0));
            commands.spawn((
                d.circle(Color::srgba(1.0, 1.0, 0.95, 0.8), pos, -9.0, size),
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
        for _ in 0..90 {
            let pos = Vec2::new(rng.random_range(-720.0..720.0), rng.random_range(-380.0..380.0));
            commands.spawn((
                d.circle(Color::srgba(1.0, 1.0, 1.0, 0.75), pos, 30.0, rng.random_range(1.3..3.0)),
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

    // Montagnes (deux plans).
    let far = Color::srgb(0.07, 0.10, 0.22);
    let near = Color::srgb(0.04, 0.06, 0.14);
    let mut x = -800.0;
    while x < 800.0 {
        let w = rng.random_range(240.0..400.0);
        let h = rng.random_range(220.0..360.0);
        commands.spawn(d.tri(far, Vec2::new(x, -170.0 + h / 2.0), -7.0, Vec2::new(w, h), 0.0));
        commands.spawn(d.tri(
            Color::srgba(1.0, 1.0, 1.0, 0.5),
            Vec2::new(x, -170.0 + h - 14.0),
            -6.9,
            Vec2::new(w * 0.16, 28.0),
            0.0,
        ));
        x += w * 0.55;
    }
    let mut x = -750.0;
    while x < 800.0 {
        let w = rng.random_range(260.0..460.0);
        let h = rng.random_range(120.0..220.0);
        commands.spawn(d.tri(near, Vec2::new(x, -180.0 + h / 2.0), -6.0, Vec2::new(w, h), 0.0));
        x += w * 0.6;
    }

    // Fjord.
    commands.spawn(d.rect(Color::srgb(0.05, 0.12, 0.26), Vec2::new(0.0, -520.0), -5.0, Vec2::new(4000.0, 700.0), 0.0));
    for row in 0..7 {
        for k in 0..18 {
            let base = Vec2::new(-800.0 + k as f32 * 95.0 + row as f32 * 23.0, -192.0 - row as f32 * 38.0);
            commands.spawn((
                d.ellipse(Color::srgba(0.45, 0.70, 0.95, 0.35), base, -4.5, 28.0, 3.0, 0.0),
                Wave { phase: rng.random_range(0.0..6.3), base },
            ));
        }
    }

    // Drakkar.
    let hull = Color::srgb(0.36, 0.22, 0.12);
    commands
        .spawn((Ship, Transform::from_xyz(-250.0, -205.0, -4.0), Visibility::default()))
        .with_children(|p| {
            p.spawn(d.rect(hull, Vec2::ZERO, 0.0, Vec2::new(120.0, 18.0), 0.0));
            p.spawn(d.tri(hull, Vec2::new(64.0, 12.0), 0.0, Vec2::new(16.0, 42.0), -0.25));
            p.spawn(d.tri(hull, Vec2::new(-64.0, 12.0), 0.0, Vec2::new(16.0, 42.0), 0.25));
            p.spawn(d.circle(Color::srgb(0.85, 0.2, 0.15), Vec2::new(68.0, 34.0), 0.1, 6.0));
            p.spawn(d.rect(hull.darker(0.05), Vec2::new(0.0, 40.0), -0.1, Vec2::new(4.0, 70.0), 0.0));
            p.spawn(d.rect(Color::srgb(0.92, 0.88, 0.78), Vec2::new(0.0, 48.0), -0.2, Vec2::new(64.0, 46.0), 0.0));
            for i in 0..3 {
                p.spawn(d.rect(Color::srgb(0.75, 0.18, 0.15), Vec2::new(-20.0 + i as f32 * 20.0, 48.0), -0.15, Vec2::new(8.0, 46.0), 0.0));
            }
            for i in 0..5 {
                let c = [Color::srgb(0.7, 0.2, 0.15), Color::srgb(0.2, 0.4, 0.65)][i % 2];
                p.spawn(d.circle(c, Vec2::new(-40.0 + i as f32 * 20.0, 8.0), 0.2, 7.0));
                p.spawn(d.circle(Color::srgb(0.75, 0.75, 0.8), Vec2::new(-40.0 + i as f32 * 20.0, 8.0), 0.3, 2.0));
            }
        });

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
