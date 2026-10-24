# Photos de scènes du site

[English](README.en.md) · Français

Les photos de l'accueil (le salon du hero, les quatre scènes « Au quotidien », le salon du Raspberry Pi) sont des pièces générées par IA dans lesquelles on pose de **vraies captures de KyberNow Link**. Les personnes et les pièces sont inventées ; les interfaces sur les écrans sont celles de l'app, au pixel près.

## Comment

1. **La pièce** est générée avec Z-Image Turbo sur ComfyUI (`zimage.py`, réglages repris du flux « Image » : 8 étapes, `res_multistep`, CFG 1, AuraFlow 3). L'écran (télé, Steam Deck, portable) y est demandé d'un vert chroma uni.
2. **La capture** vient de l'app Linux, rendue par `kybernow --preview` en français et en anglais : accueil au salon (`salon-01-accueil`), accueil du Steam Deck (`deck-01-accueil`), accueil du bureau (`bureau-01-accueil`), accueil de KyberNow Link OS (`os-10b-accueil-a-jour`). Les écrans « en partie » montrent un jeu générique, généré lui aussi.
3. **La pose** (`composite.py`) trouve le vert, ses quatre coins, et y pose la capture en perspective ; ce qui passe devant l'écran (une tête, une main) reste devant. Les franges vertes sont neutralisées et les reflets verts dans la pièce (table, visages, clavier) prennent la couleur de l'image posée ; les feuillages sont épargnés.
4. **L'export** écrit deux tailles en WebP (1536 et 960 pixels de large) dans `site/assets/img/scenes/` et `site/assets/img/pi/`, une version par langue quand l'écran affiche du texte.

## Refaire

```sh
cd apps
cargo run -p kybernow-linux --no-default-features -- --preview ~/kn-captures/fr fr
cargo run -p kybernow-linux --no-default-features -- --preview ~/kn-captures/en en
cd ..
scripts/site-scenes/scenes.py ~/kn-captures            # COMFY=http://hôte:8188 pour un autre serveur
scripts/site-scenes/scenes.py ~/kn-captures --only canape   # une seule sortie
```

Les pièces générées restent dans `~/kybernow-scenes` (`--work`) : relancer refait seulement la pose et l'export, par exemple après un changement de l'interface. `--regen` régénère les pièces (graines fixes : mêmes images avec le même modèle). Il faut Pillow, numpy et opencv-python, et un ComfyUI avec `z_image_turbo_int8_convrot`, `qwen_3_4b_fp8_mixed` et `ae`.

Les prompts, graines et captures de chaque scène sont dans `scenes.py` (`SCENES`, `OUTPUTS`). Consignes de l'auteur pour les personnes : 30 à 50 ans, hommes et femmes, de dos ou de trois quarts, ambiance chaleureuse du soir, des moments partagés.

L'image de l'allumage de KyberNow Link OS (`site/assets/img/pi/link-os-allumage.webp`) est une vraie capture de l'OS démarré en émulation (`apps/linux/os/emulate.sh`, `emulate/qmp.py shot`).
