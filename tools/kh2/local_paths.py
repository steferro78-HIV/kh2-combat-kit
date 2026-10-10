"""Dove stanno, su questo PC, il gioco e gli strumenti di OpenKH.

Ogni PC è diverso, quindi i percorsi non sono scritti negli strumenti. Si cercano, in ordine:

1. nelle variabili d'ambiente `KH2_GAME_DIR` e `OPENKH_DIR`;
2. nel file `extracted/local_paths.json` (mai nel repository), fatto così:
   {"kh2_game_dir": "...", "openkh_dir": "..."}
3. per il gioco soltanto, nella cartella in cui Steam lo installa di solito.

`KH2_GAME_DIR` è la cartella di "KINGDOM HEARTS -HD 1.5+2.5 ReMIX-" (quella che contiene
`Image`). `OPENKH_DIR` è la cartella di OpenKH che contiene i programmi `OpenKh.Command.*.exe`
(https://github.com/OpenKH/OpenKh). L'installazione del gioco viene solo letta.
"""
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LOCAL_FILE = ROOT / "extracted" / "local_paths.json"
STEAM_DEFAULT = Path(r"C:\Program Files (x86)\Steam\steamapps\common\KINGDOM HEARTS -HD 1.5+2.5 ReMIX-")


def _configured(variable, key):
    value = os.environ.get(variable)
    if not value and LOCAL_FILE.exists():
        value = json.loads(LOCAL_FILE.read_text(encoding="utf-8")).get(key)
    return Path(value) if value else None


def _stop(what, variable, key):
    sys.exit(f"Non trovo {what}.\nImposta la variabile d'ambiente {variable}, oppure scrivi in\n"
             f"{LOCAL_FILE}\nuna riga come: {{\"{key}\": \"percorso della cartella\"}}")


def game_data_dir():
    """La cartella con gli archivi di KH2 (kh2_*.hed e kh2_*.pkg)."""
    game = _configured("KH2_GAME_DIR", "kh2_game_dir") or STEAM_DEFAULT
    data = game / "Image" / "dt"
    if not data.is_dir():
        _stop(f"gli archivi di KH2 in {data}", "KH2_GAME_DIR", "kh2_game_dir")
    return data


def openkh_tool(name):
    """Il percorso di un programma di OpenKH, per esempio "OpenKh.Command.IdxImg.exe"."""
    folder = _configured("OPENKH_DIR", "openkh_dir")
    if folder is None or not (folder / name).exists():
        _stop(f"il programma di OpenKH {name}" + (f" in {folder}" if folder else ""), "OPENKH_DIR", "openkh_dir")
    return folder / name
