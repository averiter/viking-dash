//! Les vikings : saut, collisions, pouvoirs, mort et caméras.

use crate::audio::{Sfx, SfxBank};
use crate::fx::{self, Ragnarok};
use crate::input::Inputs;
use crate::world::{Bonus, BonusKind, Draw, Enemy, Orbiter, Track, PLAYER_R};
use crate::{save_best, AutoJump, CamShake, GameEntity, GameFont, GameMode, GameState, PlayerCam, Powers, Score};
use bevy::color::Luminance;
use bevy::prelude::*;
use rand::Rng;
use std::f32::consts::{PI, TAU};

const JUMP_SPEED: f32 = 560.0;
const HAMMER_SPEED: f32 = 1050.0;
const COMBO_WINDOW: f32 = 0.7;
const DEATH_DURATION: f32 = 1.6;

#[derive(Component)]
pub struct Player {
    pub id: u8,
    pub on: u32,
    pub jumping: bool,
    pub t: f32,
    pub dur: f32,
    pub from: Vec2,
    pub to: Vec2,
    pub landed_at: f32,
    pub vel: Vec2,
    pub level: u32,
    pub gold: u32,
    pub combo: u32,
    pub powers: Powers,
    /// Temps écoulé depuis la mort (chute), `None` si vivant ou déjà retiré.
    pub dying: Option<f32>,
    pub dead: bool,
}

impl Player {
    pub fn alive(&self) -> bool {
        self.dying.is_none() && !self.dead
    }
}

#[derive(Component)]
struct ShieldAura;

#[derive(Component)]
struct HammerGlow;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                jump_input,
                advance_jump,
                check_collisions,
                tick_powers,
                dying_anim,
                check_game_over,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(Update, (camera_follow, aura_visibility));
    }
}

/// Décalage horizontal des vikings quand ils partagent un bouclier (à deux).
pub fn stand_offset(id: u8, players: u8) -> Vec2 {
    if players == 2 {
        Vec2::new((id as f32 - 0.5) * 36.0, 0.0)
    } else {
        Vec2::ZERO
    }
}

pub fn spawn_player(commands: &mut Commands, d: &mut Draw, pos: Vec2, on: u32, id: u8) {
    let skin = Color::srgb(0.95, 0.78, 0.62);
    // Joueur 1 : cape rouge et barbe rousse. Joueur 2 : cape bleue et barbe brune.
    let (cape, tunic, beard) = if id == 0 {
        (Color::srgb(0.6, 0.12, 0.12), Color::srgb(0.45, 0.30, 0.16), Color::srgb(0.85, 0.42, 0.12))
    } else {
        (Color::srgb(0.12, 0.3, 0.7), Color::srgb(0.30, 0.32, 0.40), Color::srgb(0.35, 0.20, 0.10))
    };
    let iron = Color::srgb(0.55, 0.58, 0.64);
    let z = 5.0 + id as f32 * 0.5;
    commands
        .spawn((
            Player {
                id,
                on,
                jumping: false,
                t: 0.0,
                dur: 1.0,
                from: pos,
                to: pos,
                landed_at: -10.0,
                vel: Vec2::ZERO,
                level: on,
                gold: 0,
                combo: 0,
                powers: Powers::default(),
                dying: None,
                dead: false,
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
            p.spawn(d.tri(cape, Vec2::new(-4.0, -6.0), -0.2, Vec2::new(30.0, 34.0), PI + 0.3));
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

/// Secoue la caméra d'un joueur (ou toutes avec `None`).
fn shake(cams: &mut Query<(&PlayerCam, &mut CamShake)>, id: Option<u8>, amount: f32) {
    for (cam, mut s) in cams.iter_mut() {
        if id.is_none_or(|i| i == cam.0) {
            s.0 = s.0.max(amount);
        }
    }
}

fn jump_input(
    mut commands: Commands,
    time: Res<Time>,
    inputs: Res<Inputs>,
    mut auto_jump: ResMut<AutoJump>,
    track: Res<Track>,
    mode: Res<GameMode>,
    sfx: Res<SfxBank>,
    mut q: Query<(&mut Player, &Transform)>,
) {
    let auto = std::mem::take(&mut auto_jump.0);
    for (mut player, tf) in &mut q {
        let id = player.id as usize;
        if !(inputs.jump[id] || auto[id]) || player.jumping || !player.alive() {
            continue;
        }
        let Some(to) = track.pos(player.on + 1) else { continue };
        let from = tf.translation.truncate();
        let to = to + stand_offset(player.id, mode.players);
        let speed = if player.powers.hammer_jumps > 0 {
            player.powers.hammer_jumps -= 1;
            HAMMER_SPEED
        } else {
            JUMP_SPEED
        };
        player.from = from;
        player.to = to;
        player.dur = (from.distance(to) / speed).max(0.05);
        player.t = 0.0;
        player.jumping = true;
        if time.elapsed_secs() - player.landed_at < COMBO_WINDOW {
            player.combo += 1;
        } else {
            player.combo = 0;
        }
        sfx.play(&mut commands, Sfx::Jump, 0.5);
    }
}

fn advance_jump(
    mut commands: Commands,
    time: Res<Time>,
    mut score: ResMut<Score>,
    mut ragnarok: ResMut<Ragnarok>,
    sfx: Res<SfxBank>,
    game_font: Res<GameFont>,
    mut cams: Query<(&PlayerCam, &mut CamShake)>,
    mut q: Query<(&mut Player, &mut Transform)>,
    bonuses: Query<(Entity, &Bonus)>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::rng();
    let mut taken: Vec<Entity> = Vec::new();
    for (mut player, mut tf) in &mut q {
        if !player.jumping {
            if player.alive() {
                tf.scale = Vec3::ONE;
            }
            continue;
        }
        player.t += dt;
        let k = (player.t / player.dur).min(1.0);
        let pos = player.from.lerp(player.to, k);
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
        tf.rotation = Quat::from_rotation_z(k * TAU);
        let s = (k * PI).sin();
        tf.scale = Vec3::new(1.0 - 0.15 * s, 1.0 + 0.2 * s, 1.0);

        // Traînée.
        let trail = if player.powers.hammer_jumps > 0 || player.dur < 0.3 {
            Color::srgb(1.0, 0.85, 0.3)
        } else {
            Color::srgba(0.7, 0.9, 1.0, 0.8)
        };
        commands.spawn((
            Sprite::from_color(trail, Vec2::splat(rng.random_range(3.0..6.0))),
            Transform::from_translation(pos.extend(4.0)),
            fx::Particle::new(
                Vec2::new(rng.random_range(-40.0..40.0), rng.random_range(-40.0..40.0)),
                0.35,
                0.0,
            ),
            GameEntity,
        ));

        if k < 1.0 {
            continue;
        }

        // Atterrissage.
        player.jumping = false;
        player.on += 1;
        player.landed_at = time.elapsed_secs();
        tf.rotation = Quat::IDENTITY;
        player.level = player.on;
        if player.level > score.best {
            score.best = player.level;
            score.new_record = true;
        }
        sfx.play(&mut commands, Sfx::Land, 0.6);
        fx::burst(&mut commands, pos - Vec2::new(0.0, 20.0), Color::srgb(0.95, 0.97, 1.0), 8, 90.0, 4.0, 200.0, 0.5);
        shake(&mut cams, Some(player.id), 2.0);

        if player.combo >= 2 {
            player.gold += player.combo;
            fx::float_text(
                &mut commands,
                &game_font,
                pos + Vec2::new(0.0, 50.0),
                &format!("BERSERK x{} !", player.combo),
                Color::srgb(1.0, 0.8, 0.2),
                26.0 + player.combo as f32 * 2.0,
            );
            sfx.play(&mut commands, Sfx::Drum, 0.6);
        }

        // Bonus posé sur ce bouclier ? Le premier arrivé le prend.
        for (e, bonus) in &bonuses {
            if bonus.platform != player.on || taken.contains(&e) {
                continue;
            }
            taken.push(e);
            commands.entity(e).despawn();
            sfx.play(&mut commands, Sfx::Pickup, 0.6);
            player.gold += 5;
            let (text, color) = match bonus.kind {
                BonusKind::Hammer => {
                    player.powers.hammer_jumps = 5;
                    ("MJÖLNIR : 5 sauts foudroyants", Color::srgb(1.0, 0.9, 0.4))
                }
                BonusKind::Shield => {
                    player.powers.shield = true;
                    ("BOUCLIER RUNIQUE", Color::srgb(0.5, 0.8, 1.0))
                }
                BonusKind::Mead => {
                    player.powers.mead = 7.0;
                    ("HYDROMEL : le temps ralentit", Color::srgb(1.0, 0.75, 0.3))
                }
            };
            fx::float_text(&mut commands, &game_font, pos + Vec2::new(0.0, 80.0), text, color, 24.0);
            fx::burst(&mut commands, pos, color, 16, 160.0, 5.0, 100.0, 0.7);
        }

        // Tous les dix boucliers : Ragnarök.
        if player.on % 10 == 0 {
            ragnarok.0 = 2.6;
            shake(&mut cams, None, 12.0);
            sfx.play(&mut commands, Sfx::Thunder, 0.8);
            fx::float_text(&mut commands, &game_font, pos + Vec2::new(0.0, 110.0), "RAGNARÖK !", Color::srgb(1.0, 0.25, 0.15), 44.0);
        }
    }
}

fn check_collisions(
    mut commands: Commands,
    track: Res<Track>,
    score: Res<Score>,
    sfx: Res<SfxBank>,
    game_font: Res<GameFont>,
    mut cams: Query<(&PlayerCam, &mut CamShake)>,
    mut q: Query<(&mut Player, &Transform)>,
    enemies: Query<(Entity, &Orbiter, &Enemy)>,
) {
    let mut destroyed: Vec<Entity> = Vec::new();
    for (mut player, tf) in &mut q {
        if !player.jumping {
            continue;
        }
        let pos = tf.translation.truncate();
        for (e, o, enemy) in &enemies {
            if destroyed.contains(&e) || (o.platform != player.on && o.platform != player.on + 1) {
                continue;
            }
            let Some(center) = track.pos(o.platform) else { continue };
            let epos = center + Vec2::from_angle(o.angle) * o.radius;
            if pos.distance(epos) >= PLAYER_R + enemy.radius {
                continue;
            }
            if player.powers.shield {
                player.powers.shield = false;
                destroyed.push(e);
                commands.entity(e).despawn();
                sfx.play(&mut commands, Sfx::Clang, 0.7);
                fx::burst(&mut commands, epos, Color::srgb(0.6, 0.85, 1.0), 20, 240.0, 6.0, 150.0, 0.7);
                fx::float_text(&mut commands, &game_font, pos + Vec2::new(0.0, 60.0), "Le bouclier éclate !", Color::srgb(0.6, 0.85, 1.0), 22.0);
                shake(&mut cams, Some(player.id), 8.0);
                break;
            }
            // Mort : le viking est projeté, puis emporté par les Valkyries.
            player.jumping = false;
            player.dying = Some(0.0);
            player.combo = 0;
            player.vel = (pos - epos).normalize_or(Vec2::Y) * 260.0 + Vec2::new(0.0, 220.0);
            sfx.play(&mut commands, Sfx::Death, 0.8);
            fx::burst(&mut commands, pos, Color::srgb(1.0, 0.85, 0.3), 18, 240.0, 6.0, 120.0, 0.9);
            fx::burst(&mut commands, pos, Color::srgb(0.95, 0.97, 1.0), 14, 160.0, 4.0, 60.0, 1.1);
            shake(&mut cams, Some(player.id), 14.0);
            if score.new_record {
                save_best(score.best);
            }
            break;
        }
    }
}

fn tick_powers(time: Res<Time>, mut q: Query<&mut Player>) {
    for mut player in &mut q {
        if player.powers.mead > 0.0 {
            player.powers.mead = (player.powers.mead - time.delta_secs()).max(0.0);
        }
    }
}

fn dying_anim(time: Res<Time>, mut q: Query<(&mut Player, &mut Transform, &mut Visibility)>) {
    let dt = time.delta_secs();
    for (mut player, mut tf, mut vis) in &mut q {
        let Some(t) = player.dying else { continue };
        player.vel.y -= 900.0 * dt;
        tf.translation += (player.vel * dt).extend(0.0);
        tf.rotate_z(-7.0 * dt);
        if t + dt > DEATH_DURATION {
            player.dying = None;
            player.dead = true;
            *vis = Visibility::Hidden;
        } else {
            player.dying = Some(t + dt);
        }
    }
}

fn check_game_over(q: Query<&Player>, mut next: ResMut<NextState<GameState>>) {
    if !q.is_empty() && q.iter().all(|p| p.dead) {
        next.set(GameState::GameOver);
    }
}

fn camera_follow(
    time: Res<Time>,
    state: Res<State<GameState>>,
    players: Query<(&Player, &Transform), Without<PlayerCam>>,
    mut cams: Query<(&PlayerCam, &mut Transform, &Projection, &mut CamShake)>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::rng();
    for (cam, mut tf, projection, mut shake) in &mut cams {
        let player = players.iter().find(|(p, _)| p.id == cam.0);
        // Le viking reste dans le tiers inférieur de l'écran, quelle que soit sa hauteur.
        let visible_h = match projection {
            Projection::Orthographic(o) => o.area.height().max(crate::VIEW_H),
            _ => crate::VIEW_H,
        };
        let target_y = match (state.get(), player) {
            (GameState::Playing, Some((p, ptf))) if p.alive() => ptf.translation.y + visible_h * 0.21,
            (GameState::Playing | GameState::GameOver, _) => tf.translation.y,
            _ => 120.0,
        };
        let k = 1.0 - (-5.0 * dt).exp();
        let y = tf.translation.y + (target_y - tf.translation.y) * k;
        let off = if shake.0 > 0.05 {
            Vec2::new(rng.random_range(-1.0..1.0), rng.random_range(-1.0..1.0)) * shake.0
        } else {
            Vec2::ZERO
        };
        shake.0 *= (-7.0 * dt).exp();
        tf.translation.x = off.x;
        tf.translation.y = y + off.y;
    }
}

fn aura_visibility(
    time: Res<Time>,
    players: Query<&Player>,
    mut shield: Query<(&mut Visibility, &mut Transform, &ChildOf), (With<ShieldAura>, Without<HammerGlow>)>,
    mut hammer: Query<(&mut Visibility, &mut Transform, &ChildOf), (With<HammerGlow>, Without<ShieldAura>)>,
) {
    let t = time.elapsed_secs();
    for (mut v, mut tf, child_of) in &mut shield {
        let on = players.get(child_of.parent()).is_ok_and(|p| p.powers.shield);
        *v = if on { Visibility::Inherited } else { Visibility::Hidden };
        tf.scale = Vec3::splat(30.0 + 2.0 * (t * 6.0).sin());
    }
    for (mut v, mut tf, child_of) in &mut hammer {
        let on = players.get(child_of.parent()).is_ok_and(|p| p.powers.hammer_jumps > 0);
        *v = if on { Visibility::Inherited } else { Visibility::Hidden };
        tf.scale = Vec3::splat(34.0 + 4.0 * (t * 9.0).sin());
    }
}
