// Les passkeys des pages /compte (docs/comptes.md, lot 1b) : le seul script du
// relais. Il ne charge rien et ne parle qu'au navigateur (navigator.credentials) ;
// la réponse part par le formulaire de la page, comme le reste. Sans lui, les
// formulaires des passkeys restent cachés et tout marche avec le mot de passe.
(() => {
  "use strict";
  if (!window.PublicKeyCredential || !navigator.credentials) return;

  const toB64 = (buf) =>
    btoa(String.fromCharCode(...new Uint8Array(buf))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  const fromB64 = (s) => Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0));
  const ids = (list) => (list ? list.split(",") : []).map((id) => ({ type: "public-key", id: fromB64(id) }));

  const create = async (d) => {
    const cred = await navigator.credentials.create({
      publicKey: {
        rp: { name: d.rpName },
        user: { id: fromB64(d.userId), name: d.userName, displayName: d.userDisplay },
        challenge: fromB64(d.challenge),
        // Ed25519, ES256, puis RS256 (anciennes puces de Windows Hello).
        pubKeyCredParams: [-8, -7, -257].map((alg) => ({ type: "public-key", alg })),
        authenticatorSelection: { residentKey: "required", requireResidentKey: true, userVerification: "required" },
        attestation: "none",
        excludeCredentials: ids(d.exclude),
        timeout: 300000,
      },
    });
    const r = cred.response;
    return {
      client_data: toB64(r.clientDataJSON),
      attestation: toB64(r.attestationObject),
      transports: (r.getTransports ? r.getTransports() : []).join(","),
    };
  };

  const get = async (d) => {
    const cred = await navigator.credentials.get({
      publicKey: { challenge: fromB64(d.challenge), allowCredentials: ids(d.allow), userVerification: "required", timeout: 300000 },
    });
    const r = cred.response;
    return {
      credential: toB64(cred.rawId),
      client_data: toB64(r.clientDataJSON),
      authenticator: toB64(r.authenticatorData),
      signature: toB64(r.signature),
      user: r.userHandle ? toB64(r.userHandle) : "",
    };
  };

  for (const el of document.querySelectorAll("[data-passkey-none]")) el.hidden = true;
  for (const el of document.querySelectorAll("[data-passkey-or]")) el.hidden = false;
  for (const form of document.querySelectorAll("form[data-passkey]")) {
    form.hidden = false;
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const error = form.querySelector(".passkey-error");
      const button = form.querySelector("button");
      if (error) error.hidden = true;
      if (button) button.disabled = true;
      try {
        const fields = form.dataset.passkey === "create" ? await create(form.dataset) : await get(form.dataset);
        for (const [name, value] of Object.entries(fields)) {
          const input = document.createElement("input");
          input.type = "hidden";
          input.name = name;
          input.value = value;
          form.append(input);
        }
        form.submit();
      } catch (e) {
        if (error) error.hidden = false;
        if (button) button.disabled = false;
      }
    });
  }
})();
