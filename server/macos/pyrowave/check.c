/*
 * kybernow-pyrowave-check : KyberNow Admin le lance une fois, en fond, au
 * démarrage d'une Station Mac (server/admin/src/pyrowave.rs), comme
 * pyrowave-device-validation.exe sous Windows : la puce convient-elle, et
 * l'encodeur PyroWave démarre-t-il (aller-retour d'une mire) ? Processus à
 * part : un plantage n'emporte pas Admin.
 *
 * Code de sortie : 0 bon, 2 puce qui ne convient pas, 4 encodeur qui ne
 * démarre pas, 5 aller-retour faux. Une ligne d'explication sur la sortie.
 */
#include <stdio.h>

#include "kn_pyrowave_metal.h"

int main(void)
{
    char msg[512] = { 0 };
    int code = kn_pw_selftest(msg, sizeof(msg));
    printf("%s\n", msg);
    return code;
}
