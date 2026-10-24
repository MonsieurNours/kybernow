# Site scene photos

English · [Français](README.md)

The home page photos (the hero's living room, the four "Everyday use" scenes, the Raspberry Pi living room) are AI-generated rooms into which **real KyberNow Link captures** are placed. The people and rooms are invented; the interfaces on the screens are the app's own, pixel for pixel.

## How

1. **The room** is generated with Z-Image Turbo on ComfyUI (`zimage.py`, settings taken from the "Image" workflow: 8 steps, `res_multistep`, CFG 1, AuraFlow 3). The screen (TV, Steam Deck, laptop) is asked for in flat chroma green.
2. **The capture** comes from the Linux app, rendered by `kybernow --preview` in French and English: living-room home (`salon-01-accueil`), Steam Deck home (`deck-01-accueil`), desktop home (`bureau-01-accueil`), KyberNow Link OS home (`os-10b-accueil-a-jour`). Screens shown "mid-game" carry a generic game, also generated.
3. **Placing** (`composite.py`) finds the green, its four corners, and places the capture in perspective; anything in front of the screen (a head, a hand) stays in front. Green fringes are neutralised and green reflections in the room (table, faces, keyboard) take the colour of the placed picture; foliage is left alone.
4. **Export** writes two WebP sizes (1536 and 960 pixels wide) into `site/assets/img/scenes/` and `site/assets/img/pi/`, one version per language when the screen shows text.

## Rebuilding

```sh
cd apps
cargo run -p kybernow-linux --no-default-features -- --preview ~/kn-captures/fr fr
cargo run -p kybernow-linux --no-default-features -- --preview ~/kn-captures/en en
cd ..
scripts/site-scenes/scenes.py ~/kn-captures            # COMFY=http://host:8188 for another server
scripts/site-scenes/scenes.py ~/kn-captures --only canape   # a single output
```

Generated rooms stay in `~/kybernow-scenes` (`--work`): running again only redoes placing and export, for instance after an interface change. `--regen` regenerates the rooms (fixed seeds: same images with the same model). You need Pillow, numpy and opencv-python, and a ComfyUI with `z_image_turbo_int8_convrot`, `qwen_3_4b_fp8_mixed` and `ae`.

Each scene's prompt, seed and capture are in `scenes.py` (`SCENES`, `OUTPUTS`). The author's guidance for people: 30 to 50 years old, women and men, seen from behind or three-quarter, a warm evening mood, shared moments.

The KyberNow Link OS start-up image (`site/assets/img/pi/link-os-allumage.webp`) is a real capture of the OS booted under emulation (`apps/linux/os/emulate.sh`, `emulate/qmp.py shot`).
