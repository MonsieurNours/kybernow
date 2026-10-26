//! Textes du côté Rust : `{nom}` remplacés par leur valeur et, en français,
//! « de {nom} » élidé devant une voyelle ou un h (« Mise à jour de la Station
//! d'Atelier », mais « … de Bureau-Salon »), comme `t()` de l'interface
//! (`ui/src/lib/i18n/index.svelte.ts`). Sans rien de Windows : testé sur le Mac.

/// Un nom qui commence par une voyelle (accentuées comprises) ou un h élide « de ».
pub fn elides(name: &str) -> bool {
    name.trim_start()
        .chars()
        .next()
        .and_then(|c| c.to_lowercase().next())
        .is_some_and(|c| "aeiouyhàâäéèêëîïôöùûüÿæœ".contains(c))
}

/// `template` avec ses `{nom}` remplacés ; `french` : élision de « de {nom} »
/// (« de » est un mot entier : « Mode {nom} » ne change pas).
pub fn fill(template: &str, params: &[(&str, &str)], french: bool) -> String {
    let mut text = template.to_string();
    if french {
        for (name, value) in params {
            if !elides(value) {
                continue;
            }
            let from = format!("de {{{name}}}");
            let mut out = String::with_capacity(text.len());
            let mut rest = text.as_str();
            while let Some(at) = rest.find(&from) {
                let before = &rest[..at];
                let word = before.chars().next_back().is_none_or(|c| !c.is_alphanumeric());
                out.push_str(before);
                out.push_str(if word { "d'" } else { "de " });
                out.push_str(&format!("{{{name}}}"));
                rest = &rest[at + from.len()..];
            }
            out.push_str(rest);
            text = out;
        }
    }
    for (name, value) in params {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::fill;

    #[test]
    fn elision_de_de() {
        let t = "Mise à jour de la Station de {name}";
        assert_eq!(fill(t, &[("name", "Atelier")], true), "Mise à jour de la Station d'Atelier");
        assert_eq!(fill(t, &[("name", "Hugo-PC")], true), "Mise à jour de la Station d'Hugo-PC");
        assert_eq!(fill(t, &[("name", "Éole")], true), "Mise à jour de la Station d'Éole");
        assert_eq!(fill(t, &[("name", "Bureau-Salon")], true), "Mise à jour de la Station de Bureau-Salon");
        // En anglais, rien ne s'élide.
        assert_eq!(fill("Station update for {name}", &[("name", "Atelier")], false), "Station update for Atelier");
        // « de » doit être un mot entier.
        assert_eq!(fill("Mode {name}", &[("name", "Atelier")], true), "Mode Atelier");
        assert_eq!(fill("{name} est en ligne", &[("name", "Atelier")], true), "Atelier est en ligne");
    }
}
