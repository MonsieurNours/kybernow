# shellcheck shell=bash
# Identité de signature Apple de KyberNow, en un seul endroit (audit TRANS-20) :
# l'équipe (Team ID) et le certificat Developer ID. Lu (source) par
# apps/macos/scripts/sign-release.sh, server/macos/scripts/build-station-app.sh
# et run-kyber.sh, et les build.sh de server/macos (camera-extension, gamepads,
# audio-driver, display-agent). Les projets Xcode (project.yml) nomment
# l'équipe de leur côté ; les scripts de sideload d'iOS et de tvOS la
# remplacent, quelle qu'elle soit, par celle de l'utilisateur.
#
# KYBER_SIGN_IDENTITY : un autre certificat (le nom, ou son empreinte SHA-1).

# shellcheck disable=SC2034  # lus par les scripts qui sourcent ce fichier
{
KYBER_TEAM_ID=P6DB682S4K
KYBER_SIGN_IDENTITY="${KYBER_SIGN_IDENTITY:-Developer ID Application: Mathieu Requenna ($KYBER_TEAM_ID)}"
}
