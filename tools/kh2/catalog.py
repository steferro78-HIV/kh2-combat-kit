"""Legge il catalogo dei colpi scritto da tools/kh2/build_catalog.py (stesso formato che carica il core).

Serve agli strumenti in Python che devono sapere quali animazioni esistono e con quali tempi,
senza tenere copie a mano di quei numeri.
"""
from pathlib import Path

PATH = Path(__file__).resolve().parents[2] / "extracted" / "kh2" / "catalog" / "sora_base.catalog"
HITBOX_TRIGGERS, COMBO_ALLOWED, FINISHER_ALLOWED = (10, 33), 11, 50
# Animazioni che non sono colpi ma servono a chi disegna il personaggio.
IDLE, RUN, JUMP, FALL, LAND = 0, 2, 3, 4, 5
# Ritorno in equilibrio, colpo respinto a terra, parata riuscita, colpo respinto in aria, e una
# reazione al danno che l'arena usa come "sbalzo".
REACTIONS = (40, 41, 42, 43, 64)


def number(text):
    try:
        return int(text)
    except ValueError:
        try:
            return float(text)
        except ValueError:
            return text


def read(path=PATH):
    """Il catalogo come dizionario: entries (lista, in ordine), motions, attacks, abilities."""
    if not Path(path).exists():
        raise FileNotFoundError(f"manca {path}: esegui prima python tools/kh2/build_catalog.py")
    catalog = {"entries": [], "motions": {}, "attacks": {}, "abilities": {}}
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        keyword, _, rest = line.partition(" ")
        fields = dict(word.split("=", 1) for word in rest.split() if "=" in word)
        params = [int(v) for v in fields.pop("params", "").split(";") if v]
        values = {name: number(text) for name, text in fields.items()}
        if keyword == "entry":
            catalog["entries"].append(values)
        elif keyword == "motion":
            catalog["motions"][values["id"]] = {"name": values["name"], "frames": values["frames"],
                                                "body": [], "weapon": [], "body_points": [], "weapon_points": []}
        elif keyword == "span":
            catalog["motions"][values["motion"]][values["who"]].append((values["start"], values["end"], values["trigger"], params))
        elif keyword == "point":
            catalog["motions"][values["motion"]][values["who"] + "_points"].append((values["frame"], values["trigger"], params))
        elif keyword == "attack":
            catalog["attacks"].setdefault(values["id"], []).append(values)
        elif keyword == "ability":
            catalog["abilities"][values["id"]] = values["name"]
    return catalog


def shown_motions(catalog):
    """Le animazioni che servono per disegnare Sora: fermo, corsa, salto, caduta, tutti i colpi
    e le reazioni (ritorno in equilibrio, colpo respinto, parata riuscita, sbalzo)."""
    wanted = {IDLE, RUN, JUMP, FALL, LAND, *REACTIONS}
    for entry in catalog["entries"]:
        wanted.add(entry["motion"])
        wanted.add(entry["next_motion"])
    return sorted(number for number in wanted if number in catalog["motions"])


def timing(catalog, motion):
    """Tempi di un colpo: (durata, [(primo, ultimo frame di danno)], frame da cui si prosegue o None)."""
    data = catalog["motions"][motion]
    hits = sorted((start, end) for start, end, trigger, _params in data["weapon"] + data["body"] if trigger in HITBOX_TRIGGERS)
    chain = min((start for start, _end, trigger, _params in data["body"] if trigger == COMBO_ALLOWED), default=None)
    return data["frames"], hits, chain
