//! Ambiance : aurore boréale, étoiles, neige, fjord, drakkar, particules,
//! secousses de caméra et éclairs du Ragnarök.

use crate::world::{Draw, Palette, Shapes};
use crate::{ui, GameEntity, MainCamera, Score};
use bevy::color::{Alpha, Luminance};
use bevy::prelude::*;
use bevy::sprite_render::AlphaMode2d;
use rand::Rng;
use std::f32::consts::PI;

/// Intensité de secousse de caméra (décroît toute seule).
#[derive(Resource, Default)]
pub struct Shake(pub f32);

/// Secondes restantes d'éclairs rouges.
#[derive(Resource, Default)]
pub struct Ragnarok(pub f32);

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

#[derive(Component)]
struct AuroraBand(usize);

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
        app.init_resource::<Shake>()
            .init_resource::<Ragnarok>()
            .add_systems(Startup, setup_sky)
            .add_systems(
                Update,
                (
                    particles,
                    float_texts,
                    sky_follow,
                    sky_tint,
                    ragnarok_tick,
                    waves_and_ship,
                ),
            );
    }
}

fn setup_sky(
    mut commands: Commands,
    shapes: Res<Shapes>,
    mut palette: ResMut<Palette>,
    mut mats: ResMut<Assets<ColorMaterial>>,
) {
    let mut rng = rand::rng();
    let mut d = Draw {
        shapes: &shapes,
        palette: &mut palette,
        mats: &mut mats,
    };

    // Étoiles.
    for _ in 0..140 {
        let size = rng.random_range(0.8..2.4);
        let pos = Vec2::new(rng.random_range(-560.0..560.0), rng.random_range(-400.0..1100.0));
        commands.spawn((
            d.circle(Color::srgba(1.0, 1.0, 0.95, 0.8), pos, -9.0, size),
            Star { phase: rng.random_range(0.0..6.3), size },
        ));
    }

    // Aurore boréale : rideau de bandes translucides.
    let tints = [
        Color::srgba(0.30, 1.00, 0.60, 0.15),
        Color::srgba(0.30, 0.85, 1.00, 0.13),
        Color::srgba(0.70, 0.45, 1.00, 0.11),
    ];
    for i in 0..42 {
        let x = -640.0 + i as f32 * 31.0;
        commands.spawn((
            d.rect(tints[i % 3], Vec2::new(x, 300.0), -8.0, Vec2::new(30.0, 150.0), 0.0),
            AuroraBand(i),
        ));
    }

    // Neige.
    for _ in 0..90 {
        let pos = Vec2::new(rng.random_range(-560.0..560.0), rng.random_range(-380.0..380.0));
        commands.spawn((
            d.circle(Color::srgba(1.0, 1.0, 1.0, 0.75), pos, 30.0, rng.random_range(1.3..3.0)),
            Snow {
                speed: rng.random_range(25.0..70.0),
                drift: rng.random_range(10.0..35.0),
                phase: rng.random_range(0.0..6.3),
            },
        ));
    }

    // Montagnes (deux plans).
    let far = Color::srgb(0.07, 0.10, 0.22);
    let near = Color::srgb(0.04, 0.06, 0.14);
    let mut x = -700.0;
    while x < 700.0 {
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
    let mut x = -650.0;
    while x < 700.0 {
        let w = rng.random_range(260.0..460.0);
        let h = rng.random_range(120.0..220.0);
        commands.spawn(d.tri(near, Vec2::new(x, -180.0 + h / 2.0), -6.0, Vec2::new(w, h), 0.0));
        x += w * 0.6;
    }

    // Fjord.
    commands.spawn(d.rect(Color::srgb(0.05, 0.12, 0.26), Vec2::new(0.0, -520.0), -5.0, Vec2::new(4000.0, 700.0), 0.0));
    for row in 0..7 {
        for k in 0..16 {
            let base = Vec2::new(-700.0 + k as f32 * 95.0 + row as f32 * 23.0, -192.0 - row as f32 * 38.0);
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

    // Voile de lumière plein écran pour le Ragnarök.
    let flash = mats.add(ColorMaterial {
        color: Color::srgba(1.0, 0.1, 0.05, 0.0),
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    });
    commands.spawn((
        Flash,
        Mesh2d(shapes.square.clone()),
        MeshMaterial2d(flash.clone()),
        Transform::from_xyz(0.0, 0.0, 40.0).with_scale(Vec3::new(4000.0, 4000.0, 1.0)),
    ));
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
pub fn float_text(commands: &mut Commands, pos: Vec2, text: &str, color: Color, size: f32) {
    commands.spawn((
        Text2d::new(text),
        ui::font(size),
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
    cam: Single<&Transform, (With<MainCamera>, Without<Star>, Without<Snow>, Without<AuroraBand>, Without<Flash>)>,
    mut stars: Query<(&Star, &mut Transform), (Without<Snow>, Without<AuroraBand>, Without<Flash>)>,
    mut snow: Query<(&Snow, &mut Transform), (Without<Star>, Without<AuroraBand>, Without<Flash>)>,
    mut aurora: Query<(&AuroraBand, &mut Transform), (Without<Star>, Without<Snow>, Without<Flash>)>,
    mut flash: Query<&mut Transform, (With<Flash>, Without<Star>, Without<Snow>, Without<AuroraBand>)>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let cy = cam.translation.y;
    let mut rng = rand::rng();

    for (s, mut tf) in &mut stars {
        if tf.translation.y < cy - 420.0 {
            tf.translation.y += 1500.0;
            tf.translation.x = rng.random_range(-560.0..560.0);
        } else if tf.translation.y > cy + 1100.0 {
            tf.translation.y -= 1500.0;
        }
        let tw = 0.7 + 0.3 * (t * 2.0 + s.phase).sin();
        tf.scale = Vec3::splat(s.size * tw);
    }
    for (s, mut tf) in &mut snow {
        tf.translation.y -= s.speed * dt;
        tf.translation.x += s.drift * (t * 0.8 + s.phase).sin() * dt;
        if tf.translation.y < cy - 390.0 {
            tf.translation.y = cy + 390.0;
            tf.translation.x = rng.random_range(-560.0..560.0);
        } else if tf.translation.y > cy + 400.0 {
            tf.translation.y = cy - 380.0;
        }
    }
    for (b, mut tf) in &mut aurora {
        let i = b.0 as f32;
        let h = 150.0 + 80.0 * (t * 0.6 + i * 0.35).sin() + 40.0 * (t * 1.3 + i * 0.9).sin();
        tf.scale = Vec3::new(30.0, h, 1.0);
        tf.translation.y = cy + 230.0 + 30.0 * (t * 0.25 + i * 0.3).sin();
    }
    for mut tf in &mut flash {
        tf.translation.x = cam.translation.x;
        tf.translation.y = cy;
    }
}

fn sky_tint(score: Res<Score>, ragnarok: Res<Ragnarok>, mut clear: ResMut<ClearColor>) {
    let k = (score.level as f32 / 45.0).clamp(0.0, 1.0);
    let a = Vec3::new(0.03, 0.06, 0.14);
    let b = Vec3::new(0.10, 0.03, 0.14);
    let mut c = a.lerp(b, k);
    if ragnarok.0 > 0.0 {
        c = c.lerp(Vec3::new(0.25, 0.03, 0.02), (ragnarok.0 / 2.6).min(1.0) * 0.6);
    }
    clear.0 = Color::srgb(c.x, c.y, c.z);
}

fn ragnarok_tick(time: Res<Time>, mut r: ResMut<Ragnarok>, flash: Res<FlashMat>, mut mats: ResMut<Assets<ColorMaterial>>) {
    if r.0 <= 0.0 {
        return;
    }
    r.0 -= time.delta_secs();
    let alpha = if r.0 > 0.0 {
        let k = r.0 / 2.6;
        0.4 * k * (r.0 * 9.0).sin().abs()
    } else {
        0.0
    };
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
