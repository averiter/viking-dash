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

Le jeu s'adapte à l'écran : en portrait sur téléphone on voit plus loin devant soi, en paysage
sur PC plus large ; les textes suivent la plus petite dimension de l'écran. Sur le web, le record
est conservé dans le `localStorage` du navigateur.

## Jouer

- **Clic gauche / Espace / Entrée / Flèche haut** : bondir vers le bouclier suivant.
- Les créatures orbitent autour des boucliers : il faut traverser l'anneau quand la voie est libre.
- **Niveau** = nombre de boucliers franchis. Le record est sauvegardé dans `~/.viking_dash_best`.
- **Berserk** : enchaîne les sauts en moins de 0,7 s pour monter un combo et gagner de l'or.

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
| `src/player.rs` | Saut, collisions, pouvoirs, mort, caméra |
| `src/fx.rs` | Aurore boréale, neige, fjord, drakkar, particules, Ragnarök |
| `src/ui.rs` | Menu, HUD, écran de fin |
| `src/audio.rs` | Synthèse des sons et encodage WAV |
| `src/autotest.rs` | Mode de test automatique (hors web) |
| `web/index.html`, `build-web.sh` | Page et script de build pour le navigateur |
