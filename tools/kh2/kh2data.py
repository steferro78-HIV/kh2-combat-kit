"""Lettura dei file dati di KH2 estratti in extracted/ (mai nel repository).

Formati secondo la documentazione OpenKH (https://openkh.dev/kh2/):
BAR (contenitore), ATKP e PTYA dentro 00battle.bin, MSET/ANB con i trigger di animazione.
"""
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXTRACTED = ROOT / "extracted" / "kh2" / "original"


def read_bar(data, base=0):
    """Restituisce le voci di un contenitore BAR: (tipo, nome, offset assoluto, dimensione)."""
    if data[base:base + 4] != b"BAR\x01":
        raise ValueError("non è un BAR")
    count = struct.unpack_from("<I", data, base + 4)[0]
    entries = []
    for i in range(count):
        kind, _dup, name, offset, size = struct.unpack_from("<HH4sII", data, base + 16 + i * 16)
        entries.append((kind, name.rstrip(b"\0").decode("ascii", "replace"), base + offset, size))
    return entries


def read_atkp(data, offset):
    """Tabella dei parametri d'attacco, indicizzata per Id."""
    _kind, count = struct.unpack_from("<II", data, offset)
    table = {}
    for i in range(count):
        p = offset + 8 + i * 48
        (sub_id, atk_id, atk_type, crit, power, team, element, reaction, _fx,
         knock1, knock2, _unk, flags) = struct.unpack_from("<HHBBHBBBBhhhB", data, p)
        interval, = struct.unpack_from("<H", data, p + 38)
        revenge, _tr, combo_group, _rnd, kind_bits = struct.unpack_from("<BBBBB", data, p + 42)
        table.setdefault(atk_id, []).append({
            "sub_id": sub_id, "id": atk_id, "type": atk_type, "crit": crit, "power": power,
            "team": team, "element": element, "reaction": reaction, "knock1": knock1,
            "knock2": knock2, "flags": flags, "interval": interval, "revenge": revenge,
            "combo_group": combo_group, "kind": kind_bits,
        })
    return table


PTYA_FIELDS = ("id", "type", "sub", "combo_offset", "flag", "motion", "next_motion", "jump",
               "jump_max", "jump_min", "speed_min", "speed_max", "near", "far", "low", "high",
               "inner_min", "inner_max", "blend", "dist_adjust", "ability", "score")


def read_ptya(data, offset):
    """Restituisce (puntatori, insiemi): ogni insieme è la lista di attacchi di un personaggio."""
    _kind, count = struct.unpack_from("<II", data, offset)
    pointers = list(struct.unpack_from("<%dI" % count, data, offset + 8))
    sets = {}
    for ptr in sorted(set(pointers)):
        p = offset + ptr
        n, = struct.unpack_from("<I", data, p)
        rows = []
        for i in range(n):
            values = struct.unpack_from("<BBbbIHH13fHH", data, p + 4 + i * 68)
            rows.append(dict(zip(PTYA_FIELDS, values)))
        sets[ptr] = rows
    return pointers, sets


def read_triggers(data, offset, size):
    """Trigger di un'animazione: (a intervallo [inizio, fine, id, parametri], a frame singolo)."""
    if size < 4:
        return [], []
    n_range, n_frame, _frame_off = struct.unpack_from("<BBH", data, offset)
    p = offset + 4
    ranges, frames = [], []
    for _ in range(n_range):
        start, end, trig, n_param = struct.unpack_from("<hhBB", data, p)
        params = struct.unpack_from("<%dh" % n_param, data, p + 6)
        ranges.append((start, end, trig, params))
        p += 6 + n_param * 2
    for _ in range(n_frame):
        frame, trig, n_param = struct.unpack_from("<hBB", data, p)
        params = struct.unpack_from("<%dh" % n_param, data, p + 4)
        frames.append((frame, trig, params))
        p += 4 + n_param * 2
    return ranges, frames


def load(name):
    return (EXTRACTED / name).read_bytes()
