// La page d'un journal déposé (docs/relais.md, « Dépôt d'un journal ») : la clé
// est dans le fragment de l'adresse (#k=…), que le navigateur n'envoie jamais.
// Le script la retire de l'adresse, reprend le dépôt une fois (GET /v1/journal/<id>),
// le déchiffre (AES-256-GCM, WebCrypto) et le décompresse (gzip), puis propose
// « Enregistrer » et « Partager ». Il ne parle qu'au relais qui sert la page.
(() => {
  "use strict";
  const root = document.getElementById("kj");
  if (!root) return;
  const el = (id) => document.getElementById(id);
  const show = (key) => {
    el("kj-status").hidden = true;
    const box = el("kj-error");
    box.textContent = root.dataset[key] || "";
    box.hidden = false;
  };
  const id = location.pathname.split("/").pop() || "";
  const k = new URLSearchParams(location.hash.slice(1)).get("k") || "";
  // La clé quitte l'adresse affichée et l'historique.
  if (location.hash) history.replaceState(null, "", location.pathname);
  const unb64 = (s) => {
    if (!/^[A-Za-z0-9_-]*$/.test(s)) return null;
    try {
      const bin = atob(s.replace(/-/g, "+").replace(/_/g, "/") + "===".slice((s.length + 3) % 4));
      return Uint8Array.from(bin, (c) => c.charCodeAt(0));
    } catch (_) {
      return null;
    }
  };
  const key = unb64(k);
  if (!key || key.length !== 32 || !/^[A-Za-z0-9_-]{22}$/.test(id)) return show("incomplete");
  if (!window.crypto || !crypto.subtle || typeof DecompressionStream === "undefined") return show("failed");

  // « Pixel 7 · 1,2 Mo · 2026-11-27 12:00 UTC » : l'appareil et la date viennent de l'en-tête du journal.
  const line = (text, size) => {
    const head = text.slice(0, 4096);
    const pick = (re) => (head.match(re) || [])[1] || "";
    const device = pick(/^(?:Appareil|Device)\s*:\s*(.+)$/m).trim();
    const date = pick(/^(?:Journal écrit le|Log written on)\s+(.+)$/m).trim();
    const lang = root.dataset.lang === "fr" ? "fr" : "en";
    const units = lang === "fr" ? ["o", "Ko", "Mo"] : ["B", "KB", "MB"];
    let n = size, u = 0;
    while (n >= 1000 && u < 2) { n /= 1000; u += 1; }
    const amount = new Intl.NumberFormat(lang, { maximumFractionDigits: u ? 1 : 0 }).format(n) + " " + units[u];
    return [device, amount, date].filter(Boolean).join(" · ");
  };

  (async () => {
    let res;
    try {
      res = await fetch("/v1/journal/" + id, { cache: "no-store", credentials: "omit", referrerPolicy: "no-referrer" });
    } catch (_) {
      return show("failed");
    }
    if (res.status === 404) return show("gone");
    if (!res.ok) return show("failed");
    let text;
    try {
      const box = new Uint8Array(await res.arrayBuffer());
      const magic = String.fromCharCode(box[0], box[1], box[2], box[3]);
      if (box.length < 32 || magic !== "KNJ1") return show("failed");
      const aes = await crypto.subtle.importKey("raw", key, "AES-GCM", false, ["decrypt"]);
      const gz = await crypto.subtle.decrypt({ name: "AES-GCM", iv: box.slice(4, 16) }, aes, box.slice(16));
      const plain = new Blob([gz]).stream().pipeThrough(new DecompressionStream("gzip"));
      text = await new Response(plain).text();
    } catch (_) {
      return show("failed");
    }
    const file = new File([text], "kybernow-journal.txt", { type: "text/plain;charset=utf-8" });
    el("kj-line").textContent = line(text, file.size);
    el("kj-save").addEventListener("click", () => {
      const a = document.createElement("a");
      a.href = URL.createObjectURL(file);
      a.download = file.name;
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(a.href), 60000);
    });
    const share = el("kj-share");
    if (navigator.canShare && navigator.canShare({ files: [file] })) {
      share.hidden = false;
      share.addEventListener("click", () => {
        navigator.share({ files: [file], title: file.name }).catch(() => {});
      });
    }
    el("kj-status").hidden = true;
    el("kj-ready").hidden = false;
  })();
})();
