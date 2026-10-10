"""Prepara per l'arena un modello di KH2 (forme, texture, pesi delle ossa), leggendolo dal gioco.

Legge extracted/kh2/original/obj/<modello>.mdlx (estratto dalla propria copia di KH2 con
tools/kh2/extract_files.py) e scrive, sempre dentro extracted/ (mai nel repository):
- extracted/kh2/arena/<modello>_model.bin, un file binario semplice che l'arena sa leggere;
- extracted/kh2/arena/<modello>/Texture0000.png, Texture0001.png... le texture.

Nessun passaggio a mano: fino all'8 ottobre 2026 servivano un editor e un'esportazione FBX.

Uso: python tools/kh2/export_arena_model.py P_EX100     (Sora)
     python tools/kh2/export_arena_model.py W_EX010     (Catena Regale)

Formato del file .bin (numeri little-endian):
  "KMDL", numero di ossa, poi per ogni osso la matrice di riposo (16 float, riga per riga,
  con la traslazione nell'ultima riga); numero di parti; per ogni parte: lunghezza e nome della
  texture, numero di vertici, poi per ogni vertice posizione (3 float), normale (3 float),
  uv (2 float), quattro ossa (4 interi a 16 bit) e i loro quattro pesi (4 float).
  I vertici sono a gruppi di tre: un triangolo ogni tre vertici.
"""
import math
import struct
import sys
from collections import defaultdict

import kh2data
import mdlx_model
import mdlx_texture

INFLUENCES = 4


def main(model_name):
    source = kh2data.EXTRACTED / "obj" / f"{model_name}.mdlx"
    if not source.exists():
        print("manca", source, "- lancia prima: python tools/kh2/extract_files.py obj/" + model_name + ".mdlx")
        return 1
    data = source.read_bytes()
    model = mdlx_model.Model(data)
    out_dir = kh2data.EXTRACTED.parent / "arena"
    texture_dir = out_dir / model_name
    texture_dir.mkdir(parents=True, exist_ok=True)

    for index, (width, height, rgba) in enumerate(mdlx_texture.read_textures(data)):
        mdlx_texture.write_png(texture_dir / f"Texture{index:04d}.png", width, height, rgba)
        print(f"texture {index}: {width}x{height}")

    parts = []
    for group in model.groups:
        # Il gioco non salva le normali: si calcolano facendo la media delle facce attorno a
        # ogni punto.
        normals = defaultdict(lambda: [0.0, 0.0, 0.0])
        for a, b, c in group["triangles"]:
            pa, pb, pc = a["position"], b["position"], c["position"]
            u = [pb[i] - pa[i] for i in range(3)]
            v = [pc[i] - pa[i] for i in range(3)]
            face = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
            for corner in (a, b, c):
                for i in range(3):
                    normals[corner["position"]][i] += face[i]

        vertices = bytearray()
        for triangle in group["triangles"]:
            for corner in triangle:
                normal = normals[corner["position"]]
                length = math.sqrt(sum(n * n for n in normal)) or 1.0
                top = sorted(((weight, bone) for bone, weight in corner["weights"]), reverse=True)[:INFLUENCES]
                top += [(0.0, 0)] * (INFLUENCES - len(top))
                total = sum(weight for weight, _bone in top) or 1.0
                vertices += struct.pack("<3f3f2f", *corner["position"], *(n / length for n in normal), *corner["uv"])
                vertices += struct.pack("<4H4f", *(bone for _w, bone in top), *(w / total for w, _b in top))
        texture = f"Texture{group['texture']:04d}.png"
        parts.append((texture, len(group["triangles"]) * 3, vertices))
        print(f"parte con {texture}: {len(group['triangles'])} triangoli")

    out = out_dir / f"{model_name}_model.bin"
    with open(out, "wb") as handle:
        handle.write(b"KMDL" + struct.pack("<I", len(model.bind)))
        for matrix in model.bind:
            handle.write(struct.pack("<16f", *matrix))
        handle.write(struct.pack("<I", len(parts)))
        for texture, count, vertices in parts:
            name = texture.encode("ascii")
            handle.write(struct.pack("<I", len(name)) + name + struct.pack("<I", count) + vertices)
    print("scritto", out, f"({out.stat().st_size // 1024} kB)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "P_EX100"))
