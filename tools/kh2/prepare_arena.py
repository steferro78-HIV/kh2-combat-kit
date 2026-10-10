"""Prepara tutto ciò che serve all'arena partendo dalla propria copia di KH2, con un solo comando.

Uso: python tools/kh2/prepare_arena.py

Cosa fa, in ordine, per Sora (P_EX100) e per la Catena Regale (W_EX010):
1. estrae dalla copia installata del gioco il modello (.mdlx) e le animazioni (.mset);
2. costruisce il catalogo dei colpi (lo stesso che carica kh2-combat-core);
3. converte modello e texture per l'arena;
4. fa calcolare a OpenKH le pose di fermo, corsa, salto, caduta e di tutti i colpi del catalogo
   (la prima volta ci vuole qualche minuto; le pose già calcolate non si rifanno);
5. converte le pose per l'arena.

Tutto finisce in extracted/ (mai nel repository). Serve OpenKH installato e la propria copia di
KH2: i percorsi sono scritti in tools/kh2/extract_files.py e tools/kh2/bake_motions.py.
"""
import sys

import bake_motions
import build_catalog
import export_arena_model
import export_arena_poses
import extract_files

MODELS = ("P_EX100", "W_EX010")


def main():
    files = [f"obj/{model}.{kind}" for model in MODELS for kind in ("mdlx", "mset")]
    steps = [("estrazione dal gioco", lambda: extract_files.main(files)),
             ("catalogo dei colpi", build_catalog.main)]
    for model in MODELS:
        # All'arma possono mancare alcune animazioni: non è un errore.
        steps += [
            (f"{model}: modello e texture", lambda model=model: export_arena_model.main(model)),
            (f"{model}: pose delle animazioni",
             lambda model=model: bake_motions.main(model, tuple(export_arena_poses.motions()), required=model == MODELS[0])),
            (f"{model}: pose per l'arena", lambda model=model: export_arena_poses.main(model)),
        ]
    for name, step in steps:
        print(f"\n== {name} ==")
        if step() != 0:
            print(f"FERMO: il passo '{name}' non è riuscito")
            return 1
    print("\nFatto. Avvia l'arena con: cargo run --manifest-path src/Cargo.toml -p arena")
    return 0


if __name__ == "__main__":
    sys.exit(main())
