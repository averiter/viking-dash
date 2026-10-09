# Viking Dash — La Route de Valhalla

Jeu d'adresse en Rust + [Bevy 0.19](https://bevy.org), inspiré du *Cochon d'Halloween* :
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
cargo install wasm-bindgen-cli --version 0.2.129
brew install binaryen   # wasm-opt, facultatif : réduit le .wasm d'environ un quart
```

Le `.wasm` pèse environ 20 Mo (5,7 Mo une fois compressé en gzip par l'hébergeur) grâce à une
sélection minimale des fonctionnalités Bevy dans `Cargo.toml` (2D, interface, texte, audio).

### Déploiement sur GitHub Pages

Le workflow [`.github/workflows/deploy.yml`](.github/workflows/deploy.yml) compile la version web
à chaque push sur `master` (Rust stable, `wasm-bindgen` 0.2.129 et `wasm-opt` téléchargés depuis
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

- Aucun asset externe : tout est dessiné avec des primitives (`Mesh2d`) et les sons
  (cors, tambours, chocs, tonnerre) sont synthétisés en mémoire au démarrage (`src/audio.rs`).
- Police : *Luminari* (système) sur macOS, sinon Fira Sans Bold embarquée (`assets/fonts`, licence OFL),
  notamment sur le web.
- `VIKING_WINDOW=390x844 cargo run` ouvre une fenêtre au format téléphone portrait.
- `VIKING_START=30 cargo run` démarre directement au niveau 30.
- `VIKING_AUTOTEST=<dossier> cargo run` joue tout seul et enregistre des captures d'écran
  (utilisé pour vérifier le rendu).

## Structure

| Fichier | Rôle |
|---|---|
| `src/main.rs` | États, ressources, caméra, sauvegarde du record |
| `src/world.rs` | Boucliers, créatures, bonus, génération et difficulté |
| `src/input.rs` | Clavier, souris, tactile, manettes : attribution aux joueurs |
| `src/player.rs` | Sauts, collisions, pouvoirs, mort, caméras (une par joueur) |
| `src/fx.rs` | Aurore boréale, neige, fjord, drakkar, particules, Ragnarök |
| `src/ui.rs` | Menu, HUD, écran de fin |
| `src/audio.rs` | Synthèse des sons et encodage WAV |
| `src/autotest.rs` | Mode de test automatique (hors web) |
| `web/index.html`, `build-web.sh` | Page et script de build pour le navigateur |
| `.github/workflows/deploy.yml` | Build web et publication sur GitHub Pages |
