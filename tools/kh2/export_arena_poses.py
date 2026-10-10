"""Prepara per l'arena lo scheletro di un modello di KH2 e le pose delle sue animazioni a terra.

Legge la tabella delle ossa dal modello (obj/<modello>.mdlx, formato da OpenKH:
OpenKh.Kh2/Models/ModelSkeletal.cs e ModelCommon.cs, Apache-2.0) e le pose calcolate da
tools/kh2/bake_motions.py, e scrive un file binario semplice che l'arena sa leggere:
extracted/kh2/arena/<modello>_ground.bin (mai nel repository).

Quali animazioni: fermo, corsa, salto, caduta e tutti i colpi del catalogo
(tools/kh2/build_catalog.py). Nel file ogni animazione ha per nome il suo numero in KH2.

Formato (numeri little-endian): "KPOS", numero di ossa, il genitore di ogni osso (intero a 32
bit, -1 per la radice), numero di animazioni; per ogni animazione: lunghezza e nome, numero di
pose, poi per ogni posa e per ogni osso la sua matrice assoluta (16 float, riga per riga, con
la traslazione nell'ultima riga). C'è una posa per ogni tick di gioco (60 al secondo).

Le pose di un'arma sono relative all'osso della mano che la tiene.

Uso: python tools/kh2/prepare_arena.py    (fa tutto, compreso il calcolo delle pose)
     python tools/kh2/export_arena_poses.py P_EX100     (solo questo passo, per Sora)
"""
import re
import struct
import sys

import catalog
import kh2data

MODEL_ENTRY_TYPE = 4
RESERVED = 0x90   # spazio che il gioco usa a runtime, prima dell'intestazione del modello
BONE_SIZE = 0x40


def motions():
    """Nome dell'animazione nei file di KH2 -> (nome nel file delle pose, cioè il suo numero; durata in tick)."""
    data = catalog.read()
    return {data["motions"][number]["name"]: (str(number), data["motions"][number]["frames"])
            for number in catalog.shown_motions(data)}


def read_parents(mdlx):
    for kind, _name, offset, _size in kh2data.read_bar(mdlx):
        if kind == MODEL_ENTRY_TYPE:
            header = offset + RESERVED
            bone_count, _textures, bone_offset = struct.unpack_from("<HHI", mdlx, header + 0x10)
            parents = []
            for i in range(bone_count):
                index, _sibling, parent, _child = struct.unpack_from("<hhhh", mdlx, header + bone_offset + i * BONE_SIZE)
                if index != i:
                    raise ValueError(f"tabella delle ossa inattesa: osso {i} ha indice {index}")
                parents.append(parent)
            return parents
    raise ValueError("nessun modello nel file")


def read_matrices(xml, bone_count):
    """Per ogni posa, i 16 numeri della matrice di ogni osso, tutti di fila."""
    frames = []
    for matrices in re.findall(r"<Matrices>([^<]*)</Matrices>", xml):
        bones = matrices.split(",")[:bone_count]
        frames.append([float(v) for bone in bones for v in bone.split()])
    return frames


def main(model):
    obj = kh2data.EXTRACTED / "obj"
    baked = kh2data.EXTRACTED.parent / "baked" / model
    out_dir = kh2data.EXTRACTED.parent / "arena"
    out_dir.mkdir(parents=True, exist_ok=True)
    parents = read_parents((obj / f"{model}.mdlx").read_bytes())

    available = {source: name for source, name in motions().items() if (baked / f"{source}.xml").exists()}
    if not available:
        print("nessuna posa calcolata in", baked, "- lancia prima prepare_arena.py")
        return 1
    chunks = [b"KPOS", struct.pack("<I", len(parents)), struct.pack(f"<{len(parents)}i", *parents),
              struct.pack("<I", len(available))]
    for source, (name, duration) in available.items():
        path = baked / f"{source}.xml"
        frames = read_matrices(path.read_text(encoding="utf-8"), len(parents))
        # Le animazioni che non partono dal frame zero escono con delle pose in più all'inizio
        # (il tratto della linea del tempo che il gioco non usa): valgono le ultime.
        frames = frames[-duration:]
        encoded = name.encode("ascii")
        chunks.append(struct.pack("<I", len(encoded)) + encoded + struct.pack("<I", len(frames)))
        for frame in frames:
            chunks.append(struct.pack(f"<{len(frame)}f", *frame))
    out = out_dir / f"{model}_ground.bin"
    out.write_bytes(b"".join(chunks))
    print("scritto", out, f"({len(available)} animazioni, {out.stat().st_size // 1024} kB)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "P_EX100"))
