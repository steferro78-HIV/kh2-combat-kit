"""Fa calcolare a OpenKH le pose (matrici delle ossa) di singole animazioni di KH2.

Lo strumento OpenKH.Research.Kh2Anim calcola tutte le animazioni di un file .mset, che per
Sora vuol dire decine di GB. Qui si ritagliano dal .mset solo le animazioni volute, ognuna in
un file .anb a sé, e si fa calcolare solo quelle. Tutto resta in extracted/ (mai nel repository).

Uso: python tools/kh2/bake_motions.py P_EX100 A000 A002 A300 A301 A302 A303 A304
     python tools/kh2/bake_motions.py W_EX010 A000 A002 A300 A301 A302 A303 A304
Risultato: extracted/kh2/baked/<modello>/<nome>.xml, una matrice 4x4 per osso per ogni tick di
gioco (60 al secondo): la prima posa è il tick 1, l'ultima è la fine dell'animazione.

ATTENZIONE, misurato l'8 ottobre 2026: lo strumento, lasciato fare, calcola solo la PRIMA METÀ
di ogni animazione. Il tempo che passa al calcolo è in tick di gioco, ma si ferma al valore
"fine" scritto nell'animazione, che è in frame da un trentesimo di secondo: la metà dei tick.
Per questo qui gli si dà una copia dell'animazione con quel valore raddoppiato. Verifiche fatte:
le pose dei tick in comune restano identiche; la corsa e il fermo si chiudono su sé stessi; il
colpo A avanza di 151 unità contro le 150 misurate nel gioco.
"""
import os
import shutil
import struct
import subprocess
import sys
from pathlib import Path

import kh2data
import local_paths

MOTION_ENTRY_TYPE = 0x09
RESERVED = 0x90        # spazio che il gioco usa a runtime, prima dell'intestazione
TOTAL_TICKS_OFFSET = 0x14  # intero: durata in tick di gioco
FRAME_START_OFFSET = 0x90  # due float: "inizio" e "fine" in frame da un trentesimo (formato da OpenKH, AnimReader.cs)
# Lo strumento chiama "fps" il numero di pose per unità di tempo: 1 = una posa per tick.
SAMPLES_PER_TICK = 1
# Senza questo, su un Windows in italiano i numeri escono con la virgola decimale, che si
# confonde con la virgola che separa una matrice dall'altra.
ENVIRONMENT = dict(os.environ, DOTNET_SYSTEM_GLOBALIZATION_INVARIANT="1")


def whole_length_copy(anb):
    """Copia dell'animazione in cui "inizio" e "fine" sono scritti in tick invece che in trentesimi."""
    patched = bytearray(anb)
    for kind, _name, offset, _size in kh2data.read_bar(anb):
        if kind == MOTION_ENTRY_TYPE:
            header = offset + RESERVED
            total_ticks = struct.unpack_from("<i", anb, header + TOTAL_TICKS_OFFSET)[0]
            frame_start, frame_end = struct.unpack_from("<2f", anb, header + FRAME_START_OFFSET)
            if (frame_end - frame_start) * 2 != total_ticks:
                raise ValueError(f"durata inattesa: da {frame_start} a {frame_end}, tick {total_ticks}")
            # Quasi tutte le animazioni partono da zero; alcune (salto, atterraggio, Vicinity
            # Break) usano solo un tratto della loro linea del tempo.
            struct.pack_into("<2f", patched, header + FRAME_START_OFFSET, frame_start * 2, frame_end * 2)
            return bytes(patched), total_ticks
    raise ValueError("nessuna animazione nel file")


def main(model, names, required=True):
    obj = kh2data.EXTRACTED / "obj"
    mset = (obj / f"{model}.mset").read_bytes()
    anb_dir = kh2data.EXTRACTED.parent / "anb" / model
    out_dir = kh2data.EXTRACTED.parent / "baked" / model
    anb_dir.mkdir(parents=True, exist_ok=True)
    out_dir.mkdir(parents=True, exist_ok=True)
    wanted = set(names)
    for _kind, name, offset, size in kh2data.read_bar(mset):
        # Lo stesso nome compare più volte: vale la prima voce con dati, che è la variante
        # usata in battaglia (nei log di KH2 la variante in battaglia è sempre la 0).
        if name not in wanted or size == 0:
            continue
        wanted.discard(name)
        if (out_dir / f"{name}.xml").exists():
            continue  # già calcolata: le pose non cambiano
        anb = mset[offset:offset + size]
        (anb_dir / f"{name}.anb").write_bytes(anb)
        try:
            patched, total_ticks = whole_length_copy(anb)
        except ValueError as problem:
            # Animazioni che non partono dal frame zero (visto: il salto): il rimedio qui sopra
            # non vale per loro. Si saltano: chi disegna userà un'altra posa.
            print(f"{name}: saltata ({problem})")
            continue
        whole = anb_dir / f"{name}.durata-intera.anb"
        whole.write_bytes(patched)
        work = out_dir / f"_{name}"
        subprocess.run([str(local_paths.openkh_tool("OpenKh.Research.Kh2Anim.exe")), "bake", str(obj / f"{model}.mdlx"), str(whole), str(work), "--fps",
                        str(SAMPLES_PER_TICK)], check=True, env=ENVIRONMENT, stdout=subprocess.DEVNULL)
        shutil.move(str(work / "0.xml"), str(out_dir / f"{name}.xml"))
        work.rmdir()
        print(f"{name}: {total_ticks} tick -> {out_dir / (name + '.xml')}")
    if wanted:
        print("NON TROVATE:", sorted(wanted))
        return 1 if required else 0
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2:]))
