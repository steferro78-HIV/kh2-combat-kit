"""Estrae singoli file di KH2 in extracted/kh2 senza l'estrazione completa.

Lo strumento OpenKH estrae solo archivi interi. Qui si copia l'archivio (.pkg) in una
cartella temporanea dentro extracted/ con un indice (.hed) ridotto alle sole voci volute,
e si lancia OpenKH su quella copia. L'installazione del gioco viene solo letta.

Dove stanno il gioco e OpenKH lo dice `local_paths.py`.

Uso: python tools/kh2/extract_files.py 00battle.bin obj/P_EX100.mset
"""
import glob
import hashlib
import os
import shutil
import subprocess
import sys
from pathlib import Path

import local_paths

ROOT = Path(__file__).resolve().parents[2]
TMP = ROOT / "extracted" / "_tmp_pkg"
OUT = ROOT / "extracted" / "kh2"
ENTRY_SIZE = 32  # MD5 del nome (16 byte) + posizione e dimensioni


def main(names):
    game_dt = local_paths.game_data_dir()
    idximg = local_paths.openkh_tool("OpenKh.Command.IdxImg.exe")
    wanted = {hashlib.md5(n.encode()).digest(): n for n in names}
    TMP.mkdir(parents=True, exist_ok=True)
    found = set()
    for hed in sorted(glob.glob(str(game_dt / "kh2_*.hed"))):
        data = Path(hed).read_bytes()
        entries = [data[i:i + ENTRY_SIZE] for i in range(0, len(data), ENTRY_SIZE)]
        keep = [e for e in entries if e[:16] in wanted]
        if not keep:
            continue
        found.update(wanted[e[:16]] for e in keep)
        stem = Path(hed).stem
        (TMP / (stem + ".hed")).write_bytes(b"".join(keep))
        pkg_copy = TMP / (stem + ".pkg")
        if not pkg_copy.exists():
            shutil.copyfile(game_dt / (stem + ".pkg"), pkg_copy)
        subprocess.run([str(idximg), "hed", "extract", str(TMP / (stem + ".hed")), "-o", str(OUT)], check=True)
    missing = set(names) - found
    if missing:
        print("NON TROVATI:", sorted(missing))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
