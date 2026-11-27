// Le fuseau du navigateur (audit du 27/11, REL-29) : « jusqu'au JJ/MM » finit à
// minuit chez la personne, pas à minuit UTC. Remplit les champs cachés
// `data-tz` avec le décalage de ce jour-là (en minutes, comme getTimezoneOffset) ;
// il ne charge rien. Sans lui, la fin du jour reste celle d'UTC.
(() => {
  "use strict";
  for (const input of document.querySelectorAll("input[data-tz]")) {
    const form = input.form;
    const fill = () => {
      const date = form && form.querySelector("input[name=fin]");
      const day = date && /^\d{4}-\d{2}-\d{2}$/.test(date.value) ? new Date(date.value + "T23:59:00") : new Date();
      input.value = String(day.getTimezoneOffset());
    };
    fill();
    if (form) form.addEventListener("submit", fill);
  }
})();
