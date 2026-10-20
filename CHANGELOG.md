# Versions de KyberNow

Chaque section devient les notes de la version publiée sur GitHub : la
console de l'ordinateur et l'app Mac les affichent avant d'installer.

## 0.2.0

Première version publiée.

**Ordinateur (serveur et console)**
- Installateur clé en main : Kyber 0.29 corrigé, console d'administration, icône de la zone de notification, écran virtuel, manettes, son à distance.
- Kyber corrigé : son HD et multicanal, son continu et à faible latence (paquets de 10 ms), pointeur stable, image clé à la demande d'une app qui a perdu des paquets.
- Console : appairage des apps (Bonjour, QR code, lien), détection des jeux en plein écran, mises à jour en un clic.

**App Mac**
- Image décodée par l'app (VideoToolbox, H.264, HEVC, AV1) et affichée avec Metal : latence minimale, écrans à fréquence variable suivis, 4:4:4.
- Son décodé par l'app (Opus, stéréo, 5.1, 7.1), avec une vingtaine de millisecondes de tampon.
- Définition réelle de l'écran du Mac (4K, 5K…), profils Jeu, Bureau et Max ramenés à ce que la machine décode.
- Souris Auto, Bureau ou Jeu ; presse-papiers partagé ; manette et barre de session ; latence de bout en bout mesurée.
- Mises à jour en un clic (Sparkle).
