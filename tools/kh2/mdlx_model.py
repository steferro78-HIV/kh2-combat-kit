"""Lettore del modello di un personaggio di KH2 (file .mdlx): scheletro, forme e pesi delle ossa.

Serve a non dover più esportare il modello a mano con un editor. Il formato è quello descritto
dal progetto OpenKH (Apache-2.0): `OpenKh.Kh2/Models/ModelSkeletal.cs`, `ModelCommon.cs`,
`OpenKh.Ps2/VifUnpacker.cs` e `VpuPacket.cs`. Qui è riscritto da capo in Python.

Come sono fatti i dati, in breve:
- il modello è diviso in gruppi, uno per ogni pezzo che usa una certa texture;
- i vertici di ogni gruppo sono impacchettati come li voleva la PlayStation 2: una serie di
  comandi che riempiono una piccola memoria, da cui poi si leggono le tabelle;
- ogni vertice è salvato **rispetto a un osso** (o a più ossa, già moltiplicato per il peso),
  non nello spazio del modello: per avere la posizione vera va applicata la posa di riposo.
"""
import math
import struct

import kh2data

MODEL_ENTRY_TYPE = 4
RESERVED = 0x90          # spazio che il gioco usa a runtime, prima dell'intestazione
HEADER_SIZE = 0x20
GROUP_HEADER_SIZE = 0x20
BONE_SIZE = 0x40
DMA_PACKET_SIZE = 0x10
VU_MEMORY_SIZE = 16 * 1024
UV_SCALE = 4096.0

# Come un nodo della striscia di triangoli chiude (o non chiude) un triangolo.
DRAW_BOTH_SIDES, DRAW_NOTHING, DRAW_TRIANGLE, DRAW_TRIANGLE_INVERSE = 0x00, 0x10, 0x20, 0x30


# --- matrici 4x4 scritte riga per riga, con la traslazione nell'ultima riga -------------------

def multiply(a, b):
    return [sum(a[row * 4 + k] * b[k * 4 + column] for k in range(4)) for row in range(4) for column in range(4)]


def transform(point, matrix):
    """Applica la matrice a un punto (x, y, z, w) e restituisce (x, y, z)."""
    x, y, z, w = point
    return tuple(x * matrix[c] + y * matrix[4 + c] + z * matrix[8 + c] + w * matrix[12 + c] for c in range(3))


def local_matrix(scale, rotation, translation):
    """Scala, poi rotazione attorno a X, Y e Z, poi traslazione."""
    sx, sy, sz = scale
    (cx, sxn), (cy, syn), (cz, szn) = ((math.cos(a), math.sin(a)) for a in rotation)
    scale_m = [sx, 0, 0, 0, 0, sy, 0, 0, 0, 0, sz, 0, 0, 0, 0, 1]
    rot_x = [1, 0, 0, 0, 0, cx, sxn, 0, 0, -sxn, cx, 0, 0, 0, 0, 1]
    rot_y = [cy, 0, -syn, 0, 0, 1, 0, 0, syn, 0, cy, 0, 0, 0, 0, 1]
    rot_z = [cz, szn, 0, 0, -szn, cz, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    move = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, translation[0], translation[1], translation[2], 1]
    result = scale_m
    for step in (rot_x, rot_y, rot_z, move):
        result = multiply(result, step)
    return result


# --- lo spacchettamento dei comandi della PlayStation 2 ---------------------------------------

def unpack_vif(code):
    """Esegue i comandi di un pacchetto e restituisce la memoria che hanno riempito."""
    memory = bytearray(VU_MEMORY_SIZE)
    masks = [0, 0, 0, 0]
    pc = 0

    def read(size, signed):
        nonlocal pc
        value = int.from_bytes(code[pc:pc + size], "little", signed=signed)
        pc += size
        return value & 0xFFFFFFFF

    while pc + 4 <= len(code):
        opcode = struct.unpack_from("<I", code, pc)[0]
        pc += 4
        command = (opcode >> 24) & 0x7F
        if command & 0x60 == 0x60:  # trasferisce dati nella memoria
            address = (opcode & 0x1FF) * 16
            signed = (opcode & 0x400) != 0
            size = (4, 2, 1, 2)[(opcode >> 24) & 3]
            components = ((opcode >> 26) & 3) + 1
            use_mask = (opcode >> 28) & 1
            for item in range((opcode >> 16) & 0xFF):
                mask = masks[item & 3] if use_mask else 0
                writes = [((mask >> (2 * c)) & 3) == 0 for c in range(4)]
                if components == 1:
                    value = read(size, signed)
                    values = [value] * 4
                elif components == 2:
                    x, y = read(size, signed), read(size, signed)
                    values = [x, y, x, y]
                elif components == 3:
                    values = [read(size, signed) if writes[c] else None for c in range(3)]
                    # Il quarto valore, se va scritto, è quello che viene dopo, senza consumarlo.
                    values.append(int.from_bytes(code[pc:pc + size], "little", signed=signed) & 0xFFFFFFFF)
                else:
                    values = [read(size, signed) if writes[c] else None for c in range(4)]
                for c in range(4):
                    if writes[c] and values[c] is not None and address + c * 4 + 4 <= VU_MEMORY_SIZE:
                        struct.pack_into("<I", memory, address + c * 4, values[c])
                address += 16
            pc = (pc + 3) & ~3
        elif command in (0x14, 0x17):  # avvia il microprogramma: il pacchetto è finito
            break
        elif command == 0x20:          # imposta le maschere
            value = struct.unpack_from("<I", code, pc)[0]
            pc += 4
            masks = [(value >> (8 * i)) & 0xFF for i in range(4)]
        elif command in (0x30, 0x31):  # valori di riempimento per righe e colonne: non usati
            pc += 16
        elif command not in (0x00, 0x01):
            raise ValueError(f"comando VIF sconosciuto {command:#x} a {pc - 4:#x}")
    return memory


def read_table(memory, offset, count, fmt):
    size = struct.calcsize(fmt)
    return [struct.unpack_from(fmt, memory, offset * 16 + i * size) for i in range(count)]


def read_packet(memory, bones):
    """Legge le tabelle di un pacchetto spacchettato. `bones` sono le ossa a cui si riferisce.

    Restituisce (vertici, nodi): ogni vertice è una lista di (osso, x, y, z, peso); ogni nodo
    della striscia è (u, v, indice del vertice nel pacchetto, funzione).
    """
    (_kind, _color_step, _magic, _buffer, node_count, node_offset, range_offset, _matrix_offset, _color_count,
     _color_offset, weight_group_count, weight_group_offset, coord_count, coord_offset, _index_offset,
     range_count) = struct.unpack_from("<16i", memory, 0)
    ranges = [value for (value,) in read_table(memory, range_offset, range_count, "<i")]
    nodes = read_table(memory, node_offset, node_count, "<4i")
    coords = read_table(memory, coord_offset, coord_count, "<4f")

    # Le coordinate sono in fila, raggruppate per osso: tante per il primo, tante per il secondo...
    positions = []
    bone_slot, used = 0, 0
    for x, y, z, w in coords:
        positions.append((bones[bone_slot], x, y, z, w))
        used += 1
        if bone_slot < len(ranges) and used >= ranges[bone_slot]:
            bone_slot, used = bone_slot + 1, 0

    if weight_group_count == 0:
        vertices = [[position] for position in positions]
    else:
        # Prima i vertici legati a un osso solo, poi quelli a due, a tre...
        counts = [value for (value,) in read_table(memory, weight_group_offset, weight_group_count, "<i")]
        cursor = weight_group_offset * 16 + weight_group_count * 4
        vertices = []
        for bones_per_vertex, count in enumerate(counts, start=1):
            cursor = (cursor + 15) & ~15
            for _ in range(count):
                indices = struct.unpack_from(f"<{bones_per_vertex}i", memory, cursor)
                cursor += 4 * bones_per_vertex
                vertices.append([positions[index] for index in indices])
    return vertices, nodes


# --- il modello ---------------------------------------------------------------------------------

class Model:
    """Scheletro e forme di un modello, nella posa di riposo.

    - `parents[i]`: osso genitore dell'osso i (-1 per la radice);
    - `bind[i]`: matrice assoluta di riposo dell'osso i;
    - `groups`: per ogni gruppo, la texture usata e la lista dei triangoli; ogni triangolo ha tre
      vertici, ognuno un dizionario con `position` (x, y, z), `uv` (u, v) e `weights`
      [(osso, peso), ...].
    """

    def __init__(self, data):
        base = next(offset for kind, _name, offset, _size in kh2data.read_bar(data) if kind == MODEL_ENTRY_TYPE)
        header = base + RESERVED
        bone_count, self.texture_count, bone_offset, _bone_data_offset, group_count = struct.unpack_from(
            "<HHIIi", data, header + 0x10)

        self.parents, locals_ = [], []
        for i in range(bone_count):
            start = header + bone_offset + i * BONE_SIZE
            _index, _sibling, parent, _child = struct.unpack_from("<4h", data, start)
            scale = struct.unpack_from("<3f", data, start + 0x10)
            rotation = struct.unpack_from("<3f", data, start + 0x20)
            translation = struct.unpack_from("<3f", data, start + 0x30)
            self.parents.append(parent)
            locals_.append(local_matrix(scale, rotation, translation))
        # Un genitore viene sempre prima dei suoi figli.
        self.bind = []
        for i, matrix in enumerate(locals_):
            self.bind.append(multiply(matrix, self.bind[self.parents[i]]) if self.parents[i] >= 0 else matrix)

        self.groups = []
        cursor = header + bone_offset + bone_count * BONE_SIZE
        for g in range(group_count):
            (_attributes, texture, _polygons, _has_buffer, _alternative, dma_offset, bone_list_offset,
             packet_count, _reserved) = struct.unpack_from("<iIIHHIIII", data, header + HEADER_SIZE + g * GROUP_HEADER_SIZE)
            vif_start = (cursor + 15) & ~15
            dma_start = header + dma_offset

            # Le istruzioni di caricamento: una "testa" con la lunghezza del pacchetto, poi i
            # riferimenti alle ossa, poi una voce di chiusura con lunghezza zero.
            packet_sizes, open_packet = [], False
            for p in range(packet_count):
                quad_words = struct.unpack_from("<H", data, dma_start + p * DMA_PACKET_SIZE)[0]
                if not open_packet:
                    packet_sizes.append(quad_words * 16)
                    open_packet = True
                elif quad_words == 0:
                    open_packet = False

            # Le ossa usate da ogni pacchetto, separate da -1.
            list_start = header + bone_list_offset
            count = struct.unpack_from("<i", data, list_start)[0]
            bone_lists, current = [], []
            for value in struct.unpack_from(f"<{count}i", data, list_start + 4):
                if value == -1:
                    bone_lists.append(current)
                    current = []
                else:
                    current.append(value)
            if current:
                bone_lists.append(current)
            cursor = list_start + 4 + count * 4

            vertices, nodes = [], []
            for size, bones in zip(packet_sizes, bone_lists):
                packet_vertices, packet_nodes = read_packet(unpack_vif(data[vif_start:vif_start + size]), bones)
                first = len(vertices)
                vertices.extend(packet_vertices)
                nodes.extend((u, v, first + index, function) for u, v, index, function in packet_nodes)
                vif_start += size

            corners = []
            for u, v, index, _function in nodes:
                influences = vertices[index]
                position = [0.0, 0.0, 0.0]
                weights = []
                for bone, x, y, z, w in influences:
                    # Con un osso solo il peso non è scritto e vale 1.
                    w = w if w != 0 else 1.0
                    moved = transform((x, y, z, w), self.bind[bone])
                    position = [position[i] + moved[i] for i in range(3)]
                    weights.append((bone, w))
                corners.append({"position": tuple(position), "uv": (u / UV_SCALE, v / UV_SCALE), "weights": weights})

            triangles = []
            for i, (_u, _v, _index, function) in enumerate(nodes):
                if function in (DRAW_TRIANGLE, DRAW_BOTH_SIDES):
                    triangles.append((corners[i], corners[i - 2], corners[i - 1]))
                if function in (DRAW_TRIANGLE_INVERSE, DRAW_BOTH_SIDES):
                    triangles.append((corners[i], corners[i - 1], corners[i - 2]))
            self.groups.append({"texture": texture, "triangles": triangles})


def read_model(path):
    with open(path, "rb") as handle:
        return Model(handle.read())
