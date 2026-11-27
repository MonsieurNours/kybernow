# Traduire le guide en anglais

Le guide français est dans `docs/refonte-site-2026-11/guide/*.md` (27 articles, `faq.md`, `_accueil.md`), au format décrit dans `BRIEF.md`. La version anglaise va dans `docs/refonte-site-2026-11/guide/en/`, **un fichier par article, même nom de fichier** (`apple-tv.md` → `en/apple-tv.md`).

## Règles
- Anglais britannique (« colour », « favourite »…), écrit comme un rédacteur technique anglophone l'écrirait : pas une traduction mot à mot. Même voix que le français (direct, concret, une pointe de légèreté, jargon expliqué), mêmes faits. Lisez la partie anglaise de `docs/ton-du-site.md` (et `docs/ton-du-site.en.md` s'il existe) : pas de « seamless », « effortless », « next-gen », pas d'antithèse « not X, but Y », pas de slogan elliptique.
- **Libellés d'interface** : utilisez les vrais libellés anglais des apps. Ils sont dans `docs/link-ui.en.md` (et `docs/link-ui.md`, qui donne souvent les deux langues), dans les catalogues des apps (`grep -rn` dans `apps/` et `server/` : fichiers `en.json`, `Localizable.strings`, `*.ftl`, `strings.xml`…) et dans l'ancien guide anglais `site/en/guide/index.html`. Si un libellé anglais est introuvable, traduisez-le naturellement et signalez-le dans votre bilan.
- Ne changez aucun fait, chiffre, commande, nom de fichier.
- Typographie anglaise : pas d'espace avant ; : ! ?, guillemets “ ” pour citer un libellé, point décimal (9.4 ms), espace insécable entre nombre et unité.

## En-tête
Même en-tête YAML, traduit, avec en plus `slug_en:` (un slug anglais court et parlant pour l'URL, ex. `apple-tv`, `install-station-windows`, `connect-a-device`, `faq`). Gardez `slug:` (le slug français) tel quel : il sert à relier les deux langues. `rubrique:` reste en français (c'est une clé) ; ajoutez `rubrique_en:` avec le nom anglais de la rubrique (Get started, Install the Station, Connect a device, During a session, Consoles, Share, Accounts and teams, Away from home, Maintenance). `connexes:` garde les slugs français. `title` 60 caractères au plus, `description` 150 au plus.

## Liens
Dans le texte, gardez les liens internes sous la forme `/guide/<slug-français>/` : le générateur les convertit en adresses anglaises.

## Ce que vous rendez
Vos fichiers dans `guide/en/`, puis un bilan de dix lignes au plus : les `slug_en` choisis, les libellés anglais non confirmés.
