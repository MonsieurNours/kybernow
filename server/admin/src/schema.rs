//! Catalogue des réglages de `kyber_config.toml` exposés dans la console.
//!
//! Chaque entrée décrit où vit la clé, son type, sa valeur par défaut côté Kyber
//! (celle documentée dans le modèle de configuration 0.29) et les textes
//! pédagogiques affichés dans l'interface (résumé, infobulle, conseil).
//!
//! Les réglages du pilote d'écran virtuel « Kidd » de Kyber ne sont pas exposés :
//! ce pilote n'est pas publié et KyberNow gère son propre écran virtuel.

use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
    /// Explication courte affichée dans l'infobulle.
    pub hint: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Kind {
    Bool,
    Int { min: i64, max: i64 },
    Enum { options: &'static [Choice] },
}

#[derive(Debug, Clone, Serialize)]
pub struct Field {
    pub id: &'static str,
    pub group: &'static str,
    #[serde(skip)]
    pub table: &'static [&'static str],
    #[serde(skip)]
    pub key: &'static str,
    pub kind: Kind,
    pub default: Value,
    pub label: &'static str,
    /// Une phrase, affichée sous le libellé.
    pub help: &'static str,
    /// Explication détaillée (infobulle) : ce que fait le réglage.
    pub details: &'static str,
    /// Quand y toucher, ou la valeur conseillée.
    pub advice: &'static str,
    /// Mise en garde éventuelle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caution: Option<&'static str>,
    /// Unité d'affichage (le stockage reste celui de Kyber).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Group {
    pub id: &'static str,
    pub label: &'static str,
    pub help: &'static str,
}

pub const GROUPS: &[Group] = &[
    Group {
        id: "video",
        label: "Image",
        help: "Comment l'écran de l'ordinateur est capturé, compressé puis envoyé. Ces réglages jouent sur la latence, la qualité d'image et la charge du GPU.",
    },
    Group {
        id: "audio",
        label: "Son",
        help: "Comment le son de l'ordinateur est capturé puis envoyé. Le son suit la sortie audio de Windows : stéréo, 5.1 ou 7.1.",
    },
    Group {
        id: "network",
        label: "Réseau",
        help: "Comment vos appareils joignent cet ordinateur. En général, rien à changer : l'installateur a déjà choisi un port libre.",
    },
    Group {
        id: "input",
        label: "Clavier, manettes et presse-papiers",
        help: "Ce que vos appareils ont le droit de faire sur cet ordinateur à distance.",
    },
    Group {
        id: "auth",
        label: "Connexion",
        help: "Les façons de prouver qui vous êtes avant d'accéder au stream. Les comptes eux-mêmes se gèrent dans l'onglet Utilisateurs.",
    },
    Group {
        id: "logs",
        label: "Journaux",
        help: "La quantité de détails écrits dans les journaux. Utile surtout pour diagnostiquer un problème.",
    },
];

const LOG_LEVELS: &[Choice] = &[
    Choice { value: "error", label: "Erreurs seulement", hint: "Le strict minimum : uniquement ce qui empêche quelque chose de fonctionner." },
    Choice { value: "warn", label: "Avertissements", hint: "Erreurs et situations anormales mais non bloquantes." },
    Choice { value: "info", label: "Normal", hint: "Les événements importants : connexions, démarrages, arrêts. Le bon réglage au quotidien." },
    Choice { value: "debug", label: "Détaillé", hint: "Beaucoup plus d'informations, pour comprendre un problème précis." },
    Choice { value: "trace", label: "Très détaillé", hint: "Tout, y compris chaque image : journaux volumineux, à n'activer que le temps d'un diagnostic." },
    Choice { value: "off", label: "Désactivé", hint: "Aucun journal. Déconseillé : en cas de problème, il n'y aura rien à lire." },
];

pub fn fields() -> Vec<Field> {
    vec![
        Field {
            id: "video.encoder", group: "video",
            table: &["kyavserver"], key: "encoder",
            kind: Kind::Enum { options: &[
                Choice { value: "nvenc", label: "NVENC — carte NVIDIA", hint: "La puce d'encodage dédiée des cartes NVIDIA. Très rapide, et n'enlève presque rien aux performances des jeux." },
                Choice { value: "x264", label: "x264 — processeur", hint: "Encodage par le processeur. Fonctionne partout, mais charge le CPU et ajoute de la latence." },
                Choice { value: "amf", label: "AMF — carte AMD (expérimental)", hint: "L'équivalent de NVENC chez AMD. Encore expérimental dans Kyber." },
                Choice { value: "qsv", label: "Quick Sync — Intel (expérimental)", hint: "L'encodeur des processeurs et cartes Intel. Encore expérimental dans Kyber." },
            ]},
            default: json!("nvenc"),
            label: "Encodeur vidéo",
            help: "La puce ou le programme qui compresse chaque image avant de l'envoyer.",
            details: "Une image 4K brute pèse environ 25 Mo : impossible de l'envoyer telle quelle 60 fois par seconde. L'encodeur la compresse en quelques millisecondes. Un encodeur matériel (NVENC) le fait sans ralentir le reste de l'ordinateur.",
            advice: "Avec une carte NVIDIA, gardez NVENC. Essayez x264 seulement si l'image est corrompue.",
            caution: None,
            unit: None,
        },
        Field {
            id: "video.backend", group: "video",
            table: &["kyavserver"], key: "backend",
            kind: Kind::Enum { options: &[
                Choice { value: "txproto", label: "Historique (txproto)", hint: "Le moteur utilisé par défaut, éprouvé." },
                Choice { value: "libavconv", label: "Nouveau (libavconv)", hint: "Le nouveau moteur de Kyber, en cours de déploiement. Peut améliorer la latence, peut aussi être moins stable." },
            ]},
            default: json!("txproto"),
            label: "Moteur de capture",
            help: "Le module de Kyber qui capture l'écran et le son puis pilote l'encodeur.",
            details: "Kyber est en train de remplacer son moteur de capture. Les deux coexistent dans la version 0.29 : l'historique est le plus sûr, le nouveau reçoit les dernières optimisations.",
            advice: "Gardez le moteur historique, sauf pour tester le nouveau si vous rencontrez un problème de capture.",
            caution: None,
            unit: None,
        },
        Field {
            id: "video.minimum_bitrate", group: "video",
            table: &["kyavserver"], key: "minimum_bitrate",
            kind: Kind::Int { min: 500_000, max: 500_000_000 },
            default: json!(5_000_000),
            label: "Débit minimum",
            help: "Le plancher sous lequel la qualité d'image ne descendra jamais.",
            details: "Le débit, c'est la quantité de données envoyées chaque seconde : plus il est élevé, plus l'image est nette. Kyber l'ajuste en permanence selon le réseau, mais ne descendra jamais sous ce plancher.",
            advice: "5 Mbit/s convient au Wi-Fi. En Ethernet, 15 à 30 Mbit/s évitent les flous dans les scènes rapides.",
            caution: Some("Un plancher trop haut sur un réseau lent provoque des saccades au lieu d'une image simplement plus floue."),
            unit: Some("bps"),
        },
        Field {
            id: "audio.quality", group: "audio",
            table: &["kyavserver"], key: "audio_quality",
            kind: Kind::Enum { options: &[
                Choice { value: "standard", label: "Standard", hint: "128 kbit/s, quel que soit le nombre de canaux : la valeur d'origine de Kyber." },
                Choice { value: "hd", label: "HD", hint: "256 kbit/s en stéréo, 576 en 5.1, 768 en 7.1 : indiscernable de l'original." },
                Choice { value: "studio", label: "Studio", hint: "Le maximum d'Opus : 510 kbit/s en stéréo, 1 536 en 5.1, 2 048 en 7.1." },
            ] },
            default: json!("hd"),
            label: "Qualité du son",
            help: "Le débit du son, adapté au nombre de canaux de la sortie de Windows.",
            details: "Le son est compressé en Opus, le format le plus efficace aujourd'hui. Son débit grandit avec le nombre de canaux : une sortie Windows réglée en 5.1 ou 7.1 envoie tous ses canaux, et l'appareil qui reçoit les réduit si sa propre sortie en a moins.",
            advice: "HD convient à tout : à ce débit, on ne distingue plus le son compressé de l'original. Studio sert surtout à se rassurer, il ne coûte presque rien face à l'image.",
            caution: None,
            unit: None,
        },
        Field {
            id: "video.intra_refresh", group: "video",
            table: &["kyavserver"], key: "intra_refresh",
            kind: Kind::Bool,
            default: json!(true),
            label: "Rafraîchissement progressif",
            help: "Lisse les pics de débit en rafraîchissant l'image par bandes.",
            details: "Régulièrement, l'encodeur doit envoyer une image complète, bien plus lourde que les autres : c'est un pic de débit qui peut provoquer une micro-saccade. Avec ce réglage, l'image complète est envoyée par bandes, réparties sur plusieurs images.",
            advice: "Laissez activé. Il ne fonctionne qu'avec l'encodeur NVENC.",
            caution: None,
            unit: None,
        },
        Field {
            id: "network.port", group: "network",
            table: &["kycontroller"], key: "port",
            kind: Kind::Int { min: 1024, max: 65535 },
            default: json!(8080),
            label: "Port du stream",
            help: "La « porte » réseau par laquelle vos appareils se connectent, en TCP et en UDP.",
            details: "Un ordinateur peut héberger plusieurs services ; chacun écoute sur un numéro de port différent. L'installateur en a tiré un au hasard et vérifié qu'il était libre. Vos appareils le découvrent seuls sur le réseau local.",
            advice: "Ne le changez que si un autre logiciel a besoin de ce numéro.",
            caution: Some("Après un changement, le pare-feu Windows doit autoriser le nouveau port (TCP et UDP), et les appareils qui le connaissaient devront le mettre à jour."),
            unit: None,
        },
        Field {
            id: "network.listen_mode", group: "network",
            table: &["kycontroller"], key: "listen_mode",
            kind: Kind::Enum { options: &[
                Choice { value: "dual_stack", label: "IPv4 et IPv6", hint: "Accepte les deux types d'adresses réseau. Le choix le plus compatible." },
                Choice { value: "ipv4", label: "IPv4 seulement", hint: "Pour les réseaux où IPv6 pose problème." },
            ]},
            default: json!("dual_stack"),
            label: "Adresses acceptées",
            help: "Les versions du protocole Internet sur lesquelles le serveur écoute.",
            details: "Les appareils ont souvent deux adresses : une IPv4 (comme 192.168.1.100) et une IPv6, plus longue. Écouter sur les deux garantit que tous peuvent se connecter.",
            advice: "Gardez IPv4 et IPv6, sauf problème de connexion inexpliqué.",
            caution: None,
            unit: None,
        },
        Field {
            id: "network.multi_client", group: "network",
            table: &["kycontroller"], key: "multi_client",
            kind: Kind::Bool,
            default: json!(true),
            label: "Plusieurs appareils en même temps",
            help: "Autorise plusieurs appareils à regarder le même écran simultanément.",
            details: "Activé, l'écran est encodé une seule fois puis partagé entre tous les appareils connectés. Désactivé, un seul appareil à la fois, mais avec un encodage qui lui est entièrement dédié : c'est la latence la plus basse possible.",
            advice: "Pour jouer, désactivez-le. Pour montrer votre écran à plusieurs personnes, laissez-le activé.",
            caution: None,
            unit: None,
        },
        Field {
            id: "network.tray", group: "network",
            table: &["kycontroller"], key: "tray",
            kind: Kind::Bool,
            default: json!(true),
            label: "Icône près de l'horloge",
            help: "Affiche l'état du serveur dans la zone de notification de Windows.",
            details: "Une petite icône indique si le serveur tourne et si un appareil est connecté. Pratique pour savoir d'un coup d'œil que quelqu'un utilise l'ordinateur à distance.",
            advice: "Laissez-la : elle signale aussi une connexion en cours aux personnes devant l'ordinateur.",
            caution: None,
            unit: None,
        },
        Field {
            id: "input.gamepad_force_xbox360", group: "input",
            table: &["kynputserver"], key: "gamepad_force_xbox360",
            kind: Kind::Bool,
            default: json!(false),
            label: "Toutes les manettes en Xbox 360",
            help: "Présente chaque manette à l'ordinateur comme une manette Xbox 360.",
            details: "Par défaut, une manette PlayStation reste une manette PlayStation pour l'ordinateur. Certains jeux anciens ne reconnaissent que les manettes Xbox 360 : ce réglage les fait passer pour telles.",
            advice: "Activez-le seulement si un jeu ne détecte pas votre manette.",
            caution: Some("Nécessite le pilote ViGEmBus, installé avec KyberNow."),
            unit: None,
        },
        Field {
            id: "input.clipboard_allow_copy", group: "input",
            table: &["kynputserver"], key: "clipboard_allow_copy",
            kind: Kind::Bool,
            default: json!(true),
            label: "Copier de l'ordinateur vers l'appareil",
            help: "Ce que vous copiez sur l'ordinateur peut être collé sur votre Mac ou votre appareil.",
            details: "Le presse-papiers de l'ordinateur est transmis à l'appareil connecté : un texte copié dans une fenêtre de l'ordinateur peut être collé directement sur le Mac.",
            advice: "Pratique pour le travail. Désactivez-le si d'autres personnes se connectent à cet ordinateur.",
            caution: None,
            unit: None,
        },
        Field {
            id: "input.clipboard_allow_paste", group: "input",
            table: &["kynputserver"], key: "clipboard_allow_paste",
            kind: Kind::Bool,
            default: true.into(),
            label: "Coller de l'appareil vers l'ordinateur",
            help: "Ce que vous copiez sur votre appareil peut être collé sur l'ordinateur.",
            details: "Le presse-papiers de l'appareil connecté est transmis à l'ordinateur : un lien copié sur le Mac peut être collé dans le navigateur de l'ordinateur.",
            advice: "Pratique pour le travail. Désactivez-le si vous ne faites pas confiance à tous les appareils connectés.",
            caution: None,
            unit: None,
        },
        Field {
            id: "input.keyboard_key_repeat", group: "input",
            table: &["kynputserver"], key: "keyboard_key_repeat",
            kind: Kind::Bool,
            default: json!(true),
            label: "Répétition des touches",
            help: "Une touche maintenue enfoncée se répète, comme sur un clavier branché à l'ordinateur.",
            details: "Quand vous maintenez une touche, l'ordinateur la répète (« aaaaa ») après un court délai. Certains jeux préfèrent ne recevoir qu'un seul appui maintenu.",
            advice: "Laissez activé, sauf si un jeu réagit bizarrement aux touches maintenues.",
            caution: None,
            unit: None,
        },
        Field {
            id: "auth.basic_enabled", group: "auth",
            table: &["kycontroller", "auth", "basic"], key: "enabled",
            kind: Kind::Bool,
            default: json!(true),
            label: "Identifiant et mot de passe",
            help: "Connexion avec les comptes définis dans l'onglet Utilisateurs.",
            details: "La méthode classique : chaque appareil saisit un identifiant et un mot de passe. Le mot de passe n'est jamais stocké en clair sur l'ordinateur, seulement son empreinte.",
            advice: "Laissez activé : c'est la méthode utilisée par le client web et le client en ligne de commande.",
            caution: Some("Si vous la désactivez sans autre méthode active, plus personne ne pourra se connecter au stream."),
            unit: None,
        },
        Field {
            id: "auth.jwt_enabled", group: "auth",
            table: &["kycontroller", "auth", "jwt"], key: "enabled",
            kind: Kind::Bool,
            default: json!(true),
            label: "Jetons d'appareil",
            help: "Connexion par jeton signé, sans retaper de mot de passe.",
            details: "Un jeton est une sorte de badge numérique signé par cet ordinateur. Les apps KyberNow pour Mac et Apple TV s'en serviront pour se reconnecter sans vous redemander le mot de passe. L'installateur a remplacé la clé de démonstration de Kyber par un secret propre à cet ordinateur.",
            advice: "Laissez activé pour les apps Mac et Apple TV.",
            caution: None,
            unit: None,
        },
        Field {
            id: "logs.controller", group: "logs",
            table: &["kycontroller"], key: "log_level",
            kind: Kind::Enum { options: LOG_LEVELS },
            default: json!("info"),
            label: "Connexions",
            help: "Journal du contrôleur : connexions, authentification, sessions.",
            details: "Le contrôleur est la partie de Kyber qui accueille les appareils, vérifie leur identité et démarre les sessions.",
            advice: "Normal au quotidien ; Détaillé si un appareil n'arrive pas à se connecter.",
            caution: None,
            unit: None,
        },
        Field {
            id: "logs.avserver", group: "logs",
            table: &["kyavserver"], key: "log_level",
            kind: Kind::Enum { options: LOG_LEVELS },
            default: json!("info"),
            label: "Image et son",
            help: "Journal de la capture et de l'encodage.",
            details: "Le serveur audio/vidéo capture l'écran et le son, puis les envoie à l'encodeur.",
            advice: "Normal au quotidien ; Détaillé en cas d'image noire, figée ou corrompue.",
            caution: None,
            unit: None,
        },
        Field {
            id: "logs.inputserver", group: "logs",
            table: &["kynputserver"], key: "log_level",
            kind: Kind::Enum { options: LOG_LEVELS },
            default: json!("info"),
            label: "Clavier et manettes",
            help: "Journal des entrées : clavier, souris, manettes, presse-papiers.",
            details: "Le serveur d'entrées rejoue sur l'ordinateur les touches, mouvements de souris et boutons de manette reçus des appareils.",
            advice: "Normal au quotidien ; Détaillé si une touche ou une manette ne répond pas.",
            caution: None,
            unit: None,
        },
    ]
}

impl Field {
    /// Vérifie qu'une valeur envoyée par l'interface est acceptable pour ce champ.
    pub fn validate(&self, value: &Value) -> Result<(), String> {
        match (self.kind, value) {
            (Kind::Bool, Value::Bool(_)) => Ok(()),
            (Kind::Int { min, max }, Value::Number(n)) => match n.as_i64() {
                Some(v) if (min..=max).contains(&v) => Ok(()),
                _ => Err(format!("{} : valeur attendue entre {min} et {max}", self.label)),
            },
            (Kind::Enum { options }, Value::String(s)) if options.iter().any(|o| o.value == s) => Ok(()),
            (Kind::Enum { options }, _) => Err(format!(
                "{} : valeurs possibles {}",
                self.label,
                options.iter().map(|o| o.value).collect::<Vec<_>>().join(", ")
            )),
            _ => Err(format!("{} : type de valeur invalide", self.label)),
        }
    }
}
