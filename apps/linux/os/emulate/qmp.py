#!/usr/bin/env python3
# QMP minimal pour l'émulation de KyberNow Link OS (emulate.sh) :
#   qmp.py shot NOM            capture d'écran → DOSSIER/shots/NOM.png
#   qmp.py key TOUCHE…         touches QEMU (ret, up, down, left, right, esc, tab, spc, a, ctrl-c…)
#   qmp.py quit                arrêt de QEMU
# Le dossier de travail est $KYBERNOW_QEMU (celui d'emulate.sh), ou ~/kybernow-os/qemu.
import json, os, socket, sys, time

WORK = os.path.expanduser(os.environ.get("KYBERNOW_QEMU", "~/kybernow-os/qemu"))


def qmp(cmds):
    s = socket.socket(socket.AF_UNIX)
    s.connect(os.path.join(WORK, "qmp.sock"))
    f = s.makefile("rw")
    f.readline()
    f.write(json.dumps({"execute": "qmp_capabilities"}) + "\n")
    f.flush()
    f.readline()
    out = []
    for c in cmds:
        f.write(json.dumps(c) + "\n")
        f.flush()
        while True:
            line = f.readline()
            if not line:
                break
            d = json.loads(line)
            if "return" in d or "error" in d:
                out.append(d)
                break
    s.close()
    return out


op = sys.argv[1] if len(sys.argv) > 1 else "help"
if op == "shot":
    p = os.path.join(WORK, "shots", "%s.png" % sys.argv[2])
    os.makedirs(os.path.dirname(p), exist_ok=True)
    r = qmp([{"execute": "screendump", "arguments": {"filename": p, "format": "png"}}])
    print(p if r and "return" in r[0] else r)
elif op == "key":
    for k in sys.argv[2:]:
        qmp([{"execute": "send-key", "arguments": {"keys": [{"type": "qcode", "data": x} for x in k.split("-")]}}])
        time.sleep(0.15)
    print("ok")
elif op == "quit":
    print(qmp([{"execute": "quit"}]))
else:
    print(__doc__ or "qmp.py shot NOM | key TOUCHE… | quit")
    sys.exit(2)
