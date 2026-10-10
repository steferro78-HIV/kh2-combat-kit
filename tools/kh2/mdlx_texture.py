"""Lettore delle texture dentro il modello di un personaggio di KH2 (file .mdlx).

Serve a non dover più esportare le texture a mano con un editor. Il formato è quello descritto
dal progetto OpenKH (Apache-2.0, `OpenKh.Kh2/ModelTexture.cs`), riscritto da capo in Python.

Come sono fatti i dati, in breve: ogni immagine è una griglia di numeri (uno per punto) che
indicano un colore dentro una tavolozza condivisa da 256 colori; la tavolozza è salvata
nell'ordine un po' rimescolato che voleva la PlayStation 2, e la trasparenza va da 0 a 128
invece che da 0 a 255.
"""
import struct
import zlib

import kh2data

TEXTURE_ENTRY_TYPE = 7
TRANSFER_SIZE = 0x90      # un blocco "come caricare questi dati nella memoria video"
TRANSFER_BUFFER = 0x20    # dentro il blocco: dove vanno i dati e in che formato
TRANSFER_DATA_OFFSET = 0x74
IMAGE_INFO_SIZE = 0xA0    # un blocco "come usare questa immagine"
IMAGE_INFO_TEX0 = 0x70
FORMAT_32_BITS, FORMAT_8_BITS, FORMAT_4_BITS = 0x00, 0x13, 0x14


# --- la memoria video della PlayStation 2 -------------------------------------------------------
# La memoria è fatta di pagine da 8192 byte, ogni pagina di 32 blocchi da 256 byte, ogni blocco
# di 4 colonne da 64 byte. Dove finisce un punto dell'immagine dipende dal formato con cui la si
# guarda. Le texture a 8 bit di KH2 sono state caricate "come se" fossero immagini a 32 bit: per
# riaverle bisogna rifare quel caricamento e poi rileggere la memoria come immagine a 8 bit.
# Le tre tabelle descrivono la macchina (ordine dei blocchi nella pagina e dei punti nella
# colonna) e sono le stesse in ogni emulatore; qui sono prese da OpenKH (`Ps2.Reform32.cs`,
# `Ps2.Reform8.cs`).
PAGE_BYTES, BLOCK_BYTES, COLUMN_BYTES = 8192, 256, 64
BLOCK_ORDER = (0, 1, 4, 5, 16, 17, 20, 21, 2, 3, 6, 7, 18, 19, 22, 23,
               8, 9, 12, 13, 24, 25, 28, 29, 10, 11, 14, 15, 26, 27, 30, 31)
POINT_ORDER_32 = (0, 1, 4, 5, 8, 9, 12, 13, 2, 3, 6, 7, 10, 11, 14, 15)
POINT_ORDER_8 = (
    (0x00, 0x04, 0x10, 0x14, 0x20, 0x24, 0x30, 0x34, 0x02, 0x06, 0x12, 0x16, 0x22, 0x26, 0x32, 0x36,
     0x08, 0x0c, 0x18, 0x1c, 0x28, 0x2c, 0x38, 0x3c, 0x0a, 0x0e, 0x1a, 0x1e, 0x2a, 0x2e, 0x3a, 0x3e,
     0x21, 0x25, 0x31, 0x35, 0x01, 0x05, 0x11, 0x15, 0x23, 0x27, 0x33, 0x37, 0x03, 0x07, 0x13, 0x17,
     0x29, 0x2d, 0x39, 0x3d, 0x09, 0x0d, 0x19, 0x1d, 0x2b, 0x2f, 0x3b, 0x3f, 0x0b, 0x0f, 0x1b, 0x1f),
    (0x20, 0x24, 0x30, 0x34, 0x00, 0x04, 0x10, 0x14, 0x22, 0x26, 0x32, 0x36, 0x02, 0x06, 0x12, 0x16,
     0x28, 0x2c, 0x38, 0x3c, 0x08, 0x0c, 0x18, 0x1c, 0x2a, 0x2e, 0x3a, 0x3e, 0x0a, 0x0e, 0x1a, 0x1e,
     0x01, 0x05, 0x11, 0x15, 0x21, 0x25, 0x31, 0x35, 0x03, 0x07, 0x13, 0x17, 0x23, 0x27, 0x33, 0x37,
     0x09, 0x0d, 0x19, 0x1d, 0x29, 0x2d, 0x39, 0x3d, 0x0b, 0x0f, 0x1b, 0x1f, 0x2b, 0x2f, 0x3b, 0x3f),
)


def address_as_32_bits(x, y, pages_per_row):
    """Dove sta in memoria il punto (x, y) di un'immagine a 32 bit (pagina 64x32, blocco 8x8)."""
    page = x // 64 + pages_per_row * (y // 32)
    block = BLOCK_ORDER[(x % 64) // 8 + 8 * ((y % 32) // 8)]
    column = (y % 8) // 2
    return page * PAGE_BYTES + block * BLOCK_BYTES + column * COLUMN_BYTES + 4 * POINT_ORDER_32[x % 8 + 8 * (y % 2)]


def address_as_8_bits(x, y, pages_per_row):
    """Dove sta in memoria il punto (x, y) di un'immagine a 8 bit (pagina 128x64, blocco 16x16)."""
    page = x // 128 + pages_per_row * (y // 64)
    block = BLOCK_ORDER[(x % 128) // 16 + 8 * ((y % 64) // 16)]
    column = (y % 16) // 4
    return page * PAGE_BYTES + block * BLOCK_BYTES + column * COLUMN_BYTES + POINT_ORDER_8[column & 1][x % 16 + 16 * (y % 4)]


def unscramble_8_bits(stored, width, height):
    """Rimette in ordine un'immagine a 8 bit che è stata salvata come se fosse a 32 bit."""
    pages_per_row = width // 128
    memory = bytearray(len(stored))
    for y in range(height // 2):          # la stessa memoria, vista a 32 bit, è larga e alta la metà
        for x in range(width // 2):
            source = 4 * (x + (width // 2) * y)
            target = address_as_32_bits(x, y, pages_per_row)
            memory[target:target + 4] = stored[source:source + 4]
    return bytes(memory[address_as_8_bits(x, y, pages_per_row)] for y in range(height) for x in range(width))


def bits(value, position, size):
    return (value >> position) & ((1 << size) - 1)


def palette_slot(index, base, group):
    """Posizione nella tavolozza salvata del colore numero `index`."""
    return ((index & 7) + (index & 8) * 8 + (index & 16) // 2 + (index & ~31) * 4
            + (base & 7) * 4 + (base & 8) * 0x80 + (base & 16) * 2 + (base & ~31) * 0x40
            + (group & 1) * 8 + (group & 14) * 0x40)


def read_textures(mdlx):
    """Restituisce le immagini del modello: lista di (larghezza, altezza, byte RGBA riga per riga)."""
    base = next(offset for kind, _name, offset, _size in kh2data.read_bar(mdlx) if kind == TEXTURE_ENTRY_TYPE)
    (_magic, color_count, transfer_count, image_count, order_offset, transfers_offset, infos_offset,
     _pixels_offset, palette_offset) = struct.unpack_from("<9i", mdlx, base)
    palette = mdlx[base + palette_offset:base + palette_offset + color_count * 4]
    order = mdlx[base + order_offset:base + order_offset + image_count]

    def transfer(index):
        start = base + transfers_offset + index * TRANSFER_SIZE
        buffer = struct.unpack_from("<q", mdlx, start + TRANSFER_BUFFER)[0]
        return bits(buffer, 32, 14), bits(buffer, 56, 6), struct.unpack_from("<i", mdlx, start + TRANSFER_DATA_OFFSET)[0]

    palette_base, _format, _offset = transfer(0)  # il primo blocco è quello della tavolozza
    images = []
    for i in range(image_count):
        tex0 = struct.unpack_from("<q", mdlx, base + infos_offset + i * IMAGE_INFO_SIZE + IMAGE_INFO_TEX0)[0]
        pixel_format = bits(tex0, 20, 6)
        width, height = 1 << bits(tex0, 26, 4), 1 << bits(tex0, 30, 4)
        colors_base, colors_group = bits(tex0, 37, 14) - palette_base, bits(tex0, 56, 5)
        _target, upload_format, data_offset = transfer(1 + order[i])
        scrambled = upload_format == FORMAT_32_BITS
        if upload_format not in (pixel_format, FORMAT_32_BITS) or (scrambled and pixel_format != FORMAT_8_BITS):
            raise NotImplementedError(f"texture {i}: formato {pixel_format:#x} caricato come {upload_format:#x}, non gestito")

        if pixel_format == FORMAT_8_BITS:
            indices = mdlx[base + data_offset:base + data_offset + width * height]
            if scrambled:
                indices = unscramble_8_bits(indices, width, height)
            color_count_here = 256
        elif pixel_format == FORMAT_4_BITS:
            packed = mdlx[base + data_offset:base + data_offset + width * height // 2]
            indices = bytes(value for byte in packed for value in (byte & 15, byte >> 4))
            color_count_here = 16
        else:
            raise NotImplementedError(f"formato di texture {pixel_format:#x} non gestito")

        colors = []
        for index in range(color_count_here):
            slot = palette_slot(index, colors_base, colors_group) * 4
            red, green, blue, alpha = palette[slot:slot + 4]
            colors.append(bytes((red, green, blue, min(alpha * 2, 255))))
        images.append((width, height, b"".join(colors[index] for index in indices)))
    return images


def write_png(path, width, height, rgba):
    """Scrive un'immagine RGBA in un file PNG, senza librerie esterne."""
    def chunk(kind, payload):
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload))

    rows = b"".join(b"\x00" + rgba[y * width * 4:(y + 1) * width * 4] for y in range(height))
    with open(path, "wb") as handle:
        handle.write(b"\x89PNG\r\n\x1a\n")
        handle.write(chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)))
        handle.write(chunk(b"IDAT", zlib.compress(rows, 9)))
        handle.write(chunk(b"IEND", b""))
