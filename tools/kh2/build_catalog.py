"""Costruisce il catalogo del combattimento di Sora (forma base) dalla propria copia di KH2.

Uso: python tools/kh2/build_catalog.py
Risultato (in extracted/, mai nel repository: sono dati del gioco):
- extracted/kh2/catalog/sora_base.catalog       il catalogo che kh2-combat-core carica;
- extracted/kh2/catalog/sora_base_riassunto.txt lo stesso contenuto in forma leggibile.

Cosa contiene, tutto letto dai file del gioco (formati documentati da OpenKH,
https://openkh.dev/kh2/):
- il repertorio dei colpi di Sora (tabella PTYA di 00battle.bin, nell'ordine del file, che è
  l'ordine di priorità): animazione, colpo successivo, condizioni di distanza, dislivello e
  angolo, salto, velocità, abilità richiesta, punteggio;
- per ogni animazione di Sora e del Keyblade: durata ed eventi (trigger) con i loro frame;
- per ogni zona di danno citata dagli eventi: la voce ATKP (potenza, reazione, spinta...).

Il formato del catalogo è testo semplice, una riga per cosa, "parola chiave=valore" (gli
esempi qui sotto hanno numeri inventati):
    entry id=5 type=3 sub=6 ...                                    una voce del repertorio
    motion id=10 name=A000 frames=40                               un'animazione
    span motion=10 who=body start=20 end=-1 trigger=11 params=     un suo evento "a tratto"
    point motion=10 who=body frame=40 trigger=16 params=8          un suo evento "a istante"
    attack id=3 sub_id=0 power=100 ...                             una voce ATKP
    ability id=97 name=Aerial_Sweep                                nome di un'abilità (vedi ABILITY_NAMES)
"""
import struct
import sys

import extract_files
import kh2data as k

OUT_DIR = k.EXTRACTED.parent / "catalog"
CHARACTER, WEAPON = "P_EX100", "W_EX010"
SORA_PTYA_SET = 1          # quale insieme PTYA usa Sora in forma base
FILES = ("00battle.bin", f"obj/{CHARACTER}.mset", f"obj/{WEAPON}.mset")
MOTION_ENTRY, TRIGGER_ENTRY = 0x09, 0x10
VARIANTS = 4               # ogni animazione ha fino a quattro varianti; in battaglia si usa la prima che esiste
HITBOX_TRIGGERS = (10, 33)

# Numero dell'abilità (quello usato dal repertorio e dagli interruttori del personaggio) -> nome.
# I nomi vengono dall'elenco degli oggetti documentato da OpenKH. L'abbinamento ai numeri è
# stato fatto confrontando le voci del repertorio con le descrizioni delle abilità date dalla
# community. Fanno eccezione gli ultimi due, Berserk Charge e Combo Master: nessuna voce del
# repertorio li richiede, e il loro numero viene dallo studio del programma del gioco (vedi
# CREDITS.md, "How the rules were obtained"). Sono qui solo per completezza: la libreria non
# usa questi due numeri, per quelle due abilità ha due interruttori a parte.
ABILITY_NAMES = {
    1: "Guard", 18: "Upper_Slash", 90: "Slapshot", 91: "Dodge_Slash", 92: "Slide_Dash",
    93: "Guard_Break", 94: "Explosion", 95: "Finishing_Leap", 96: "Counterguard",
    97: "Aerial_Sweep", 98: "Aerial_Spiral", 99: "Horizontal_Slash", 100: "Aerial_Finish",
    101: "Retaliating_Slash", 177: "Flash_Step", 178: "Aerial_Dive", 179: "Magnet_Burst",
    180: "Vicinity_Break", 107: "Berserk_Charge", 173: "Combo_Master",
}
SPAN_NAMES = {0: "a terra", 1: "in aria", 2: "sfuma verso fermo", 3: "niente gravità", 9: "salto/atterraggio",
              10: "ZONA DI DANNO", 11: "combo permessa", 12: "scia dell'arma", 14: "reaction command permesso",
              19: "disattiva invincibilità", 20: "reaction command (sé)", 22: "gira verso il bersaglio",
              24: "niente gravità", 27: "non colpibile", 28: "gira verso l'aggancio", 29: "non lascia il suolo",
              33: "ZONA DI DANNO (combo)", 35: "combo permessa (magia)", 41: "movimento permesso",
              42: "mantiene lo slancio", 44: "fermo per attrito", 47: "gira verso il movimento",
              50: "finale permesso"}
POINT_NAMES = {0: "salto", 15: "gira verso il bersaglio", 17: "lancia magia", 22: "appare l'arma",
               28: "controllo Revenge", 29: "appare l'arma", 34: "controllo Quick Run", 35: "caduta se in aria"}
QUIET_SPANS, QUIET_POINTS = {23}, {1, 2, 8, 13, 14, 19, 31}  # espressioni del viso, suoni, effetti


def motion_table(mset):
    """Le animazioni di un file .mset: numero -> (nome, durata in frame, eventi a intervallo, eventi a frame)."""
    slots = k.read_bar(mset)
    motions = {}
    for number in range(len(slots) // VARIANTS):
        for variant in range(VARIANTS):
            _kind, name, offset, size = slots[number * VARIANTS + variant]
            if size == 0:
                continue
            frames, spans, points = None, [], []
            for kind, _name, off, sz in k.read_bar(mset, offset):
                if kind == MOTION_ENTRY and sz >= 0x130 and struct.unpack_from("<i", mset, off + 0x90)[0] == 0:
                    frames = struct.unpack_from("<i", mset, off + 0xA0 + 0x04)[0]
                elif kind == TRIGGER_ENTRY:
                    spans, points = k.read_triggers(mset, off, sz)
            if frames is not None:
                motions[number] = (name, frames, spans, points)
            break
    return motions


def line(keyword, **values):
    def text(value):
        if isinstance(value, float):
            return repr(round(value, 6))
        if isinstance(value, (list, tuple)):
            return ";".join(str(v) for v in value)
        return str(value)
    return keyword + " " + " ".join(f"{name}={text(value)}" for name, value in values.items())


def main():
    if any(not (k.EXTRACTED / name).exists() for name in FILES):
        print("estraggo dal gioco:", ", ".join(FILES))
        if extract_files.main(list(FILES)) != 0:
            return 1
    battle = k.load("00battle.bin")
    parts = {name: offset for _kind, name, offset, _size in k.read_bar(battle)}
    attacks = k.read_atkp(battle, parts["atkp"])
    pointers, sets = k.read_ptya(battle, parts["ptya"])
    entries = sets[pointers[SORA_PTYA_SET]]
    body = motion_table(k.load(f"obj/{CHARACTER}.mset"))
    weapon = motion_table(k.load(f"obj/{WEAPON}.mset"))

    lines = ["kh2-catalog 1", line("source", character=CHARACTER, weapon=WEAPON, ptya_set=SORA_PTYA_SET)]
    for entry in entries:
        lines.append(line("entry", **entry))
    used_attacks = set()
    for number in sorted(body):
        name, frames, _spans, _points = body[number]
        lines.append(line("motion", id=number, name=name, frames=frames))
        for who, table in (("body", body), ("weapon", weapon)):
            if number not in table:
                continue
            _name, _frames, spans, points = table[number]
            for start, end, trigger, params in sorted(spans):
                lines.append(line("span", motion=number, who=who, start=start, end=end, trigger=trigger, params=params))
                if trigger in HITBOX_TRIGGERS and params:
                    used_attacks.add(params[0])
            for frame, trigger, params in sorted(points):
                lines.append(line("point", motion=number, who=who, frame=frame, trigger=trigger, params=params))
    for attack_id in sorted(used_attacks):
        for attack in attacks.get(attack_id, []):
            lines.append(line("attack", **attack))
    for number, name in sorted(ABILITY_NAMES.items()):
        lines.append(line("ability", id=number, name=name))

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    catalog = OUT_DIR / "sora_base.catalog"
    catalog.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    summary = OUT_DIR / "sora_base_riassunto.txt"
    summary.write_text(readable(entries, body, weapon, attacks), encoding="utf-8", newline="\n")
    print(f"scritto {catalog} ({len(entries)} voci del repertorio, {len(body)} animazioni, {len(used_attacks)} zone di danno)")
    print(f"scritto {summary}")
    return 0


def readable(entries, body, weapon, attacks):
    """Il repertorio e le animazioni dei colpi in forma leggibile, per chi studia il sistema."""
    kinds = {0: "richiamata da un comando", 1: "contro il bersaglio principale", 2: "contro ogni nemico vicino",
             3: "sempre candidata"}
    out = ["REPERTORIO DI SORA (nell'ordine del file = ordine di priorità)", ""]
    for entry in entries:
        flags = entry["flag"]
        tags = [name for bit, name in ((1, "in aria"), (2, "portata che cresce"), (4, "finale"),
                                       (8, "basso invertito"), (16, "alto invertito")) if flags & bit]
        ability = ABILITY_NAMES.get(entry["ability"], str(entry["ability"])) if entry["ability"] else "-"
        name = body.get(entry["motion"], ("?",))[0]
        out.append(f"voce {entry['id']:3d}  animazione {entry['motion']} ({name}) poi {entry['next_motion']}  "
                   f"{kinds[entry['type']]}  [{', '.join(tags) or 'a terra'}]  abilità {ability}  "
                   f"alternativa {entry['sub']}  anticipo {entry['combo_offset']}  punteggio {entry['score']}")
        if entry["type"] in (1, 2):
            out.append(f"          distanza {entry['near']:.0f}..{entry['far']:.0f}  dislivello {entry['high']:.0f}..{entry['low']:.0f}  "
                       f"coseno {entry['inner_min']:.4f}..{entry['inner_max']:.4f}")
        if entry["jump"] > 0:
            out.append(f"          salto {entry['jump']:.0f} (min {entry['jump_min']:.0f}, max {entry['jump_max']:.0f})  "
                       f"velocità {entry['speed_min']:.0f}..{entry['speed_max']:.0f}  correzione distanza {entry['dist_adjust']:.0f}")
    wanted = sorted({e["motion"] for e in entries} | {e["next_motion"] for e in entries} | set(range(0, 6)) | set(range(40, 44)))
    out += ["", "ANIMAZIONI DEI COLPI (eventi di Sora e del Keyblade; -1 = fino alla fine)", ""]
    for number in wanted:
        if number not in body:
            continue
        name, frames, _spans, _points = body[number]
        out.append(f"animazione {number} ({name}), {frames} frame")
        for who, table in (("Sora", body), ("Keyblade", weapon)):
            if number not in table:
                continue
            for start, end, trigger, params in sorted(table[number][2]):
                if trigger in QUIET_SPANS:
                    continue
                out.append(f"    {who:8s} [{start:3d}..{end:3d}] {SPAN_NAMES.get(trigger, f'evento {trigger}')} {list(params) if params else ''}")
                if trigger in HITBOX_TRIGGERS and params:
                    for a in attacks.get(params[0], []):
                        out.append(f"             colpo {a['id']}/{a['sub_id']}: potenza {a['power']}, tipo {a['type']}, reazione {a['reaction']}, "
                                   f"spinta {a['knock1']}/{a['knock2']}, revenge {a['revenge']}, gruppo {a['combo_group']}, genere {a['kind']}")
            for frame, trigger, params in sorted(table[number][3]):
                if trigger not in QUIET_POINTS:
                    out.append(f"    {who:8s} @{frame:3d} {POINT_NAMES.get(trigger, f'evento a frame {trigger}')} {list(params) if params else ''}")
    return "\n".join(out) + "\n"


if __name__ == "__main__":
    sys.exit(main())
