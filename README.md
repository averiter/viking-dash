# Viking Dash — La Route de Valhalla

Jeu d'adresse en Rust + [Bevy 0.20](https://bevy.org), inspiré du *Cochon d'Halloween* :
saute de bouclier en bouclier en cliquant au bon moment, sans toucher les créatures
qui tournent autour.

## Lancer

```bash
cargo run --release
```

(La première compilation de Bevy prend quelques minutes. `cargo run` sans `--release`
est aussi fluide grâce au profil dev optimisé.)

## Version web (PC et smartphone)

```bash
./build-web.sh
python3 -m http.server 8080 -d web
```

puis ouvre <http://localhost:8080>. Le dossier `web/` (page, `pkg/` et `assets/`) se déploie tel quel
sur n'importe quel hébergement statique (GitHub Pages, Netlify…). Prérequis une seule fois :

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130
brew install binaryen   # wasm-opt, facultatif : réduit le .wasm d'environ un quart
```

Le `.wasm` pèse environ 20 Mo (5,7 Mo une fois compressé en gzip par l'hébergeur) grâce à une
sélection minimale des fonctionnalités Bevy dans `Cargo.toml` (2D, interface, texte, audio).

### Déploiement sur GitHub Pages

Le workflow [`.github/workflows/deploy.yml`](.github/workflows/deploy.yml) compile la version web
à chaque push sur `master` (Rust stable, `wasm-bindgen` 0.2.130 et `wasm-opt` téléchargés depuis
leurs releases) et publie le dossier `web/` sur <https://averiter.github.io/viking-dash/>.
Le cache cargo rend les builds suivants nettement plus rapides que le premier (~15 min).

Le jeu s'adapte à l'écran : en portrait sur téléphone on voit plus loin devant soi, en paysage
sur PC plus large ; les textes suivent la plus petite dimension de l'écran. Sur le web, le record
est conservé dans le `localStorage` du navigateur.

## Jouer

- **Clic gauche / Espace / Entrée / Flèche haut / n'importe quel bouton de manette** : bondir vers le bouclier suivant.
- Les créatures orbitent autour des boucliers : il faut traverser l'anneau quand la voie est libre.
- **Niveau** = nombre de boucliers franchis. Le record est sauvegardé dans `~/.viking_dash_best`.
- **Berserk** : enchaîne les sauts en moins de 0,7 s pour monter un combo et gagner de l'or.
- **Aide** : bouton « AIDE » du menu (touche **H**, **3**, **F1** ou bouton **X** de la manette). Cinq pages
  illustrées avec les dessins du jeu : règles, pouvoirs, or, créatures, chacun avec une phrase sur son origine dans la mythologie nordique. Flèches ou clic pour tourner
  les pages, **Échap** pour revenir au menu.

### À deux (écran partagé)

Choisis **2 JOUEURS** dans le menu (clic, touche 2 ou bouton Y d'une manette). L'écran se partage
en deux : côte à côte en paysage, l'un au-dessus de l'autre en portrait. Les deux vikings courent
sur la même piste de boucliers ; le premier arrivé sur un bonus le prend, et l'hydromel ralentit
les créatures pour tout le monde. Quand les deux sont tombés, le niveau le plus haut l'emporte.

| | Joueur 1 (cape rouge) | Joueur 2 (cape bleue) |
|---|---|---|
| Clavier | Espace, flèche haut, W/Z | Entrée, Maj droite |
| Manette | première manette connectée | seconde manette |
| Souris / tactile | moitié gauche (haute en portrait) | moitié droite (basse en portrait) |

Échap abandonne la partie et revient au menu.

### Bonus (posés au centre de certains boucliers)

| Bonus | Effet |
|---|---|
| Mjölnir | 5 sauts foudroyants, presque deux fois plus rapides |
| Bouclier runique | Encaisse un coup et pulvérise la créature |
| Hydromel | Les créatures ralentissent pendant 7 s |

### Créatures, par ordre d'apparition

Corbeaux d'Odin, Draugr, Loups de Fenrir, Kraken, Jötunn de glace…
et tous les dix niveaux, **Ragnarök** : le ciel s'embrase et Jörmungandr encercle le bouclier.
À partir du niveau 15, certains boucliers ont un second anneau ; à partir du 22, la vitesse
des créatures devient irrégulière.

## Technique

- Ciel à petit budget : l'aurore est un seul maillage à couleurs de sommets (redessiné à chaque
  frame), le dégradé du ciel et la vignette sont deux quadrilatères, et tous les halos partagent une
  texture de 64 × 64 créée en mémoire. Pas de post-traitement.
- Dessins « cuits » : chaque bouclier, créature, bonus ou viking est assemblé une fois au démarrage
  en un seul maillage à couleurs de sommets (`src/art.rs`), partagé par toutes ses copies. Un
  bouclier compte ainsi 3 ou 4 entités au lieu de 18, et le GPU dessine les copies par lots.
- Décor d'altitude en parallaxe : des branches d'Yggdrasil entrent par les bords vers le niveau 12,
  le Bifröst apparaît au-delà du niveau 30 ; une douzaine d'emplacements par caméra, recyclés au
  fil de la montée.
- Fjord : huit bandes de houle cuites en maillages glissent à des vitesses différentes (8 entités
  au lieu de 126 vaguelettes) ; le drakkar flotte entre deux bandes, et son reflet est le même
  maillage retourné sous un matériau sombre.
- Aucun asset externe : tout est dessiné avec des primitives (`Mesh2d`) et les sons
  (cors, tambours, chocs, tonnerre) sont synthétisés en mémoire au démarrage (`src/audio.rs`).
- Polices : titres et chiffres en *Luminari* (système) sur macOS, sinon en Fira Sans Bold embarquée
  (`assets/fonts`, licence OFL), notamment sur le web ; les textes courants (consignes, aides) sont
  toujours en Fira Sans, plus lisible en petit.
- `VIKING_WINDOW=390x844 cargo run` ouvre une fenêtre au format téléphone portrait.
- `VIKING_START=30 cargo run` démarre directement au niveau 30.
- `VIKING_ALTITUDE=8000 cargo run` montre tout de suite le décor des hauteurs (branches dès 2800,
  Bifröst dès 7000).
- `VIKING_AUTOTEST=<dossier> cargo run` joue tout seul et enregistre des captures d'écran
  (utilisé pour vérifier le rendu). `VIKING_AUTOTEST_HELP=<n>` ouvre la page n de l'aide.

## Structure

| Fichier | Rôle |
|---|---|
| `src/main.rs` | États, ressources, caméra, sauvegarde du record |
| `src/art.rs` | Dessins cuits en maillages : boucliers, créatures, bonus, vikings |
| `src/world.rs` | Piste de boucliers, créatures en orbite, bonus, génération et difficulté |
| `src/input.rs` | Clavier, souris, tactile, manettes : attribution aux joueurs |
| `src/player.rs` | Sauts, collisions, pouvoirs, mort, caméras (une par joueur) |
| `src/fx.rs` | Ciel, lune, aurore, décor en parallaxe, montagnes, fjord, drakkar, particules, ondes, Ragnarök |
| `src/ui.rs` | Menu, HUD, écran de fin |
| `src/help.rs` | Pages d'aide : textes en UI, dessins du jeu rendus par-dessus par une caméra à part |
| `src/audio.rs` | Synthèse des sons et encodage WAV |
| `src/autotest.rs` | Mode de test automatique (hors web) |
| `web/index.html`, `build-web.sh` | Page et script de build pour le navigateur |
| `.github/workflows/deploy.yml` | Build web et publication sur GitHub Pages |
