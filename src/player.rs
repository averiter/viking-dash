//! Le viking : saut, collisions, pouvoirs, mort et caméra.

use crate::audio::{Sfx, SfxBank};
use crate::fx::{self, Ragnarok, Shake};
use crate::world::{Bonus, BonusKind, Draw, Enemy, Orbiter, Track, PLAYER_R};
use crate::{action_pressed, save_best, AutoJump, GameEntity, GameFont, GameState, MainCamera, Powers, Score};
use bevy::color::Luminance;
use bevy::prelude::*;
use rand::Rng;
use std::f32::consts::{PI, TAU};

const JUMP_SPEED: f32 = 560.0;
const HAMMER_SPEED: f32 = 1050.0;
const COMBO_WINDOW: f32 = 0.7;

#[derive(Component)]
pub struct Player {
    pub on: u32,
    pub jumping: bool,
    pub t: f32,
    pub dur: f32,
    pub from: Vec2,
    pub to: Vec2,
    pub landed_at: f32,
    pub vel: Vec2,
}

#[derive(Component)]
struct ShieldAura;

#[derive(Component)]
struct HammerGlow;

#[derive(Resource, Default)]
struct DeathTimer(f32);

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DeathTimer>()
            .add_systems(
                Update,
                (jump_input, advance_jump, check_collisions, tick_powers)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(Update, (camera_follow, aura_visibility))
            .add_systems(OnEnter(GameState::Dying), |mut t: ResMut<DeathTimer>| t.0 = 0.0)
            .add_systems(Update, dying_anim.run_if(in_state(GameState::Dying)));
    }
}

pub fn spawn_player(commands: &mut Commands, d: &mut Draw, pos: Vec2, on: u32) {
    let skin = Color::srgb(0.95, 0.78, 0.62);
    let beard = Color::srgb(0.85, 0.42, 0.12);
    let tunic = Color::srgb(0.45, 0.30, 0.16);
    let iron = Color::srgb(0.55, 0.58, 0.64);
    let z = 5.0;
    commands
        .spawn((
            Player {
                on,
                jumping: false,
                t: 0.0,
                dur: 1.0,
                from: pos,
                to: pos,
                landed_at: -10.0,
                vel: Vec2::ZERO,
            },
            GameEntity,
            Transform::from_translation(pos.extend(z)),
            Visibility::default(),
        ))
        .with_children(|p| {
            // Auras de pouvoirs.
            p.spawn((
                d.ring(Color::srgba(0.4, 0.7, 1.0, 0.85), Vec2::ZERO, -0.5, 30.0),
                ShieldAura,
                Visibility::Hidden,
            ));
            p.spawn((
                d.circle(Color::srgba(1.0, 0.85, 0.3, 0.3), Vec2::ZERO, -0.6, 34.0),
                HammerGlow,
                Visibility::Hidden,
            ));
            // Cape.
            p.spawn(d.tri(Color::srgb(0.6, 0.12, 0.12), Vec2::new(-4.0, -6.0), -0.2, Vec2::new(30.0, 34.0), PI + 0.3));
            // Jambes et bottes.
            for x in [-6.0, 6.0] {
                p.spawn(d.rect(Color::srgb(0.25, 0.18, 0.12), Vec2::new(x, -19.0), 0.0, Vec2::new(8.0, 10.0), 0.0));
                p.spawn(d.rect(Color::srgb(0.2, 0.12, 0.08), Vec2::new(x, -25.0), 0.1, Vec2::new(10.0, 5.0), 0.0));
            }
            // Corps, ceinture, fourrure.
            p.spawn(d.rect(tunic, Vec2::new(0.0, -6.0), 0.1, Vec2::new(26.0, 24.0), 0.0));
            p.spawn(d.rect(Color::srgb(0.2, 0.14, 0.08), Vec2::new(0.0, -11.0), 0.2, Vec2::new(26.0, 5.0), 0.0));
            p.spawn(d.circle(Color::srgb(0.85, 0.75, 0.55), Vec2::new(0.0, -11.0), 0.3, 3.0));
            p.spawn(d.ellipse(Color::srgb(0.65, 0.5, 0.35), Vec2::new(0.0, 4.0), 0.2, 16.0, 5.0, 0.0));
            // Tête, barbe, yeux.
            p.spawn(d.circle(skin, Vec2::new(0.0, 13.0), 0.3, 11.0));
            p.spawn(d.circle(beard, Vec2::new(0.0, 6.0), 0.4, 9.5));
            p.spawn(d.rect(beard, Vec2::new(0.0, 2.0), 0.4, Vec2::new(14.0, 8.0), 0.0));
            p.spawn(d.ellipse(beard.darker(0.08), Vec2::new(0.0, 10.0), 0.5, 7.0, 2.2, 0.0));
            for x in [-4.0, 4.0] {
                p.spawn(d.circle(Color::WHITE, Vec2::new(x, 15.0), 0.5, 2.6));
                p.spawn(d.circle(Color::srgb(0.1, 0.2, 0.4), Vec2::new(x + 0.6, 15.0), 0.6, 1.3));
            }
            // Casque et cornes.
            p.spawn(d.ellipse(iron, Vec2::new(0.0, 20.0), 0.6, 12.5, 9.0, 0.0));
            p.spawn(d.rect(iron.darker(0.1), Vec2::new(0.0, 17.0), 0.7, Vec2::new(25.0, 4.0), 0.0));
            p.spawn(d.rect(iron.lighter(0.15), Vec2::new(0.0, 24.0), 0.7, Vec2::new(3.0, 8.0), 0.0));
            p.spawn(d.tri(Color::srgb(0.96, 0.93, 0.85), Vec2::new(-14.0, 25.0), 0.5, Vec2::new(8.0, 16.0), 0.6));
            p.spawn(d.tri(Color::srgb(0.96, 0.93, 0.85), Vec2::new(14.0, 25.0), 0.5, Vec2::new(8.0, 16.0), -0.6));
            // Hache.
            p.spawn(d.rect(Color::srgb(0.4, 0.26, 0.14), Vec2::new(19.0, -2.0), -0.1, Vec2::new(4.0, 34.0), -0.25));
            p.spawn(d.tri(iron.lighter(0.1), Vec2::new(24.0, 14.0), 0.0, Vec2::new(16.0, 14.0), -PI / 2.0 - 0.25));
        });
}

fn jump_input(
    mut commands: Commands,
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    track: Res<Track>,
    mut score: ResMut<Score>,
    mut powers: ResMut<Powers>,
    sfx: Res<SfxBank>,
    mut auto_jump: ResMut<AutoJump>,
    mut q: Query<(&mut Player, &Transform)>,
) {
    let auto = std::mem::replace(&mut auto_jump.0, false);
    if !auto && !action_pressed(&mouse, &keys, &touches) {
        return;
    }
    let Ok((mut player, tf)) = q.single_mut() else { return };
    if player.jumping {
        return;
    }
    let Some(to) = track.pos(player.on + 1) else { return };
    let from = tf.translation.truncate();
    let speed = if powers.hammer_jumps > 0 {
        powers.hammer_jumps -= 1;
        HAMMER_SPEED
    } else {
        JUMP_SPEED
    };
    player.from = from;
    player.to = to;
    player.dur = from.distance(to) / speed;
    player.t = 0.0;
    player.jumping = true;
    if time.elapsed_secs() - player.landed_at < COMBO_WINDOW {
        score.combo += 1;
    } else {
        score.combo = 0;
    }
    sfx.play(&mut commands, Sfx::Jump, 0.5);
}

fn advance_jump(
    mut commands: Commands,
    time: Res<Time>,
    mut score: ResMut<Score>,
    mut powers: ResMut<Powers>,
    mut shake: ResMut<Shake>,
    mut ragnarok: ResMut<Ragnarok>,
    sfx: Res<SfxBank>,
    game_font: Res<GameFont>,
    mut q: Query<(&mut Player, &mut Transform)>,
    bonuses: Query<(Entity, &Bonus)>,
) {
    let Ok((mut player, mut tf)) = q.single_mut() else { return };
    if !player.jumping {
        tf.scale = Vec3::ONE;
        return;
    }
    let dt = time.delta_secs();
    player.t += dt;
    let k = (player.t / player.dur).min(1.0);
    let pos = player.from.lerp(player.to, k);
    tf.translation = pos.extend(5.0);
    tf.rotation = Quat::from_rotation_z(k * TAU);
    let s = (k * PI).sin();
    tf.scale = Vec3::new(1.0 - 0.15 * s, 1.0 + 0.2 * s, 1.0);

    // Traînée.
    let mut rng = rand::rng();
    let trail = if powers.hammer_jumps > 0 || player.dur < 0.3 {
        Color::srgb(1.0, 0.85, 0.3)
    } else {
        Color::srgba(0.7, 0.9, 1.0, 0.8)
    };
    commands.spawn((
        Sprite::from_color(trail, Vec2::splat(rng.random_range(3.0..6.0))),
        Transform::from_translation(pos.extend(4.0)),
        fx_trail_particle(&mut rng),
        GameEntity,
    ));

    if k < 1.0 {
        return;
    }

    // Atterrissage.
    player.jumping = false;
    player.on += 1;
    player.landed_at = time.elapsed_secs();
    tf.rotation = Quat::IDENTITY;
    score.level = player.on;
    if score.level > score.best {
        score.best = score.level;
        score.new_record = true;
    }
    sfx.play(&mut commands, Sfx::Land, 0.6);
    fx::burst(&mut commands, pos - Vec2::new(0.0, 20.0), Color::srgb(0.95, 0.97, 1.0), 8, 90.0, 4.0, 200.0, 0.5);
    shake.0 = shake.0.max(2.0);

    if score.combo >= 2 {
        score.gold += score.combo;
        fx::float_text(
            &mut commands,
            &game_font,
            pos + Vec2::new(0.0, 50.0),
            &format!("BERSERK x{} !", score.combo),
            Color::srgb(1.0, 0.8, 0.2),
            26.0 + score.combo as f32 * 2.0,
        );
        sfx.play(&mut commands, Sfx::Drum, 0.6);
    }

    // Bonus posé sur ce bouclier ?
    for (e, bonus) in &bonuses {
        if bonus.platform != player.on {
            continue;
        }
        commands.entity(e).despawn();
        sfx.play(&mut commands, Sfx::Pickup, 0.6);
        score.gold += 5;
        let (text, color) = match bonus.kind {
            BonusKind::Hammer => {
                powers.hammer_jumps = 5;
                ("MJÖLNIR : 5 sauts foudroyants", Color::srgb(1.0, 0.9, 0.4))
            }
            BonusKind::Shield => {
                powers.shield = true;
                ("BOUCLIER RUNIQUE", Color::srgb(0.5, 0.8, 1.0))
            }
            BonusKind::Mead => {
                powers.mead = 7.0;
                ("HYDROMEL : le temps ralentit", Color::srgb(1.0, 0.75, 0.3))
            }
        };
        fx::float_text(&mut commands, &game_font, pos + Vec2::new(0.0, 80.0), text, color, 24.0);
        fx::burst(&mut commands, pos, color, 16, 160.0, 5.0, 100.0, 0.7);
    }

    // Tous les dix boucliers : Ragnarök.
    if player.on % 10 == 0 {
        ragnarok.0 = 2.6;
        shake.0 = 12.0;
        sfx.play(&mut commands, Sfx::Thunder, 0.8);
        fx::float_text(&mut commands, &game_font, pos + Vec2::new(0.0, 110.0), "RAGNARÖK !", Color::srgb(1.0, 0.25, 0.15), 44.0);
    }
}

fn fx_trail_particle(rng: &mut impl Rng) -> crate::fx::Particle {
    crate::fx::Particle::new(
        Vec2::new(rng.random_range(-40.0..40.0), rng.random_range(-40.0..40.0)),
        0.35,
        0.0,
    )
}

fn check_collisions(
    mut commands: Commands,
    track: Res<Track>,
    mut score: ResMut<Score>,
    mut powers: ResMut<Powers>,
    mut shake: ResMut<Shake>,
    sfx: Res<SfxBank>,
    mut next: ResMut<NextState<GameState>>,
    game_font: Res<GameFont>,
    mut q: Query<(&mut Player, &Transform)>,
    enemies: Query<(Entity, &Orbiter, &Enemy)>,
) {
    let Ok((mut player, tf)) = q.single_mut() else { return };
    if !player.jumping {
        return;
    }
    let pos = tf.translation.truncate();
    for (e, o, enemy) in &enemies {
        if o.platform != player.on && o.platform != player.on + 1 {
            continue;
        }
        let Some(center) = track.pos(o.platform) else { continue };
        let epos = center + Vec2::from_angle(o.angle) * o.radius;
        if pos.distance(epos) >= PLAYER_R + enemy.radius {
            continue;
        }
        if powers.shield {
            powers.shield = false;
            commands.entity(e).despawn();
            sfx.play(&mut commands, Sfx::Clang, 0.7);
            fx::burst(&mut commands, epos, Color::srgb(0.6, 0.85, 1.0), 20, 240.0, 6.0, 150.0, 0.7);
            fx::float_text(&mut commands, &game_font, pos + Vec2::new(0.0, 60.0), "Le bouclier éclate !", Color::srgb(0.6, 0.85, 1.0), 22.0);
            shake.0 = 8.0;
            return;
        }
        // Mort.
        player.jumping = false;
        player.vel = (pos - epos).normalize_or(Vec2::Y) * 260.0 + Vec2::new(0.0, 220.0);
        sfx.play(&mut commands, Sfx::Death, 0.8);
        // Étincelles dorées et lueur blanche : les Valkyries arrivent.
        fx::burst(&mut commands, pos, Color::srgb(1.0, 0.85, 0.3), 18, 240.0, 6.0, 120.0, 0.9);
        fx::burst(&mut commands, pos, Color::srgb(0.95, 0.97, 1.0), 14, 160.0, 4.0, 60.0, 1.1);
        shake.0 = 14.0;
        if score.new_record {
            save_best(score.best);
        }
        score.combo = 0;
        next.set(GameState::Dying);
        return;
    }
}

fn tick_powers(time: Res<Time>, mut powers: ResMut<Powers>) {
    if powers.mead > 0.0 {
        powers.mead = (powers.mead - time.delta_secs()).max(0.0);
    }
}

fn dying_anim(
    time: Res<Time>,
    mut timer: ResMut<DeathTimer>,
    mut next: ResMut<NextState<GameState>>,
    mut q: Query<(&mut Player, &mut Transform)>,
) {
    let dt = time.delta_secs();
    timer.0 += dt;
    if let Ok((mut player, mut tf)) = q.single_mut() {
        player.vel.y -= 900.0 * dt;
        tf.translation += (player.vel * dt).extend(0.0);
        tf.rotate_z(-7.0 * dt);
    }
    if timer.0 > 1.6 {
        next.set(GameState::GameOver);
    }
}

fn camera_follow(
    time: Res<Time>,
    mut shake: ResMut<Shake>,
    state: Res<State<GameState>>,
    player: Query<&Transform, (With<Player>, Without<MainCamera>)>,
    mut cam: Single<(&mut Transform, &Projection), With<MainCamera>>,
) {
    let dt = time.delta_secs();
    let Ok(ptf) = player.single() else { return };
    let (cam, projection) = &mut *cam;
    // Le viking reste dans le tiers inférieur de l'écran, quelle que soit sa hauteur.
    let visible_h = match projection {
        Projection::Orthographic(o) => o.area.height().max(crate::VIEW_H),
        _ => crate::VIEW_H,
    };
    let target_y = match state.get() {
        GameState::Playing => ptf.translation.y + visible_h * 0.21,
        GameState::Dying | GameState::GameOver => cam.translation.y,
        _ => 120.0,
    };
    let k = 1.0 - (-5.0 * dt).exp();
    let y = cam.translation.y + (target_y - cam.translation.y) * k;
    let mut rng = rand::rng();
    let off = if shake.0 > 0.05 {
        Vec2::new(rng.random_range(-1.0..1.0), rng.random_range(-1.0..1.0)) * shake.0
    } else {
        Vec2::ZERO
    };
    shake.0 *= (-7.0 * dt).exp();
    cam.translation.x = off.x;
    cam.translation.y = y + off.y;
}

fn aura_visibility(
    powers: Res<Powers>,
    time: Res<Time>,
    mut shield: Query<(&mut Visibility, &mut Transform), (With<ShieldAura>, Without<HammerGlow>)>,
    mut hammer: Query<(&mut Visibility, &mut Transform), (With<HammerGlow>, Without<ShieldAura>)>,
) {
    let t = time.elapsed_secs();
    for (mut v, mut tf) in &mut shield {
        *v = if powers.shield { Visibility::Inherited } else { Visibility::Hidden };
        tf.scale = Vec3::splat(30.0 + 2.0 * (t * 6.0).sin());
    }
    for (mut v, mut tf) in &mut hammer {
        *v = if powers.hammer_jumps > 0 { Visibility::Inherited } else { Visibility::Hidden };
        tf.scale = Vec3::splat(34.0 + 4.0 * (t * 9.0).sin());
    }
}
