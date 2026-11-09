//! Dessins « cuits » : chaque bouclier, créature, bonus ou viking est assemblé une fois
//! au démarrage en un seul maillage à couleurs de sommets, puis partagé par toutes ses
//! copies. Une entité par dessin au lieu d'une par forme, et le GPU les dessine par lots.

use bevy::asset::RenderAssetUsages;
use bevy::color::Luminance;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use bevy::sprite_render::AlphaMode2d;
use std::f32::consts::{PI, TAU};

use crate::world::{BonusKind, PLATFORM_R};

/// Couleur d'un sommet (les couleurs de sommets sont en RVB linéaire).
pub fn vcol(c: Color) -> [f32; 4] {
    c.to_linear().to_f32_array()
}

/// Accumule des formes colorées, puis les fond en un maillage. Les formes sont peintes
/// dans l'ordre de leur `z` (le plus petit d'abord), comme des calques.
#[derive(Default)]
pub struct Baker {
    tris: Vec<(f32, [Vec2; 3], [f32; 4])>,
}

impl Baker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Polygone convexe plein.
    pub fn poly(&mut self, color: Color, z: f32, pts: &[Vec2]) {
        let c = vcol(color);
        for k in 1..pts.len().saturating_sub(1) {
            self.tris.push((z, [pts[0], pts[k], pts[k + 1]], c));
        }
    }

    /// Secteur d'anneau entre deux rayons et deux angles (un disque si `r_in` vaut 0).
    pub fn arc(&mut self, color: Color, pos: Vec2, z: f32, r_in: f32, r_out: f32, a0: f32, a1: f32) {
        // Assez de segments pour qu'un petit disque reste rond (et non un triangle).
        let segs = (((a1 - a0).abs() * r_out.sqrt() * 1.6).ceil() as usize).clamp(3, 72);
        for k in 0..segs {
            let u0 = Vec2::from_angle(a0 + (a1 - a0) * k as f32 / segs as f32);
            let u1 = Vec2::from_angle(a0 + (a1 - a0) * (k + 1) as f32 / segs as f32);
            if r_in <= 0.0 {
                self.poly(color, z, &[pos, pos + u0 * r_out, pos + u1 * r_out]);
            } else {
                self.poly(color, z, &[pos + u0 * r_in, pos + u0 * r_out, pos + u1 * r_out, pos + u1 * r_in]);
            }
        }
    }

    pub fn circle(&mut self, color: Color, pos: Vec2, z: f32, r: f32) {
        self.arc(color, pos, z, 0.0, r, 0.0, TAU);
    }

    pub fn ring(&mut self, color: Color, pos: Vec2, z: f32, r: f32) {
        self.arc(color, pos, z, r * 0.88, r, 0.0, TAU);
    }

    pub fn ellipse(&mut self, color: Color, pos: Vec2, z: f32, rx: f32, ry: f32, rot: f32) {
        let segs = ((rx.max(ry) * 1.3).ceil() as usize).clamp(10, 48);
        let rot = Vec2::from_angle(rot);
        let pts: Vec<Vec2> = (0..segs)
            .map(|k| {
                let a = k as f32 * TAU / segs as f32;
                pos + rot.rotate(Vec2::new(rx * a.cos(), ry * a.sin()))
            })
            .collect();
        self.poly(color, z, &pts);
    }

    pub fn rect(&mut self, color: Color, pos: Vec2, z: f32, size: Vec2, rot: f32) {
        let rot = Vec2::from_angle(rot);
        let h = size / 2.0;
        let pts = [Vec2::new(-h.x, -h.y), Vec2::new(h.x, -h.y), Vec2::new(h.x, h.y), Vec2::new(-h.x, h.y)];
        self.poly(color, z, &pts.map(|p| pos + rot.rotate(p)));
    }

    /// Triangle isocèle pointe en haut, de largeur `size.x` et de hauteur `size.y`.
    pub fn tri(&mut self, color: Color, pos: Vec2, z: f32, size: Vec2, rot: f32) {
        let rot = Vec2::from_angle(rot);
        let pts = [Vec2::new(-0.5, -0.5), Vec2::new(0.5, -0.5), Vec2::new(0.0, 0.5)];
        self.poly(color, z, &pts.map(|p| pos + rot.rotate(p * size)));
    }

    pub fn hex(&mut self, color: Color, pos: Vec2, z: f32, r: f32) {
        let pts: Vec<Vec2> = (0..6).map(|k| pos + Vec2::from_angle(PI / 2.0 + k as f32 * TAU / 6.0) * r).collect();
        self.poly(color, z, &pts);
    }

    pub fn build(mut self) -> Mesh {
        self.tris.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut positions = Vec::with_capacity(self.tris.len() * 3);
        let mut colors = Vec::with_capacity(self.tris.len() * 3);
        for (_, pts, color) in &self.tris {
            for p in pts {
                positions.push([p.x, p.y, 0.0]);
                colors.push(*color);
            }
        }
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    }
}

const SHIELD_COLORS: [Color; 6] = [
    Color::srgb(0.70, 0.18, 0.14),
    Color::srgb(0.16, 0.36, 0.62),
    Color::srgb(0.82, 0.62, 0.18),
    Color::srgb(0.22, 0.50, 0.30),
    Color::srgb(0.55, 0.22, 0.52),
    Color::srgb(0.86, 0.84, 0.76),
];
/// Motifs peints : croix, quartiers, soleil, cercle.
const SHIELD_PATTERNS: usize = 4;

pub type ArtBundle = (Mesh2d, MeshMaterial2d<ColorMaterial>, Transform);

/// Tous les dessins cuits, prêts à être posés dans le monde.
#[derive(Resource)]
pub struct Art {
    /// Matériau blanc translucide : la couleur vient des sommets.
    pub mat: Handle<ColorMaterial>,
    shields: Vec<Handle<Mesh>>,
    /// Chemin d'orbite en pointillés, de rayon 1.
    pub orbit: Handle<Mesh>,
    pub raven_body: Handle<Mesh>,
    pub raven_wing: Handle<Mesh>,
    pub draugr: Handle<Mesh>,
    pub wolf: Handle<Mesh>,
    pub kraken: Handle<Mesh>,
    pub jotunn: Handle<Mesh>,
    pub serpent_head: Handle<Mesh>,
    pub serpent_body: [Handle<Mesh>; 2],
    bonus: [Handle<Mesh>; 3],
    pub viking: [Handle<Mesh>; 2],
}

impl Art {
    /// Bouclier de la plateforme `index` : la couleur et le motif varient au fil de la piste.
    pub fn shield(&self, index: u32) -> &Handle<Mesh> {
        let i = index as usize;
        let color = i % SHIELD_COLORS.len();
        let pattern = (i / 2 + i / 7) % SHIELD_PATTERNS;
        &self.shields[color * SHIELD_PATTERNS + pattern]
    }

    pub fn bonus(&self, kind: BonusKind) -> &Handle<Mesh> {
        &self.bonus[kind as usize]
    }

    pub fn at(&self, mesh: &Handle<Mesh>, pos: Vec2, z: f32) -> ArtBundle {
        self.scaled(mesh, pos, z, 1.0)
    }

    pub fn scaled(&self, mesh: &Handle<Mesh>, pos: Vec2, z: f32, scale: f32) -> ArtBundle {
        (
            Mesh2d(mesh.clone()),
            MeshMaterial2d(self.mat.clone()),
            Transform::from_translation(pos.extend(z)).with_scale(Vec3::new(scale, scale, 1.0)),
        )
    }
}

pub struct ArtPlugin;

impl Plugin for ArtPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, setup_art);
    }
}

fn setup_art(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<ColorMaterial>>) {
    let mat = mats.add(ColorMaterial {
        color: Color::WHITE,
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    });
    let mut shields = Vec::new();
    for face in SHIELD_COLORS {
        for pattern in 0..SHIELD_PATTERNS {
            shields.push(meshes.add(shield(face, pattern)));
        }
    }
    commands.insert_resource(Art {
        mat,
        shields,
        orbit: meshes.add(orbit()),
        raven_body: meshes.add(raven_body()),
        raven_wing: meshes.add(raven_wing()),
        draugr: meshes.add(draugr()),
        wolf: meshes.add(wolf()),
        kraken: meshes.add(kraken()),
        jotunn: meshes.add(jotunn()),
        serpent_head: meshes.add(serpent_head()),
        serpent_body: [meshes.add(serpent_body(0)), meshes.add(serpent_body(1))],
        bonus: [
            meshes.add(bonus(BonusKind::Hammer)),
            meshes.add(bonus(BonusKind::Shield)),
            meshes.add(bonus(BonusKind::Mead)),
        ],
        viking: [meshes.add(viking(0)), meshes.add(viking(1))],
    });
}

// ---------- boucliers ----------

fn shield(face: Color, pattern: usize) -> Mesh {
    const R: f32 = PLATFORM_R;
    let mut b = Baker::new();
    let dark = face.darker(0.12);
    let iron = Color::srgb(0.42, 0.44, 0.48);
    let iron_light = Color::srgb(0.72, 0.74, 0.78);
    let o = Vec2::ZERO;

    // Ombre portée, face, joints des planches.
    b.circle(Color::srgba(0.0, 0.0, 0.05, 0.45), Vec2::new(5.0, -7.0), 0.8, R);
    b.circle(face, o, 1.0, R);
    for x in [-36.0_f32, -18.0, 0.0, 18.0, 36.0] {
        let half = (R * R - x * x).sqrt();
        b.rect(face.darker(0.045), Vec2::new(x, 0.0), 1.02, Vec2::new(1.6, 2.0 * half - 6.0), 0.0);
    }

    // Motif peint.
    match pattern {
        0 => {
            b.rect(dark, o, 1.1, Vec2::new(R * 1.9, 7.0), 0.0);
            b.rect(dark, o, 1.1, Vec2::new(7.0, R * 1.9), 0.0);
        }
        1 => {
            b.arc(dark, o, 1.1, 0.0, R - 5.0, 0.0, PI / 2.0);
            b.arc(dark, o, 1.1, 0.0, R - 5.0, PI, 1.5 * PI);
        }
        2 => {
            for k in 0..8 {
                let a = k as f32 * TAU / 8.0;
                b.arc(dark, o, 1.1, 0.0, R - 5.0, a, a + TAU / 16.0);
            }
        }
        _ => {
            b.arc(dark, o, 1.1, 24.0, 35.0, 0.0, TAU);
            for k in 0..4 {
                b.circle(dark, Vec2::from_angle(k as f32 * TAU / 4.0 + PI / 4.0) * 43.0, 1.1, 3.2);
            }
        }
    }

    // Reflet doux en haut à gauche, ombre légère en bas à droite.
    b.arc(Color::srgba(1.0, 1.0, 1.0, 0.10), o, 1.15, 30.0, 45.0, 1.75, 2.95);
    b.arc(Color::srgba(0.0, 0.0, 0.05, 0.12), o, 1.15, 34.0, 48.0, -1.35, -0.2);

    // Cerclage de fer avec arête éclairée et arête dans l'ombre, umbo, rivets.
    b.ring(iron, o, 1.2, R);
    b.arc(iron_light.with_alpha(0.75), o, 1.25, R * 0.95, R, 1.5, 3.1);
    b.arc(iron.darker(0.12), o, 1.25, R * 0.88, R * 0.94, -1.6, 0.0);
    b.circle(iron.darker(0.1), o, 1.28, 13.0);
    b.circle(iron, o, 1.3, 11.0);
    b.circle(iron_light, Vec2::new(-3.0, 3.0), 1.4, 4.0);
    for k in 0..8 {
        let a = k as f32 * TAU / 8.0 + PI / 8.0;
        b.circle(iron_light, Vec2::from_angle(a) * (R - 3.5), 1.3, 2.4);
    }
    b.build()
}

fn orbit() -> Mesh {
    let mut b = Baker::new();
    let color = Color::srgba(0.8, 0.85, 1.0, 0.26);
    const DASHES: usize = 40;
    for k in 0..DASHES {
        let a = k as f32 * TAU / DASHES as f32;
        // Rayon 1 : peu de segments suffisent, la courbure d'un tiret est invisible.
        let (a0, a1) = (a, a + TAU / DASHES as f32 * 0.5);
        let (u0, u1) = (Vec2::from_angle(a0), Vec2::from_angle(a1));
        b.poly(color, 0.0, &[u0 * 0.972, u0, u1, u1 * 0.972]);
    }
    b.build()
}

// ---------- créatures ----------

fn raven_body() -> Mesh {
    // Plumage bleu nuit cerné de clair pour rester lisible sur le ciel.
    let mut b = Baker::new();
    let feather = Color::srgb(0.16, 0.20, 0.34);
    let edge = Color::srgb(0.75, 0.82, 1.0);
    let beak = Color::srgb(1.0, 0.72, 0.15);
    let z = 2.0;
    // Queue.
    b.tri(edge, Vec2::new(-21.0, 0.0), z - 0.12, Vec2::new(17.0, 16.0), PI / 2.0);
    b.tri(feather, Vec2::new(-20.0, 0.0), z - 0.11, Vec2::new(12.0, 12.0), PI / 2.0);
    b.ellipse(edge, Vec2::ZERO, z - 0.1, 21.0, 14.0, 0.0);
    b.ellipse(feather, Vec2::ZERO, z, 18.5, 11.5, 0.0);
    b.ellipse(feather.lighter(0.07), Vec2::new(2.0, 3.0), z + 0.02, 11.0, 5.0, 0.0);
    b.circle(edge, Vec2::new(16.0, 6.0), z + 0.05, 11.0);
    b.circle(feather, Vec2::new(16.0, 6.0), z + 0.1, 9.0);
    b.tri(beak, Vec2::new(28.0, 5.0), z + 0.1, Vec2::new(9.0, 13.0), -PI / 2.0);
    b.circle(Color::WHITE, Vec2::new(18.0, 8.5), z + 0.2, 3.6);
    b.circle(Color::srgb(0.05, 0.05, 0.1), Vec2::new(19.2, 8.5), z + 0.3, 1.8);
    for x in [-4.0, 2.0] {
        b.rect(beak, Vec2::new(x, -13.0), z - 0.05, Vec2::new(2.5, 7.0), 0.0);
    }
    b.build()
}

/// Aile de corbeau, centrée : l'entité qui la porte bat autour de ce centre.
fn raven_wing() -> Mesh {
    let mut b = Baker::new();
    b.tri(Color::srgb(0.75, 0.82, 1.0), Vec2::ZERO, 0.0, Vec2::new(22.0, 30.0), 0.0);
    b.tri(Color::srgb(0.16, 0.20, 0.34), Vec2::ZERO, 0.1, Vec2::new(17.0, 25.0), 0.0);
    b.build()
}

/// Draugr sans ses pupilles rouges (elles palpitent : ce sont des entités à part).
fn draugr() -> Mesh {
    let mut b = Baker::new();
    let skin = Color::srgb(0.30, 0.52, 0.30);
    let rust = Color::srgb(0.50, 0.36, 0.24);
    let z = 2.0;
    b.circle(skin, Vec2::ZERO, z, 16.0);
    b.arc(skin.lighter(0.06), Vec2::ZERO, z + 0.02, 0.0, 11.0, 0.9, 2.6);
    b.rect(skin.darker(0.1), Vec2::new(0.0, -10.0), z + 0.1, Vec2::new(18.0, 8.0), 0.0);
    for k in 0..3 {
        b.rect(Color::srgb(0.95, 0.95, 0.85), Vec2::new(-5.0 + k as f32 * 5.0, -8.0), z + 0.2, Vec2::new(3.0, 4.0), 0.0);
    }
    // Reste de casque rouillé.
    b.arc(rust, Vec2::ZERO, z + 0.15, 12.0, 17.0, 0.5, PI - 0.5);
    b.rect(rust.darker(0.08), Vec2::new(0.0, 12.0), z + 0.16, Vec2::new(3.0, 9.0), 0.0);
    for side in [-1.0, 1.0] {
        b.circle(Color::srgb(0.05, 0.1, 0.05), Vec2::new(side * 6.0, 3.0), z + 0.1, 4.5);
    }
    b.build()
}

fn wolf() -> Mesh {
    let mut b = Baker::new();
    let fur = Color::srgb(0.55, 0.57, 0.62);
    let z = 2.0;
    b.circle(fur, Vec2::ZERO, z, 15.0);
    b.arc(fur.darker(0.07), Vec2::ZERO, z + 0.02, 0.0, 15.0, 2.2, 4.1);
    b.ellipse(fur.lighter(0.08), Vec2::new(11.0, -3.0), z + 0.1, 9.0, 6.5, 0.0);
    b.circle(Color::srgb(0.1, 0.1, 0.1), Vec2::new(18.0, -2.0), z + 0.2, 3.0);
    for (x, rot) in [(-7.0, 0.2), (3.0, -0.2)] {
        b.tri(fur.darker(0.1), Vec2::new(x, 12.0), z - 0.1, Vec2::new(9.0, 13.0), rot);
        b.tri(Color::srgb(0.80, 0.62, 0.62), Vec2::new(x, 11.0), z - 0.05, Vec2::new(4.5, 7.0), rot);
    }
    for x in [-3.0, 6.0] {
        b.circle(Color::srgb(1.0, 0.85, 0.2), Vec2::new(x, 4.0), z + 0.2, 2.8);
        b.circle(Color::BLACK, Vec2::new(x + 0.8, 4.0), z + 0.3, 1.3);
    }
    b.tri(Color::WHITE, Vec2::new(12.0, -9.0), z + 0.2, Vec2::new(3.0, 5.0), PI);
    b.tri(Color::WHITE, Vec2::new(7.5, -9.0), z + 0.2, Vec2::new(2.6, 4.0), PI);
    b.build()
}

/// Corps du kraken (ses tentacules ondulent : entités à part).
fn kraken() -> Mesh {
    let mut b = Baker::new();
    let purple = Color::srgb(0.48, 0.18, 0.55);
    let z = 2.0;
    b.circle(purple, Vec2::ZERO, z, 15.0);
    b.ellipse(purple.lighter(0.1), Vec2::new(0.0, 6.0), z + 0.1, 12.0, 8.0, 0.0);
    for (x, y, r) in [(-6.0, 9.0, 1.8), (5.0, 10.0, 2.2), (0.0, 12.0, 1.5)] {
        b.circle(purple.lighter(0.22), Vec2::new(x, y), z + 0.15, r);
    }
    for x in [-5.0, 5.0] {
        b.circle(Color::srgb(1.0, 0.95, 0.5), Vec2::new(x, 0.0), z + 0.2, 3.5);
        b.ellipse(Color::BLACK, Vec2::new(x, 0.0), z + 0.3, 1.2, 2.8, 0.0);
    }
    b.build()
}

fn jotunn() -> Mesh {
    let mut b = Baker::new();
    let ice = Color::srgb(0.62, 0.82, 0.95);
    let deep = Color::srgb(0.05, 0.15, 0.45);
    let z = 2.0;
    b.hex(ice.darker(0.12), Vec2::new(1.0, -1.5), z - 0.05, 19.0);
    b.hex(ice, Vec2::ZERO, z, 18.0);
    b.hex(ice.lighter(0.1), Vec2::new(-2.0, 2.0), z + 0.05, 12.0);
    b.poly(Color::srgba(1.0, 1.0, 1.0, 0.35), z + 0.06, &[Vec2::new(-11.0, 4.0), Vec2::new(-3.0, 12.0), Vec2::new(-9.0, 11.0)]);
    for (x, h) in [(-9.0, 10.0), (0.0, 15.0), (8.0, 11.0)] {
        b.tri(Color::srgb(0.9, 0.98, 1.0), Vec2::new(x, 16.0 + h * 0.3), z - 0.1, Vec2::new(6.0, h), 0.0);
    }
    for x in [-5.0, 5.0] {
        b.circle(deep, Vec2::new(x, 2.0), z + 0.2, 2.6);
    }
    b.rect(deep, Vec2::new(0.0, -7.0), z + 0.2, Vec2::new(10.0, 2.0), 0.0);
    b.build()
}

fn serpent_head() -> Mesh {
    let mut b = Baker::new();
    let green = Color::srgb(0.18, 0.55, 0.30);
    let z = 2.5;
    b.ellipse(green, Vec2::ZERO, z, 21.0, 16.0, 0.0);
    b.ellipse(green.lighter(0.08), Vec2::new(-4.0, 0.0), z + 0.02, 12.0, 8.0, 0.0);
    b.tri(Color::srgb(0.95, 0.3, 0.2), Vec2::new(26.0, 0.0), z - 0.1, Vec2::new(5.0, 16.0), -PI / 2.0);
    for y in [-6.0, 6.0] {
        b.circle(Color::srgb(1.0, 0.85, 0.1), Vec2::new(8.0, y), z + 0.1, 3.6);
        b.ellipse(Color::BLACK, Vec2::new(8.5, y), z + 0.2, 1.2, 3.0, 0.0);
    }
    for y in [-9.0, 9.0] {
        b.tri(Color::WHITE, Vec2::new(17.0, y), z + 0.1, Vec2::new(3.0, 6.0), -PI / 2.0);
    }
    // Crête.
    for x in [-12.0, -4.0] {
        b.tri(green.darker(0.1), Vec2::new(x, 0.0), z + 0.05, Vec2::new(5.0, 7.0), PI / 2.0);
    }
    b.build()
}

fn serpent_body(variant: usize) -> Mesh {
    let mut b = Baker::new();
    let shade = if variant == 0 { Color::srgb(0.18, 0.5, 0.28) } else { Color::srgb(0.25, 0.62, 0.34) };
    b.circle(shade, Vec2::ZERO, 0.0, 14.0);
    b.ring(shade.darker(0.12), Vec2::ZERO, 0.05, 14.0);
    b.arc(shade.lighter(0.1), Vec2::ZERO, 0.06, 5.0, 9.0, 0.6, 2.5);
    b.build()
}

// ---------- bonus ----------

fn bonus(kind: BonusKind) -> Mesh {
    let mut b = Baker::new();
    match kind {
        BonusKind::Hammer => {
            b.rect(Color::srgb(0.45, 0.28, 0.14), Vec2::new(0.0, -6.0), 0.1, Vec2::new(5.0, 22.0), 0.0);
            b.rect(Color::srgb(0.75, 0.78, 0.85), Vec2::new(0.0, 7.0), 0.2, Vec2::new(22.0, 12.0), 0.0);
            b.rect(Color::srgb(0.90, 0.92, 0.97), Vec2::new(0.0, 11.0), 0.25, Vec2::new(22.0, 3.0), 0.0);
            b.rect(Color::srgb(0.95, 0.85, 0.4), Vec2::new(0.0, 7.0), 0.3, Vec2::new(4.0, 8.0), 0.0);
        }
        BonusKind::Shield => {
            let pale = Color::srgb(0.9, 0.95, 1.0);
            b.circle(Color::srgb(0.2, 0.5, 0.95), Vec2::ZERO, 0.1, 13.0);
            b.ring(pale, Vec2::ZERO, 0.2, 13.0);
            b.rect(pale, Vec2::ZERO, 0.3, Vec2::new(3.0, 18.0), 0.0);
            b.rect(pale, Vec2::new(0.0, 3.0), 0.3, Vec2::new(12.0, 3.0), 0.0);
        }
        BonusKind::Mead => {
            b.tri(Color::srgb(0.93, 0.88, 0.75), Vec2::new(0.0, -2.0), 0.1, Vec2::new(16.0, 26.0), PI + 0.35);
            b.ellipse(Color::srgb(0.95, 0.72, 0.2), Vec2::new(-4.0, 9.0), 0.2, 9.0, 4.0, 0.35);
            b.circle(Color::srgb(1.0, 0.95, 0.8), Vec2::new(-6.0, 12.0), 0.3, 3.0);
        }
    }
    b.build()
}

// ---------- vikings ----------

/// Joueur 1 : cape rouge et barbe rousse. Joueur 2 : cape bleue et barbe brune.
fn viking(id: u8) -> Mesh {
    let mut b = Baker::new();
    let skin = Color::srgb(0.95, 0.78, 0.62);
    let (cape, tunic, beard) = if id == 0 {
        (Color::srgb(0.6, 0.12, 0.12), Color::srgb(0.45, 0.30, 0.16), Color::srgb(0.85, 0.42, 0.12))
    } else {
        (Color::srgb(0.12, 0.3, 0.7), Color::srgb(0.30, 0.32, 0.40), Color::srgb(0.35, 0.20, 0.10))
    };
    let iron = Color::srgb(0.55, 0.58, 0.64);
    let horn = Color::srgb(0.96, 0.93, 0.85);
    // Cape.
    b.tri(cape, Vec2::new(-4.0, -6.0), -0.2, Vec2::new(30.0, 34.0), PI + 0.3);
    // Jambes et bottes.
    for x in [-6.0, 6.0] {
        b.rect(Color::srgb(0.25, 0.18, 0.12), Vec2::new(x, -19.0), 0.0, Vec2::new(8.0, 10.0), 0.0);
        b.rect(Color::srgb(0.2, 0.12, 0.08), Vec2::new(x, -25.0), 0.1, Vec2::new(10.0, 5.0), 0.0);
    }
    // Corps, ceinture, fourrure.
    b.rect(tunic, Vec2::new(0.0, -6.0), 0.1, Vec2::new(26.0, 24.0), 0.0);
    b.rect(tunic.lighter(0.06), Vec2::new(-8.0, -6.0), 0.12, Vec2::new(6.0, 24.0), 0.0);
    b.rect(Color::srgb(0.2, 0.14, 0.08), Vec2::new(0.0, -11.0), 0.2, Vec2::new(26.0, 5.0), 0.0);
    b.circle(Color::srgb(0.85, 0.75, 0.55), Vec2::new(0.0, -11.0), 0.3, 3.0);
    b.ellipse(Color::srgb(0.65, 0.5, 0.35), Vec2::new(0.0, 4.0), 0.2, 16.0, 5.0, 0.0);
    // Tête, barbe, yeux.
    b.circle(skin, Vec2::new(0.0, 13.0), 0.3, 11.0);
    b.circle(beard, Vec2::new(0.0, 6.0), 0.4, 9.5);
    b.rect(beard, Vec2::new(0.0, 2.0), 0.4, Vec2::new(14.0, 8.0), 0.0);
    b.ellipse(beard.darker(0.08), Vec2::new(0.0, 10.0), 0.5, 7.0, 2.2, 0.0);
    for x in [-4.0, 4.0] {
        b.circle(Color::WHITE, Vec2::new(x, 15.0), 0.5, 2.6);
        b.circle(Color::srgb(0.1, 0.2, 0.4), Vec2::new(x + 0.6, 15.0), 0.6, 1.3);
    }
    // Casque et cornes.
    b.ellipse(iron, Vec2::new(0.0, 20.0), 0.6, 12.5, 9.0, 0.0);
    b.rect(iron.darker(0.1), Vec2::new(0.0, 17.0), 0.7, Vec2::new(25.0, 4.0), 0.0);
    b.rect(iron.lighter(0.15), Vec2::new(0.0, 24.0), 0.7, Vec2::new(3.0, 8.0), 0.0);
    b.tri(horn, Vec2::new(-14.0, 25.0), 0.5, Vec2::new(8.0, 16.0), 0.6);
    b.tri(horn, Vec2::new(14.0, 25.0), 0.5, Vec2::new(8.0, 16.0), -0.6);
    // Hache.
    b.rect(Color::srgb(0.4, 0.26, 0.14), Vec2::new(19.0, -2.0), -0.1, Vec2::new(4.0, 34.0), -0.25);
    b.tri(iron.lighter(0.1), Vec2::new(24.0, 14.0), 0.0, Vec2::new(16.0, 14.0), -PI / 2.0 - 0.25);
    b.build()
}
