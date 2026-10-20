# Prototype A — résultats

[English](prototype-A-results.en.md) · Français

## Run 1 — référence (2026-10-19 16:41, `tools/latency/runs/20261019-164146`)

**Configuration** : serveur Kyber 0.29.0 (installeur officiel) sur PC Windows NVIDIA `192.168.1.100`, NVENC, h264, débit par défaut. Client `kyclient` 0.29.0 compilé localement, MacBook Pro M4 Max. Réseau : RTT QUIC 0,66 ms (LAN). Écran du PC : **4K à 160 Hz**.

**Conclusion : prototype A validé.** Le serveur et le client fonctionnent de bout en bout, avec une latence conforme aux annonces de Kyber. Ce run sert de référence pour comparer nos propres clients (KyberKit, apps macOS et tvOS). Pas d'autres mesures prévues d'ici là.

| étape | p50 | p95 | p99 | max (ms) |
|---|---|---|---|---|
| encodage (acquired → encoded) | 5,19 | 9,84 | 36,37 | 307,92 |
| envoi (encoded → sent) | 0,12 | 0,44 | 0,94 | 1,42 |
| réseau (sent → received) | 0,66 | 1,30 | 2,01 | 12,84 |
| décodage (received → decoded) | 2,50 | 4,75 | 14,51 | 62,31 |
| affichage (decoded → displayed) | 0,62 | 0,85 | 1,48 | 4,30 |
| **TOTAL (acquired → displayed)** | **9,36** | **17,39** | **53,77** | **316,44** |

3 263 images analysées, 59 fps affichés, aucune image sautée. Recalage d'horloge : dispersion 0,22 ms.

### Lecture
- **La médiane (9,4 ms) correspond aux 8–10 ms annoncés par Kyber.** Le réseau est négligeable en LAN.
- **L'encodage domine** : 5,2 ms en médiane, en 4K. C'est cohérent avec les 2,5 ms cités par Kyber en 1080p, pour quatre fois moins de pixels.
- **Les pics d'encodage suivent les moments où l'écran est resté fixe** : après un écart de 200 ms ou plus sans nouvelle image, l'encodage passe à p95 = 107 ms, contre 9 ms en flux continu. C'est la signature d'un GPU qui baisse ses fréquences au repos puis doit remonter.
- **Les pics de décodage côté Mac arrivent en rafales** (6 à 8 images d'affilée au même instant) : plutôt des images volumineuses (changement de scène, image clé) qu'un problème régulier.
- L'intervalle de capture médian est de 6,3 ms, ce qui correspond à l'écran à 160 Hz.
- La session s'est terminée par « closed by peer » côté serveur : à vérifier si c'était la fermeture volontaire de la fenêtre.
