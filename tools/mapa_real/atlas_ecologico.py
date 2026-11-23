#!/usr/bin/env python3
"""Capa de ecosistemas con la leyenda del «Atlas ecológico e histórico del Uruguay» (v1.0).

El atlas (documento de trabajo que aportó el usuario, 23/11/2026) es una especificación: una
leyenda de 18 clases con códigos estables (PR01…AN02), una matriz regional, los gradientes que
gobiernan la distribución, la evolución por períodos y las reglas de integridad. No trae
polígonos: este paso **infiere** cada clase con reglas por gradientes sobre los datos que el
juego ya tiene (relieve, geología, bañados, monte nativo de 2021, costa) más la cartografía
oficial de pastizales del MGAP, y marca en cada celda si el dato es documentado, inferido o
desconocido, con qué confianza y de qué fuente sale. Ver docs/sistemas/atlas-ecologico.md.

Salidas (formato común, ver `empaquetar`):
  assets/mapas/uruguay/ecosistemas.bin      grilla del país (510 × 557, ~1 km), 5 capas por período
  assets/mapas/uruguay/ecosistemas.json     leyenda, fuentes, capas, reglas, estadísticas
  assets/mapas/montevideo/ecosistemas.bin           entorno de 74 km (grilla de 100 m de contexto.bin)
  assets/mapas/montevideo/ecosistemas_amplio.bin    entorno amplio de 300 km (grilla de 500 m)
  (la sección "ecosistemas" de assets/mapas/montevideo/meta.json describe estas dos)

Formato .bin: "ECO1", u16 ancho, u16 alto, u8 bytes por celda (6), u8 cantidad de capas; por
capa: i16 año desde, i16 año hasta (inclusive), u32 largo, y la grilla comprimida con zlib
(filas de norte a sur, columnas de oeste a este). Cada celda, 6 bytes:
  0 código primario (número estable de la leyenda; 0 = agua o fuera del alcance)
  1 código secundario (mosaico; 0 = ninguno)
  2 densidad arbórea: 0 ausente, 1 muy baja, 2 baja, 3 media, 4 alta, 5 sin dato
  3 altura del dosel en decímetros (0 = sin fuente)
  4 evidencia (bits 0-3: 0 desconocido, 1 inferencia ecológica, 2 documentado) y confianza
    (bits 4-7: 0 baja, 1 media, 2 alta)
  5 fuente (índice en "fuentes" del JSON)

Uso:
  python3 tools/mapa_real/atlas_ecologico.py --pastizales "/ruta/Pastizales del Uruguay.tif" \
      --cache /ruta/a/la/cache/del/pais
"""

import argparse
import json
import math
import os
import struct
import sys
import zlib
from collections import deque

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import construir_mapa as cm  # noqa: E402

RAIZ = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
PAIS = os.path.join(RAIZ, "assets", "mapas", "uruguay")
MVD = os.path.join(RAIZ, "assets", "mapas", "montevideo")

# --- Leyenda (códigos estables: el número no cambia nunca) ---------------------------------------
# (número, código, nombre, color de trabajo RGB). Los colores del atlas son "una paleta de trabajo";
# acá se ajustaron para que se lean sobre el mapa.
LEYENDA = [
    (1, "PR01", "Pradera natural", (176, 190, 112)),
    (2, "PR02", "Pastizal ralo sobre suelos superficiales", (206, 196, 132)),
    (3, "PR03", "Pastizal denso", (140, 176, 96)),
    (4, "PR04", "Pastizal alto y denso", (112, 156, 84)),
    (5, "SH01", "Chircal y matorral", (160, 138, 92)),
    (6, "FO01", "Monte ribereño/fluvial", (40, 104, 56)),
    (7, "FO02", "Monte de quebrada", (22, 78, 46)),
    (8, "FO03", "Monte serrano", (84, 112, 66)),
    (9, "FO04", "Bosque parque / espinal", (148, 128, 70)),
    (10, "FO05", "Monte psamófilo/costero", (92, 132, 104)),
    (11, "PA01", "Palmar de butiá", (196, 150, 52)),
    (12, "PA02", "Palmar de yatay", (210, 120, 60)),
    (13, "HU01", "Bañado / pastizal inundable", (84, 140, 136)),
    (14, "AQ01", "Vegetación acuática y de orilla", (120, 168, 186)),
    (15, "CO01", "Arenales, dunas y costa abierta", (232, 218, 172)),
    (16, "SA01", "Blanqueales/suelos alcalinos", (222, 214, 200)),
    (17, "AN01", "Sabana arbolada / arbolado disperso", (164, 150, 86)),
    (18, "AN02", "Ecotono bosque-pradera", (110, 140, 76)),
]
# Extensiones del juego (no están en el atlas; el atlas pide no mezclar nativo con plantado).
EXTENSIONES = [
    (40, "PL01", "Plantación forestal (pinos, acacias, eucaliptos)", (150, 60, 130)),
    (41, "UR01", "Área urbana", (120, 110, 110)),
    (42, "TR01", "Cobertura transformada (sin dato por celda)", (200, 180, 170)),
]
C = {c: n for n, c, _, _ in LEYENDA + EXTENSIONES}

DENS = ["ausente", "muy baja", "baja", "media", "alta", "sin dato"]
EVID = ["desconocido", "inferencia ecológica", "documentado"]
CONF = ["baja", "media", "alta"]
D_AUS, D_MB, D_B, D_M, D_A, D_ND = range(6)
E_DES, E_INF, E_DOC = 0, 1, 2
K_BAJA, K_MEDIA, K_ALTA = 0, 1, 2

# Fuentes (índice estable = byte 5 de cada celda).
FUENTES = [
    {"id": "ninguna", "cita": "Sin fuente (fuera del país o del alcance del atlas)."},
    {"id": "atlas", "cita": "Atlas ecológico e histórico del Uruguay, v1.0 (documento de trabajo, 23/11/2026): leyenda, matriz regional y períodos."},
    {"id": "dgf2021", "cita": "MGAP-DGF, Cartografía de Bosque Nativo 2021 (SNIG, DGF/Monte_Nativo_2021): presencia de monte observada en 2021."},
    {"id": "ide_banados", "cita": "IDE Uruguay, hidrografía 1:50.000, hidrografia:banado_a (y area_humeda_a en el entorno amplio)."},
    {"id": "pastizales", "cita": "MGAP-DGRN, Cartografía de pastizales del Uruguay (raster de 30 m, 5 comunidades; descargas.mgap.gub.uy/DGRN/Comunicaciones/Pastizales del Uruguay.zip)."},
    {"id": "dinamige", "cita": "DINAMIGE, Mapa de unidades geológicas 2017 (1:500.000)."},
    {"id": "ide_mdt", "cita": "IDE Uruguay, MDT de 30 m (relieve, pendiente, cursos derivados del relieve)."},
    {"id": "mgap_manual", "cita": "MGAP-DGF, Actualización del Manual de Manejo de Bosque Nativo en Uruguay, 1.ª ed., abril de 2018, cap. 3 (tipos de bosque)."},
    {"id": "gautreau2006", "cita": "P. Gautreau, tesis doctoral, Université de Lille 1, 2006 (pepite-depot.univ-lille.fr/LIBRE/Th_Num/2006/50376-2006-Gautreau.pdf)."},
    {"id": "bortolini", "cita": "S. V. Bortolini y A. Brazeiro, Distribución, abundancia y estado de conservación de los palmares de Butia yatay en Uruguay (IECA, Facultad de Ciencias, UdelaR; presentación publicada por el MGAP, 2020)."},
    {"id": "rosa_butia", "cita": "Rosa et al., campo natural del palmar de Butia odorata en Castillos (Agrociencia Uruguay; alice.cnptia.embrapa.br/alice/bitstream/doc/1004507): ~70.000 ha en dos áreas, Castillos y San Luis; 50–600 palmas/ha."},
    {"id": "porcile_atlas", "cita": "Porcile Maderni (2007), Crónicas del desarrollo forestal del Uruguay, citado por el atlas §5: forestación de Mario Ferreira en Las Toscas y Parque del Plata, más de 800 ha de dunas (no se leyó el original)."},
    {"id": "mapbiomas", "cita": "MapBiomas Uruguay, Colección 4, estadísticas por departamento 1985–2025 (mapbiomas-uruguay-collection-4-lulc.xlsx)."},
    {"id": "gautreau_atlas", "cita": "Gautreau, citado por el atlas §5: en una zona de Colonia el monte no ribereño pasó de ~271 km² (11,3 %) en planos de 1821–1835 a 107 km² (4,5 %) en 1966 (no se encontró la cifra en la tesis de 2006)."},
    {"id": "coneat", "cita": "MGAP, grupos de suelos CONEAT (rocosidad e índice de productividad; ver docs/sistemas/suelos-y-roca.md)."},
    {"id": "bosque1726", "cita": "Reconstrucción del monte de 1726 del juego (bosque_contexto.bin; ver docs/sistemas/vegetacion-historica.md)."},
    {"id": "ide_biomas", "cita": "IDE Uruguay, hidrografía: playas, bancos de arena, lagunas, bañados (biomas oficiales del mapa)."},
    {"id": "mtop", "cita": "MTOP, hidrografía (dnv_sgcm:hidro_pg) y localidades (dnv_sgcm:localidades_polig)."},
]
F = {f["id"]: k for k, f in enumerate(FUENTES)}

# Altura del dosel por clase, solo donde hay fuente (decímetros; rango en el JSON).
DOSEL = {
    "FO02": (200, "20–25 m el estrato superior (MGAP, Manual 2018, §3.3, citando a Escudero 2004)", "mgap_manual"),
    "FO05": (48, "3,5–6 m, máximo 8 m (MGAP, Manual 2018, §3.6, citando a Porcile 1988)", "mgap_manual"),
    "PA02": (80, "adultos de 6–10 m (Bortolini y Brazeiro)", "bortolini"),
    "FO03": (30, "unos 3 m (Carrere 1990; ver monte-1726-fuentes.md)", "atlas"),
    "FO01_sur": (50, "unos 5 m de altura media en el sur y el este (Major y Torighelli 1987, en Carrere 1990)", "atlas"),
}

# Matriz regional del atlas (§3), por departamento. Tacuarembó, Soriano y Colonia no figuran en
# la tabla del atlas: se asignan por cercanía y por el manual del MGAP (parque en el litoral oeste,
# incluidos Colonia y Soriano) [ESTIMADO].
REGIONES = ["norte basáltico", "litoral oeste", "Rivera y noreste", "centro", "sur", "sierras del este", "este", "costa platense y atlántica"]
R_NORTE, R_LITORAL, R_NE, R_CENTRO, R_SUR, R_SIERRAS, R_ESTE, R_COSTA = range(8)
REGION_DEPTO = {
    "Artigas": R_NORTE, "Salto": R_NORTE,
    "Paysandú": R_LITORAL, "Río Negro": R_LITORAL, "Soriano": R_LITORAL, "Colonia": R_LITORAL,
    "Rivera": R_NE, "Tacuarembó": R_NE,
    "Durazno": R_CENTRO, "Florida": R_CENTRO, "Flores": R_CENTRO,
    "San José": R_SUR, "Canelones": R_SUR, "Montevideo": R_SUR,
    "Lavalleja": R_SIERRAS, "Maldonado": R_SIERRAS, "Treinta y Tres": R_SIERRAS,
    "Rocha": R_ESTE, "Cerro Largo": R_ESTE,
}

# Lugares documentados (lon, lat). Las localidades salen de la capa de localidades del MTOP; los
# sitios de quebrada son áreas protegidas conocidas, con coordenadas aproximadas [ESTIMADO].
PALMAR_BUTIA = {
    # Castillos: entre Castillos, Aguas Dulces y Barra de Valizas (Rosa et al.; Montevideo Portal).
    "Castillos": [(-53.855, -34.197), (-53.782, -34.276), (-53.794, -34.338)],
    # San Luis: San Luis al Medio y 18 de Julio (cuenca de la Laguna Merín).
    "San Luis": [(-53.718, -33.604), (-53.554, -33.683)],
}
BUTIA_HA = 70000          # Rosa et al.: ~70.000 ha en las dos áreas
PALMAR_YATAY = {
    # Núcleos de Bortolini y Brazeiro con su área de 2013 (ha). Santo Domingo (826 ha) no se pudo
    # ubicar con las fuentes leídas: no se dibuja. Mujica y Porrúa (Chebataroff 1960) desaparecieron.
    "Quebracho": ((-57.897, -31.934), 1560),
    "Guichón": ((-57.200, -32.356), 310),
}
YATAY_RANGO_KM2 = 1450    # rango de extensión de la especie (Bortolini y Brazeiro)
QUEBRADAS = {
    "Valle del Lunarejo": (-55.90, -31.13),
    "Valle Edén": (-56.16, -31.82),
    "Quebrada de los Cuervos": (-54.46, -32.93),
}
FORESTACION_CANELONES = [(-55.741, -34.766), (-55.718, -34.756)]   # Las Toscas, Parque del Plata
FORESTACION_HA = 800      # "más de 800 hectáreas" (atlas, citando a Porcile)
COLONIA_RATIO = 271.0 / 107.0  # Gautreau (vía atlas): monte no ribereño 1821–35 / 1966

# Geología (código DINAMIGE de pais.json).
GEO_BASALTO = {52, 54, 55, 56}
GEO_ARENA_COSTA = {71, 72, 73, 76}
GEO_CRISTALINO = set(range(0, 42))         # basamento precámbrico y volcánicas del este
GEO_ALUVIAL = {75, 77}
GEO_ARENISCA_DRENADA = {53, 62, 63, 68}     # Tacuarembó, Guichón, Mercedes, Salto

# Comunidades de pastizal del raster (VAT) → código del atlas.
PASTIZAL_A_CODIGO = {1: "PR04", 2: "PR03", 3: "PR03", 4: "PR02", 5: "PR02"}


# --- Utilidades ---------------------------------------------------------------------------------
class Capa:
    """Una grilla de registros (arreglos uint8 por campo)."""

    def __init__(self, shape):
        z = lambda: np.zeros(shape, np.uint8)  # noqa: E731
        self.prim, self.sec, self.dens, self.dosel, self.ev, self.conf, self.fuente = (z() for _ in range(7))
        self.dens[:] = D_ND

    def copia(self):
        o = Capa(self.prim.shape)
        for k in ("prim", "sec", "dens", "dosel", "ev", "conf", "fuente"):
            getattr(o, k)[:] = getattr(self, k)
        return o

    def poner(self, m, prim=None, sec=None, dens=None, dosel=None, ev=None, conf=None, fuente=None):
        for k, v in (("prim", prim), ("sec", sec), ("dens", dens), ("dosel", dosel), ("ev", ev), ("conf", conf), ("fuente", fuente)):
            if v is None:
                continue
            a = getattr(self, k)
            a[m] = (C[v] if isinstance(v, str) else v) if np.isscalar(v) or isinstance(v, str) else np.asarray(v)[m]

    def bytes(self):
        reg = np.stack([self.prim, self.sec, self.dens, self.dosel, (self.ev & 15) | (self.conf << 4), self.fuente], axis=-1)
        return reg.astype(np.uint8).tobytes()


def empaquetar(ruta, capas):
    """capas: lista de (desde, hasta, Capa)."""
    alto, ancho = capas[0][2].prim.shape
    out = bytearray(b"ECO1" + struct.pack("<HHBB", ancho, alto, 6, len(capas)))
    for d, h, cp in capas:
        z = zlib.compress(cp.bytes(), 9)
        out += struct.pack("<hhI", d, h, len(z)) + z
    with open(ruta, "wb") as f:
        f.write(out)
    return len(out)


def bfs_dist(fuente, paso=1.0):
    """Distancia (vecindad 8, en pasos de celda, ~euclídea por chaflán) desde las celdas True."""
    ny, nx = fuente.shape
    INF = 1e9
    d = np.where(fuente, 0.0, INF)
    # Dos pasadas de chaflán (3-4) vectorizadas por filas.
    a, b = 1.0, 1.4142
    for _ in range(2):
        for i in range(ny):
            row = d[i]
            if i > 0:
                up = d[i - 1]
                row = np.minimum(row, up + a)
                row[1:] = np.minimum(row[1:], up[:-1] + b)
                row[:-1] = np.minimum(row[:-1], up[1:] + b)
            for j in range(1, nx):
                if row[j - 1] + a < row[j]:
                    row[j] = row[j - 1] + a
            d[i] = row
        for i in range(ny - 1, -1, -1):
            row = d[i]
            if i < ny - 1:
                dn = d[i + 1]
                row = np.minimum(row, dn + a)
                row[1:] = np.minimum(row[1:], dn[:-1] + b)
                row[:-1] = np.minimum(row[:-1], dn[1:] + b)
            for j in range(nx - 2, -1, -1):
                if row[j + 1] + a < row[j]:
                    row[j] = row[j + 1] + a
            d[i] = row
    return d * paso


def vecino_mas_cercano(valor, valido, max_pasos):
    """Rellena las celdas no válidas con el valor de la válida más cercana (BFS, vecindad 4) hasta
    `max_pasos`. Devuelve (valor, distancia en pasos; -1 sin relleno)."""
    ny, nx = valor.shape
    out = valor.copy()
    dist = np.full(valor.shape, -1, np.int32)
    q = deque()
    for i, j in zip(*np.nonzero(valido)):
        dist[i, j] = 0
        q.append((i, j))
    while q:
        i, j = q.popleft()
        if dist[i, j] >= max_pasos:
            continue
        for di, dj in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            a, b = i + di, j + dj
            if 0 <= a < ny and 0 <= b < nx and dist[a, b] < 0:
                dist[a, b] = dist[i, j] + 1
                out[a, b] = out[i, j]
                q.append((a, b))
    return out, dist


def ventana(a, r, f):
    """Máximo o mínimo en una ventana de (2r+1)² (bordes repetidos)."""
    ny, nx = a.shape
    p = np.pad(a, r, mode="edge")
    acc = None
    for di in range(-r, r + 1):
        for dj in range(-r, r + 1):
            s = p[r + di:r + di + ny, r + dj:r + dj + nx]
            acc = s if acc is None else f(acc, s)
    return acc


def km_entre(lon, lat, lon0, lat0):
    kx = 111.32 * math.cos(math.radians(lat0))
    return np.hypot((lon - lon0) * kx, (lat - lat0) * 110.95)


# --- Pastizales (raster del MGAP, UTM 21 S) -----------------------------------------------------
class Pastizales:
    def __init__(self, ruta):
        from pyproj import Transformer
        Image.MAX_IMAGE_PIXELS = None
        im = Image.open(ruta)
        tp = im.tag_v2[33922]
        sc = im.tag_v2[33550]
        self.x0, self.y0, self.d = tp[3], tp[4], sc[0]
        self.a = np.asarray(im, dtype=np.uint8)
        self.tr = Transformer.from_crs("EPSG:4326", "EPSG:32721", always_xy=True)
        print(f"  pastizales: {self.a.shape[1]} × {self.a.shape[0]} píxeles de {self.d:.0f} m", flush=True)

    def clase(self, lon, lat):
        """Comunidad (1..5) o 0 (no es pastizal hoy / fuera) en cada punto."""
        x, y = self.tr.transform(lon, lat)
        j = np.floor((x - self.x0) / self.d).astype(np.int64)
        i = np.floor((self.y0 - y) / self.d).astype(np.int64)
        ok = (i >= 0) & (j >= 0) & (i < self.a.shape[0]) & (j < self.a.shape[1])
        v = np.zeros(lon.shape, np.uint8)
        v[ok] = self.a[i[ok], j[ok]]
        v[v > 5] = 0
        return v

    def por_celda(self, lon_c, lat_c, dlon, dlat, sub):
        """Para cada celda centrada en (lon_c, lat_c): fracción observada de pastizal y la
        comunidad más frecuente (0 si ninguna), con sub × sub muestras."""
        cuentas = np.zeros(lon_c.shape + (6,), np.float32)
        for a in range(sub):
            for b in range(sub):
                lo = lon_c + ((b + 0.5) / sub - 0.5) * dlon
                la = lat_c - ((a + 0.5) / sub - 0.5) * dlat
                v = self.clase(lo, la)
                for k in range(6):
                    cuentas[..., k] += v == k
        cuentas /= sub * sub
        obs = 1.0 - cuentas[..., 0]
        mayor = np.argmax(cuentas[..., 1:], axis=-1).astype(np.uint8) + 1
        mayor[obs <= 0] = 0
        # Segunda comunidad (para el código secundario), si ocupa al menos un 20 %.
        c2 = cuentas[..., 1:].copy()
        np.put_along_axis(c2, (mayor.astype(np.int64)[..., None] - 1).clip(0), -1, axis=-1)
        segunda = np.argmax(c2, axis=-1).astype(np.uint8) + 1
        segunda[np.max(c2, axis=-1) < 0.2] = 0
        return obs, mayor, segunda


def codigo_pastizal(clase):
    out = np.zeros(clase.shape, np.uint8)
    for k, c in PASTIZAL_A_CODIGO.items():
        out[clase == k] = C[c]
    return out


# --- País ---------------------------------------------------------------------------------------
NX, NY = 510, 557
LON0, LAT0, DLON, DLAT = -58.50, -30.05, 0.0107, 0.009


def acumulacion(alt, tierra):
    """Área drenada (celdas) por D8 sobre el relieve de 1 km, de lo alto a lo bajo."""
    ny, nx = alt.shape
    acc = np.where(tierra, 1.0, 0.0)
    orden = np.argsort(-alt, axis=None)
    vec = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]
    for k in orden:
        i, j = divmod(int(k), nx)
        if not tierra[i, j]:
            continue
        h = alt[i, j]
        mejor, dest = 0.0, None
        for di, dj in vec:
            a, b = i + di, j + dj
            if 0 <= a < ny and 0 <= b < nx:
                caida = (h - alt[a, b]) / (1.4142 if di and dj else 1.0)
                if caida > mejor:
                    mejor, dest = caida, (a, b)
        if dest:
            acc[dest] += acc[i, j]
    return acc


def componente(mask, semilla):
    ny, nx = mask.shape
    out = np.zeros(mask.shape, bool)
    if not mask[semilla]:
        return out
    q = deque([semilla])
    out[semilla] = True
    while q:
        i, j = q.popleft()
        for di, dj in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            a, b = i + di, j + dj
            if 0 <= a < ny and 0 <= b < nx and mask[a, b] and not out[a, b]:
                out[a, b] = True
                q.append((a, b))
    return out


def construir_pais(past, cache):
    print("País (510 × 557 celdas de ~1 km)…", flush=True)
    mapa = json.load(open(os.path.join(PAIS, "mapa.json")))
    alt = np.frombuffer(open(os.path.join(PAIS, "relieve.bin"), "rb").read(), "<u2").reshape(NY, NX) / 10.0 - 100.0
    cap = np.frombuffer(open(os.path.join(PAIS, "capas.bin"), "rb").read(), np.uint8).reshape(NY, NX, 8)
    geo = np.frombuffer(open(os.path.join(RAIZ, "assets", "datos", "uruguay", "geologia_pais.bin"), "rb").read(), np.uint8).reshape(NY, NX).astype(np.int32) - 1
    agua = cap[..., 0] / 255.0
    emb = cap[..., 1] > 0
    depto = cap[..., 2]
    ban = cap[..., 3] / 255.0
    mon = cap[..., 4] / 255.0
    rio = cap[..., 5] > 0
    pend = cap[..., 6].astype(np.float32)
    nombre_depto = {d["id"]: d["nombre"] for d in mapa["departamentos"]}
    # Los embalses (valles en 1750) no tienen departamento en capas.bin: el del vecino.
    depto, _ = vecino_mas_cercano(depto, depto > 0, 30)
    depto = np.where(emb | (cap[..., 2] > 0), depto, 0)
    region = np.full((NY, NX), 255, np.uint8)
    for k, n in nombre_depto.items():
        region[depto == k] = REGION_DEPTO[n]
    tierra = (depto > 0) & ((agua < 0.5) | emb)
    jj, ii = np.meshgrid(np.arange(NX), np.arange(NY))
    lon = LON0 + jj * DLON
    lat = LAT0 - ii * DLAT

    # Gradientes.
    rel = ventana(alt, 1, np.maximum) - ventana(alt, 1, np.minimum)
    print("  acumulación de drenaje…", flush=True)
    acc = acumulacion(np.where(tierra, alt, -50.0), tierra)
    curso = rio | ((acc >= 40) & tierra)
    d_curso = bfs_dist(curso)  # en celdas (~km)
    agua_nat = (agua >= 0.5) & ~emb
    mar = componente(agua_nat, (NY - 1, NX - 1))
    costa_mar = mar & ((lat < -34.25) | (lon > -54.0))
    d_mar = bfs_dist(costa_mar)
    d_agua = bfs_dist(agua_nat)
    # Costa: franja de 6 km sobre el Río de la Plata y el Atlántico (región «costa» del atlas).
    region_aj = region.copy()
    region_aj[tierra & (d_mar <= 6)] = R_COSTA

    print("  pastizales por celda…", flush=True)
    obs, clase, clase2 = past.por_celda(lon, lat, DLON, DLAT, 5)
    pr_obs = codigo_pastizal(clase)
    pr_obs2 = codigo_pastizal(clase2)

    base = Capa((NY, NX))
    # Fuera del país: sin ecosistema del atlas.
    base.poner(~tierra & (depto == 0) & ~agua_nat, prim=0, dens=D_ND, ev=E_DES, conf=K_BAJA, fuente=F["ninguna"])
    base.poner(agua_nat, prim=0, dens=D_AUS, ev=E_DOC, conf=K_ALTA, fuente=F["mtop"])

    # 1) Matriz de pradera: comunidad observada hoy; donde hoy no hay pastizal, la del vecino
    #    observado más cercano (sustrato parecido) hasta 6 km; si no, pradera genérica.
    valido = tierra & (obs >= 0.25) & (pr_obs > 0)
    pr, dist = vecino_mas_cercano(np.where(valido, pr_obs, 0).astype(np.uint8), valido, 6)
    pr[(dist < 0) | (pr == 0)] = C["PR01"]
    base.poner(tierra, prim=pr, sec=np.where(valido, pr_obs2, 0), dens=D_MB, ev=E_INF,
               conf=np.where(valido, K_MEDIA, K_BAJA), fuente=np.where((dist >= 0) & (pr != C["PR01"]), F["pastizales"], F["atlas"]))
    # Basalto y sierras con mucha pendiente: suelos superficiales (pastizal ralo) aunque hoy no se observe.
    ralo = tierra & ~valido & (pend >= 25) & (np.isin(geo, list(GEO_BASALTO)) | np.isin(geo, list(GEO_CRISTALINO)))
    base.poner(ralo, prim="PR02", conf=K_BAJA, fuente=F["dinamige"])

    # 2) Chircal y matorral (secundario): regiones donde el atlas los nombra (Rivera y noreste,
    #    centro, sur), laderas de pendiente media sin monte. Confianza baja.
    chirca = tierra & np.isin(region, [R_NE, R_CENTRO, R_SUR]) & (pend >= 12) & (pend < 45) & (mon < 0.05) & (base.sec == 0)
    base.poner(chirca, sec="SH01")

    # 3) Monte: presencia observada en 2021 (DGF, polígonos de 30 ha o más). Tipo por gradientes.
    hay_monte = tierra & (mon >= 0.08)
    ribereno = d_curso <= 1.5
    quebrada = hay_monte & (rel >= 70) & (pend >= 30) & np.isin(region, [R_NORTE, R_NE, R_SIERRAS, R_ESTE])
    serrano = hay_monte & ~quebrada & np.isin(region, [R_SIERRAS, R_ESTE, R_NE]) & ((pend >= 22) | (np.isin(geo, list(GEO_CRISTALINO)) & (rel >= 45)))
    psamo = hay_monte & (d_mar <= 3) & (np.isin(geo, list(GEO_ARENA_COSTA)) | (alt < 15))
    fluvial = hay_monte & ~quebrada & ~serrano & ~psamo & ribereno
    resto = hay_monte & ~quebrada & ~serrano & ~psamo & ~fluvial
    parque = resto & np.isin(region, [R_LITORAL, R_NORTE, R_CENTRO]) & (pend < 30)
    ecotono = resto & ~parque
    dens = np.where(mon >= 0.4, D_A, np.where(mon >= 0.1, D_M, D_B)).astype(np.uint8)
    for m, cod, umbral, fuente_extra in ((fluvial, "FO01", 0.30, None), (quebrada, "FO02", 0.25, None), (serrano, "FO03", 0.30, None),
                                         (parque, "FO04", 0.20, None), (psamo, "FO05", 0.25, None), (ecotono, "AN02", 0.20, None)):
        prim_m = m & (mon >= umbral)
        sec_m = m & (mon < umbral)
        base.sec[prim_m] = base.prim[prim_m]
        base.poner(prim_m, prim=cod, dens=dens, ev=E_INF, conf=K_MEDIA, fuente=F["dgf2021"])
        base.poner(sec_m, sec=cod, dens=dens, ev=E_INF, conf=K_BAJA, fuente=F["dgf2021"])
    # Galería sin monte grande cartografiado a lo largo de los ríos con nombre: secundario.
    base.poner(tierra & rio & ~hay_monte & (base.sec == 0), sec="FO01")
    # Valles de los embalses (en 1750 eran río con su monte): monte ribereño inferido.
    base.poner(emb, prim="FO01", sec="AQ01", dens=D_ND, ev=E_INF, conf=K_BAJA, fuente=F["ide_mdt"])
    # Altura del dosel solo donde hay fuente.
    m = (base.prim == C["FO01"]) & np.isin(region_aj, [R_SUR, R_ESTE, R_SIERRAS, R_COSTA])
    base.dosel[m] = DOSEL["FO01_sur"][0]
    for cod in ("FO02", "FO03", "FO05"):
        base.dosel[base.prim == C[cod]] = DOSEL[cod][0]
    # Quebradas documentadas: más confianza cerca del sitio.
    for nombre, (lo, la) in QUEBRADAS.items():
        cerca = (km_entre(lon, lat, lo, la) <= 8) & (base.prim == C["FO02"])
        base.poner(cerca, ev=E_DOC, conf=K_MEDIA, fuente=F["mgap_manual"])

    # 4) Colonia: el monte no ribereño de 1821–35 era ~2,5 veces el de 1966 (Gautreau vía el
    #    atlas). Se reparte alrededor de los parches de parque que quedan hoy (inferencia).
    colonia = tierra & (depto == next(k for k, n in nombre_depto.items() if n == "Colonia"))
    nr = colonia & (mon >= 0.02) & ~ribereno
    actual = float((mon * nr).sum())
    objetivo = actual * COLONIA_RATIO
    cand = colonia & ~ribereno & (pend < 30) & (ban < 0.3) & (d_mar > 2) & ~np.isin(base.prim, [C[k] for k in ("FO01", "FO02", "FO03", "FO04", "FO05", "HU01", "AN02")])
    dist_p = bfs_dist(nr)
    orden = np.argsort(np.where(cand, dist_p, 1e9), axis=None)
    expansion = np.zeros((NY, NX), bool)
    suma = actual
    for k in orden:
        if suma >= objetivo:
            break
        i, j = divmod(int(k), NX)
        if not cand[i, j]:
            break
        expansion[i, j] = True
        suma += 0.25  # cobertura media supuesta de una celda de parque agregada [ESTIMADO]
    base.sec[expansion] = base.prim[expansion]
    base.poner(expansion, prim="FO04", dens=D_M, ev=E_INF, conf=K_BAJA, fuente=F["gautreau_atlas"])
    print(f"  Colonia: monte no ribereño hoy {actual:.0f} km², 1821–35 ≈ {objetivo:.0f} km² → {expansion.sum()} celdas de parque agregadas", flush=True)

    # 5) Blanqueales (secundario): terrazas bajas y llanas del litoral del Río Uruguay sobre
    #    sedimentos (el manual del MGAP los ubica dentro del bosque parque del litoral oeste).
    rio_uy = componente(agua_nat, (int((LAT0 + 33.0) / DLAT), int((-58.38 - LON0) / DLON)))
    d_ruy = bfs_dist(rio_uy & ~costa_mar)
    blanq = tierra & np.isin(region, [R_LITORAL, R_NORTE]) & (d_ruy <= 15) & (pend < 8) & (alt < 60) & ~np.isin(geo, list(GEO_BASALTO)) & (base.sec == 0)
    base.poner(blanq, sec="SA01")

    # 6) Palmares.
    def asignar_palmar(nucleos, area_km2, cod, apto, radio_doc):
        """Celdas aptas cerca de los lugares documentados hasta el área dada. El orden suma a la
        distancia la altura y la pendiente (los palmares de butiá están en los llanos bajos, de
        suelos pesados y húmedos) y un ruido suave de semilla fija, para que el borde siga el
        terreno y no sea un círculo (el atlas: no son polígonos de límites exactos)."""
        d = np.full((NY, NX), 1e9)
        for lo, la in nucleos:
            d = np.minimum(d, km_entre(lon, lat, lo, la))
        rng = np.random.default_rng(1726)
        g = rng.random((NY // 5 + 2, NX // 5 + 2))
        fi, fj = np.arange(NY) / 5.0, np.arange(NX) / 5.0
        i0, j0 = fi.astype(int), fj.astype(int)
        ti, tj = (fi - i0)[:, None], (fj - j0)[None, :]
        ruido = (g[i0][:, j0] * (1 - ti) * (1 - tj) + g[i0 + 1][:, j0] * ti * (1 - tj)
                 + g[i0][:, j0 + 1] * (1 - ti) * tj + g[i0 + 1][:, j0 + 1] * ti * tj)
        puntaje = d + 0.12 * np.maximum(alt, 0) + 0.08 * pend + 9.0 * ruido
        orden = np.argsort(np.where(apto, puntaje, 1e9), axis=None)
        sel = np.zeros((NY, NX), bool)
        suma = 0.0
        for k in orden:
            i, j = divmod(int(k), NX)
            if suma >= area_km2 or not apto[i, j]:
                break
            sel[i, j] = True
            suma += 1.0 - agua[i, j]
        base.sec[sel] = np.where(base.prim[sel] == C[cod], base.sec[sel], base.prim[sel])
        base.poner(sel, prim=cod, dens=D_M, ev=E_INF, conf=K_BAJA)
        base.poner(sel & (d <= radio_doc), ev=E_DOC, conf=K_MEDIA)
        return sel, d

    rocha = tierra & (depto == next(k for k, n in nombre_depto.items() if n == "Rocha"))
    apto_butia = rocha & (alt < 60) & (pend < 30) & (ban < 0.5) & (d_mar > 1.5) & ~agua_nat
    nucleos = [p for ps in PALMAR_BUTIA.values() for p in ps]
    butia, _ = asignar_palmar(nucleos, BUTIA_HA / 100.0, "PA01", apto_butia, 6.0)
    base.poner(butia, fuente=F["rosa_butia"])
    yatay_total = np.zeros((NY, NX), bool)
    paysandu = next(k for k, n in nombre_depto.items() if n == "Paysandú")
    rionegro = next(k for k, n in nombre_depto.items() if n == "Río Negro")
    apto_yatay = tierra & np.isin(depto, [paysandu, rionegro]) & (ban < 0.3) & (pend < 40)
    for nombre, (pt, ha) in PALMAR_YATAY.items():
        sel, d = asignar_palmar([pt], max(ha / 100.0, 1.0), "PA02", apto_yatay & ~yatay_total, 2.0)
        yatay_total |= sel
    base.poner(yatay_total, fuente=F["bortolini"], dosel=DOSEL["PA02"][0])
    # Palmar disperso (secundario) en el rango de extensión: suelos arenosos y bien drenados
    # cerca de los núcleos (manual del MGAP: yatay sobre suelos mejor drenados y arenosos).
    d_y = np.minimum(*[km_entre(lon, lat, *pt) for pt, _ in PALMAR_YATAY.values()])
    rango = apto_yatay & ~yatay_total & (d_y <= 8) & np.isin(geo, list(GEO_ARENISCA_DRENADA))
    cuota = int(YATAY_RANGO_KM2 * 0.1)  # una décima del rango de extensión [ESTIMADO]
    orden = np.argsort(np.where(rango, d_y, 1e9), axis=None)[:cuota]
    rsel = np.zeros((NY, NX), bool)
    rsel.flat[orden] = True
    rsel &= rango
    base.poner(rsel & ~np.isin(base.prim, [C["FO01"], C["FO04"]]), sec="PA02", conf=K_BAJA)

    # 7) Bañados y orillas.
    hum = tierra & (ban >= 0.35) & ~butia
    base.sec[hum] = np.where(agua[hum] >= 0.15, C["AQ01"], base.prim[hum])
    base.poner(hum, prim="HU01", dens=D_AUS, ev=E_INF, conf=K_MEDIA, fuente=F["ide_banados"], dosel=0)
    # Planicies bajas de la cuenca de la Laguna Merín: los bañados eran más extensos antes de
    # drenar para el arroz (desde 1950, atlas §5). Inferencia de confianza baja.
    merin = tierra & ~hum & ~butia & (lon > -54.6) & np.isin(region, [R_ESTE, R_SIERRAS]) & (alt < 15) & (pend < 6) & (np.isin(geo, list(GEO_ALUVIAL | {73, 74})) | (ban > 0.05))
    base.sec[merin] = base.prim[merin]
    base.poner(merin, prim="HU01", dens=D_AUS, ev=E_INF, conf=K_BAJA, fuente=F["ide_mdt"], dosel=0)
    orilla = tierra & (agua >= 0.2) & ~np.isin(base.prim, [C["HU01"]]) & (base.sec == 0)
    base.poner(orilla, sec="AQ01")

    # 8) Costa: arenales y dunas; monte psamófilo donde hay monte (ya puesto arriba).
    arena = tierra & (d_mar <= 2) & (np.isin(geo, list(GEO_ARENA_COSTA)) | ((d_mar <= 1) & (alt < 10))) & (base.prim != C["FO05"])
    keep = arena & np.isin(base.prim, [C["HU01"], C["PA01"]])
    arena &= ~keep
    base.sec[arena] = np.where(mon[arena] >= 0.02, C["FO05"], 0)
    base.poner(arena, prim="CO01", dens=D_AUS, ev=E_INF, conf=np.where(np.isin(geo, list(GEO_ARENA_COSTA)), K_MEDIA, K_BAJA), fuente=F["dinamige"], dosel=0)

    # Celdas sin árbol: densidad "muy baja" (árboles sueltos posibles) en la pradera; "ausente"
    # solo en agua, dunas y bañados (los testimonios de 1726 describen praderas casi sin árboles).
    assert ((base.prim != 0) | ~tierra).all()
    # El parque agregado en Colonia que otra regla pisó después (costa, bañado, palmar) no cuenta.
    expansion &= base.prim == C["FO04"]

    # --- Capas por período ---------------------------------------------------------------------
    capas = [(1700, 1859, base)]
    # 1860–1899: el monte no ribereño agregado en Colonia entra en un intervalo sin datos (entre
    # los planos de 1821–35 y la cartografía de 1966 no hay cifras: no se interpola).
    c1860 = base.copia()
    c1860.poner(expansion, ev=E_DES, conf=K_BAJA)
    capas.append((1860, 1899, c1860))
    # 1900–1949: forestación costera de Canelones (pinos marítimos, acacias): plantación, no nativo.
    c1900 = c1860.copia()
    d_f = np.minimum(*[km_entre(lon, lat, lo, la) for lo, la in FORESTACION_CANELONES])
    apto_f = tierra & (d_mar <= 2.5) & (depto == next(k for k, n in nombre_depto.items() if n == "Canelones"))
    orden = np.argsort(np.where(apto_f, d_f, 1e9), axis=None)
    forest = np.zeros((NY, NX), bool)
    suma = 0.0
    for k in orden:
        i, j = divmod(int(k), NX)
        if suma >= FORESTACION_HA / 100.0 or not apto_f[i, j]:
            break
        forest[i, j] = True
        suma += 1.0 - agua[i, j]
    c1900.sec[forest] = np.where(base.prim[forest] == C["PL01"], 0, base.prim[forest])
    c1900.poner(forest, prim="PL01", dens=D_ND, dosel=0, ev=E_DOC, conf=K_MEDIA, fuente=F["porcile_atlas"])
    capas.append((1900, 1949, c1900))
    # 1950–1984: Colonia según la cartografía de 1966 (monte no ribereño ~107 km²): las celdas
    # agregadas vuelven a pradera. Bañados de la cuenca de la Merín: drenaje y arrozales sin dato
    # por celda (desconocido).
    c1950 = c1900.copia()
    c1950.prim[expansion] = c1950.sec[expansion]
    c1950.poner(expansion, sec=0, dens=D_MB, ev=E_DOC, conf=K_BAJA, fuente=F["gautreau_atlas"])
    hum_merin = np.isin(c1950.prim, [C["HU01"]]) & (lon > -54.6) & np.isin(region, [R_ESTE, R_SIERRAS])
    c1950.poner(hum_merin, ev=E_DES, conf=K_BAJA, fuente=F["atlas"])
    capas.append((1950, 1984, c1950))
    # 1985–2025: lo observado (monte 2021, pastizales, palmares de hoy, localidades); donde hoy no
    # hay pastizal ni monte, cobertura transformada sin dato por celda (ver MapBiomas por depto).
    c1985 = c1950.copia()
    monte_cods = [C[k] for k in ("FO01", "FO02", "FO03", "FO04", "FO05", "AN02")]
    c1985.poner(hay_monte & np.isin(c1985.prim, monte_cods), ev=E_DOC, conf=K_ALTA, fuente=F["dgf2021"])
    pasto_obs = tierra & (obs >= 0.5) & np.isin(c1985.prim, [C[k] for k in ("PR01", "PR02", "PR03", "PR04")])
    c1985.prim[pasto_obs] = pr_obs[pasto_obs]
    c1985.poner(pasto_obs, ev=E_DOC, conf=K_ALTA, fuente=F["pastizales"])
    transf = tierra & (obs < 0.2) & (mon < 0.08) & np.isin(c1985.prim, [C[k] for k in ("PR01", "PR02", "PR03", "PR04", "SH01", "AN02")])
    c1985.sec[transf] = c1985.prim[transf]
    c1985.poner(transf, prim="TR01", dens=D_ND, ev=E_DES, conf=K_BAJA, fuente=F["mapbiomas"])
    c1985.poner(butia | yatay_total, ev=E_DOC, conf=K_MEDIA)
    c1985.poner(c1985.prim == C["HU01"], ev=E_DOC, conf=K_MEDIA, fuente=F["ide_banados"])
    c1985.poner((c1985.prim == C["HU01"]) & (ban < 0.35), ev=E_DES, conf=K_BAJA)
    # Localidades de hoy (MTOP).
    try:
        import construir_pais as cp
        loc = json.load(open(os.path.join(cache, "mtop_localidades.geojson")))["features"]
        urb = cp.fraccion(cp.rasterizar(loc, 4, 255), 4) / 255.0
        urbano = tierra & (urb >= 0.5)
        c1985.sec[urbano] = 0
        c1985.poner(urbano, prim="UR01", dens=D_ND, dosel=0, ev=E_DOC, conf=K_ALTA, fuente=F["mtop"])
        print(f"  urbano (localidades MTOP): {urbano.sum()} celdas", flush=True)
    except Exception as e:  # noqa: BLE001
        print(f"  sin localidades: {e}", flush=True)
    capas.append((1985, 2100, c1985))

    for d, h, cp_ in capas:
        assert ((cp_.prim != 0) | ~tierra).all(), f"capa {d}: tierra sin código"
    tam = empaquetar(os.path.join(PAIS, "ecosistemas.bin"), capas)
    print(f"  ecosistemas.bin: {tam / 1024:.0f} KB", flush=True)
    est = {}
    for d, h, cp_ in capas:
        cuenta = {}
        for n, cod, _, _ in LEYENDA + EXTENSIONES:
            k = int((cp_.prim == n).sum())
            if k:
                cuenta[cod] = k
        est[f"{d}-{h}"] = cuenta
    otros = {
        "colonia_monte_no_ribereno_km2": {"hoy_dgf2021": round(actual, 1), "objetivo_1821_35": round(objetivo, 1), "celdas_agregadas": int(expansion.sum())},
        "forestacion_canelones_celdas": int(forest.sum()),
        "butia_celdas": int(butia.sum()), "yatay_celdas": int(yatay_total.sum()), "yatay_disperso_celdas": int(rsel.sum()),
    }
    return capas, est, otros


# --- Mapa de la ciudad: entorno (100 m) y entorno amplio (500 m) ---------------------------------
def leer_u16(ruta, n, esc, off):
    return np.frombuffer(open(ruta, "rb").read(), "<u2").reshape(n, n) / esc - off


def leer_u8(ruta, n, canales=1):
    a = np.frombuffer(open(ruta, "rb").read(), np.uint8)
    return a.reshape(n, n, canales) if canales > 1 else a.reshape(n, n)


def lonlat_grilla(desde, paso, n):
    f = desde + np.arange(n) * paso
    zz, xx = np.meshgrid(f, f, indexing="ij")
    lat, lon = cm.a_geo(xx, zz)
    return lon, lat, xx, zz


def pais_en(lon, lat):
    """Muestras de la grilla del país (más cercana) en puntos lon/lat."""
    j = np.clip(np.round((lon - LON0) / DLON).astype(int), 0, NX - 1)
    i = np.clip(np.round((LAT0 - lat) / DLAT).astype(int), 0, NY - 1)
    return i, j


def construir_ciudad(past, capas_pais):
    meta = json.load(open(os.path.join(MVD, "meta.json")))
    base_p = capas_pais[0][2]
    cap = np.frombuffer(open(os.path.join(PAIS, "capas.bin"), "rb").read(), np.uint8).reshape(NY, NX, 8)
    geo_p = np.frombuffer(open(os.path.join(RAIZ, "assets", "datos", "uruguay", "geologia_pais.bin"), "rb").read(), np.uint8).reshape(NY, NX).astype(np.int32) - 1
    mapa = json.load(open(os.path.join(PAIS, "mapa.json")))
    nombre_depto = {d["id"]: d["nombre"] for d in mapa["departamentos"]}
    salidas = {}

    for nombre, grilla, archivo in (("entorno", meta["contexto"], "ecosistemas.bin"), ("amplio", meta["amplio"], "ecosistemas_amplio.bin")):
        n, desde, paso = grilla["muestras"], grilla["desde_m"], grilla["paso_m"]
        print(f"Ciudad, {nombre}: {n} × {n} muestras de {paso:.0f} m…", flush=True)
        lon, lat, xx, zz = lonlat_grilla(desde, paso, n)
        pi, pj = pais_en(lon, lat)
        depto = cap[pi, pj, 2]
        # La costa a 1 km cae a veces en una celda de agua del país: departamento del vecino.
        depto, _ = vecino_mas_cercano(depto, depto > 0, 12)
        region = np.full((n, n), 255, np.uint8)
        for k, nm in nombre_depto.items():
            region[depto == k] = REGION_DEPTO[nm]
        geo = geo_p[pi, pj]
        if nombre == "entorno":
            h = leer_u16(os.path.join(MVD, "contexto.bin"), n, 200.0, 50.0)
            bio = leer_u8(os.path.join(MVD, "biomas_contexto.bin"), n)
            monte = leer_u8(os.path.join(MVD, "bosque_contexto.bin"), n) / 255.0
            roca = leer_u8(os.path.join(MVD, "rocas_contexto.bin"), n) / 255.0
            ip = leer_u8(os.path.join(MVD, "productividad_contexto.bin"), n).astype(np.float32)
            banado = (bio == 2).astype(np.float32)
            agua = (h <= 0.0) | (bio == 4)
            fuente_monte = F["bosque1726"]
        else:
            h = leer_u16(os.path.join(MVD, "amplio.bin"), n, 100.0, 50.0)
            cob = leer_u8(os.path.join(MVD, "amplio_cobertura.bin"), n, 2)
            banado = cob[..., 0] / 255.0
            monte = cob[..., 1] / 255.0
            bio = np.zeros((n, n), np.uint8)
            roca = np.zeros((n, n))
            ip = np.full((n, n), 100.0)
            agua = h <= 0.0
            fuente_monte = F["dgf2021"]
        argentina = ~agua & (lat < -34.85) & (lon < -56.5)
        tierra = ~agua & (depto > 0) & ~argentina
        argentina |= ~agua & ~tierra
        gz, gx = np.gradient(h, paso)
        pend = np.hypot(gx, gz) * 1000.0  # ‰
        r = max(1, int(round(1500 / paso)))
        rel = ventana(h, min(r, 5), np.maximum) - ventana(h, min(r, 5), np.minimum)
        bajo = h - ventana(h, min(r, 5), np.minimum)
        mar = componente(agua, (n - 1, n // 2))
        d_mar = bfs_dist(mar, paso / 1000.0)  # km al Río de la Plata / mar
        obs, clase, clase2 = past.por_celda(lon, lat, paso / cm.KX, paso / cm.KY, 3)
        pr_obs = codigo_pastizal(clase)

        cp = Capa((n, n))
        cp.poner(agua, prim=0, dens=D_AUS, ev=E_DOC, conf=K_ALTA, fuente=F["ide_mdt"])
        cp.poner(argentina, prim=0, dens=D_ND, ev=E_DES, conf=K_BAJA, fuente=F["ninguna"])
        # Pradera: observada; si no, por suelos (CONEAT en el entorno) o la del país.
        valido = tierra & (obs >= 0.3) & (pr_obs > 0)
        pr_p = base_p.prim[pi, pj]
        pr_p = np.where(np.isin(pr_p, [C["PR01"], C["PR02"], C["PR03"], C["PR04"]]), pr_p, C["PR01"])
        pr = np.where(valido, pr_obs, pr_p)
        fuente = np.where(valido, F["pastizales"], F["atlas"])
        cp.poner(tierra, prim=pr, dens=D_MB, ev=E_INF, conf=np.where(valido, K_MEDIA, K_BAJA), fuente=fuente)
        if nombre == "entorno":
            # Profundidad del suelo (CONEAT): roca a la vista o productividad baja = superficial.
            sup = tierra & ~valido & ((roca >= 0.08) | ((ip > 0) & (ip < 55)))
            cp.poner(sup, prim="PR02", conf=K_MEDIA, fuente=F["coneat"])
            prof = tierra & ~valido & ~sup & (ip >= 110)
            cp.poner(prof, prim="PR03", conf=K_MEDIA, fuente=F["coneat"])
            gen = tierra & ~valido & ~sup & ~prof
            cp.poner(gen, prim="PR01", conf=K_BAJA, fuente=F["coneat"])
        else:
            ralo = tierra & ~valido & (pend >= 60) & np.isin(geo, list(GEO_CRISTALINO))
            cp.poner(ralo, prim="PR02", conf=K_BAJA, fuente=F["dinamige"])
        # Chircal (secundario): laderas de pendiente media sin monte (sur y centro).
        chir = tierra & np.isin(region, [R_SUR, R_CENTRO, R_SIERRAS]) & (pend >= 30) & (pend < 90) & (monte < 0.05)
        cp.poner(chir, sec="SH01")
        if nombre == "entorno":
            cp.poner(tierra & (roca >= 0.15) & (cp.sec == 0), sec="SH01")
        # Monte.
        hay = tierra & (monte >= 0.06)
        sierra = np.isin(geo, list(GEO_CRISTALINO)) & ((pend >= 70) | (rel >= 60)) & np.isin(region, [R_SIERRAS, R_ESTE])
        fo3 = hay & sierra & (bajo > 12)
        psamo = hay & (d_mar <= 2.0) & (np.isin(geo, list(GEO_ARENA_COSTA)) | (h < 12)) & ~fo3
        fo1 = hay & ~fo3 & ~psamo & ((bajo <= 12) | (nombre == "entorno"))
        resto = hay & ~fo3 & ~psamo & ~fo1
        fo4 = resto & np.isin(region, [R_LITORAL, R_CENTRO]) & (pend < 50)
        an2 = resto & ~fo4
        dens = np.where(monte >= 0.4, D_A, np.where(monte >= 0.1, D_M, D_B)).astype(np.uint8)
        for m, cod, um in ((fo1, "FO01", 0.35), (fo3, "FO03", 0.3), (psamo, "FO05", 0.25), (fo4, "FO04", 0.2), (an2, "AN02", 0.2)):
            pm = m & (monte >= um)
            sm = m & (monte < um)
            cp.sec[pm] = cp.prim[pm]
            cp.poner(pm, prim=cod, dens=dens, ev=E_INF, conf=K_MEDIA, fuente=fuente_monte)
            cp.poner(sm, sec=cod, dens=dens, ev=E_INF, conf=K_BAJA, fuente=fuente_monte)
            if cod in DOSEL:
                cp.dosel[pm] = DOSEL[cod][0]
        cp.dosel[(cp.prim == C["FO01"]) & np.isin(region, [R_SUR, R_ESTE, R_SIERRAS])] = DOSEL["FO01_sur"][0]
        # Ecotono en el borde del monte de galería (entorno, 100 m): cobertura intermedia.
        if nombre == "entorno":
            eco = tierra & (monte >= 0.12) & (monte < 0.35) & (cp.prim != C["FO05"])
            cp.sec[eco] = C["FO01"]
            cp.poner(eco, prim="AN02", conf=K_BAJA)
        # Bañados.
        hum = tierra & (banado >= 0.35)
        cp.sec[hum] = cp.prim[hum]
        cp.poner(hum, prim="HU01", dens=D_AUS, dosel=0, ev=E_INF, conf=K_MEDIA, fuente=F["ide_biomas"] if nombre == "entorno" else F["ide_banados"])
        # Costa: arenales y dunas.
        playa = tierra & np.isin(bio, [1, 3])
        cp.poner(playa, prim="CO01", sec=0, dens=D_AUS, dosel=0, ev=E_INF, conf=K_MEDIA, fuente=F["ide_biomas"])
        duna = tierra & ~playa & (d_mar <= (0.45 if nombre == "entorno" else 1.6)) & (h < 12) & ~hum & (cp.prim != C["FO05"])
        duna &= np.isin(geo, list(GEO_ARENA_COSTA)) | (d_mar <= (0.25 if nombre == "entorno" else 0.6))
        cp.sec[duna] = np.where(monte[duna] >= 0.03, C["FO05"], 0)
        cp.poner(duna, prim="CO01", dens=D_AUS, dosel=0, ev=E_INF, conf=K_BAJA, fuente=F["dinamige"])

        capas = [(1700, 1899, cp)]
        # Forestación costera de Canelones (desde 1900): lo que de la capa del país cae acá.
        p1900 = capas_pais[2][2]
        plant = tierra & (p1900.prim[pi, pj] == C["PL01"])
        c2 = cp.copia()
        c2.sec[plant] = cp.prim[plant]
        c2.poner(plant, prim="PL01", dens=D_ND, dosel=0, ev=E_DOC, conf=K_MEDIA, fuente=F["porcile_atlas"])
        capas.append((1900, 2100, c2))
        tam = empaquetar(os.path.join(MVD, archivo), capas)
        cuenta = {cod: int((cp.prim == k).sum()) for k, cod, _, _ in LEYENDA + EXTENSIONES if (cp.prim == k).any()}
        print(f"  {archivo}: {tam / 1024:.0f} KB · {cuenta} · plantación desde 1900: {plant.sum()} muestras", flush=True)
        salidas[nombre] = {"archivo": archivo, "desde_m": desde, "paso_m": paso, "muestras": n, "conteo_1750": cuenta, "plantacion_1900_muestras": int(plant.sum())}
    return salidas


def mapbiomas_departamentos(ruta):
    if not ruta or not os.path.exists(ruta):
        return None
    import openpyxl
    wb = openpyxl.load_workbook(ruta, read_only=True)
    filas = list(wb["Departamentos"].iter_rows(values_only=True))[1:]
    out = {}
    for area, _cid, clase, _t, dep, anio in filas:
        if anio not in (1985.0, 2025.0) or clase not in ("Bosque Natural", "Pastizal natural", "Area humeda natural", "Agricultura", "Pino", "Eucaliptus", "Area Urbana", "Pastura"):
            continue
        out.setdefault(dep.title(), {}).setdefault(clase, {})[str(int(anio))] = round(float(area), 1)
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--pastizales", required=True, help="Pastizales del Uruguay.tif (MGAP-DGRN)")
    ap.add_argument("--cache", required=True, help="caché del mapa del país (mtop_localidades.geojson)")
    ap.add_argument("--mapbiomas", help="mapbiomas-uruguay-collection-4-lulc.xlsx (estadísticas)")
    a = ap.parse_args()
    past = Pastizales(a.pastizales)
    capas, est, otros = construir_pais(past, a.cache)
    ciudad = construir_ciudad(past, capas)
    nombres_capas = [("1750–1800", "Paisaje colonial (base). Se usa desde el inicio del juego hasta 1859: Gautreau no encuentra cambios radicales en la distribución general antes de mediados del siglo XIX."),
                     ("1860–1900", "Transformación rural: sin tasa uniforme de deforestación (atlas §5). Igual a la base salvo el parque agregado en Colonia, que pasa a desconocido."),
                     ("1900–1950", "Forestación costera de Canelones (Las Toscas, Parque del Plata): plantación, no nativo."),
                     ("1950–2000", "Colonia según la cartografía de 1966; bañados de la cuenca de la Merín sin dato por celda (drenaje y arrozales)."),
                     ("1985–2025", "Observación actual: monte nativo 2021, pastizales, palmares, localidades; lo demás, cobertura transformada sin dato por celda.")]
    doc = {
        "descripcion": "Ecosistemas con la leyenda del Atlas ecológico e histórico del Uruguay (v1.0). Ver docs/sistemas/atlas-ecologico.md.",
        "grilla": {"lon_oeste": LON0, "lat_norte": LAT0, "dlon": DLON, "dlat": DLAT, "columnas": NX, "filas": NY, "archivo": "ecosistemas.bin"},
        "formato": "ECO1: u16 ancho, u16 alto, u8 bytes por celda (6), u8 capas; por capa i16 desde, i16 hasta, u32 largo y la grilla zlib. Celda: primario, secundario, densidad (0 ausente..4 alta, 5 sin dato), dosel (dm, 0 = sin fuente), evidencia | confianza << 4, fuente.",
        "leyenda": [{"n": n, "codigo": c, "nombre": nm, "color": list(col), "atlas": True} for n, c, nm, col in LEYENDA]
                   + [{"n": n, "codigo": c, "nombre": nm, "color": list(col), "atlas": False} for n, c, nm, col in EXTENSIONES],
        "densidad": DENS, "evidencia": EVID, "confianza": CONF,
        "fuentes": FUENTES,
        "capas": [{"desde": d, "hasta": h, "periodo": nombres_capas[k][0], "nota": nombres_capas[k][1]} for k, (d, h, _) in enumerate(capas)],
        "regiones": REGIONES,
        "region_departamento": {k: REGIONES[v] for k, v in REGION_DEPTO.items()},
        "dosel": {k: {"dm": v[0], "rango": v[1], "fuente": v[2]} for k, v in DOSEL.items()},
        "palmares": {"butia": {"nucleos": PALMAR_BUTIA, "ha": BUTIA_HA}, "yatay": {k: {"pos": v[0], "ha_2013": v[1]} for k, v in PALMAR_YATAY.items()},
                     "yatay_no_ubicados": "Santo Domingo (826 ha en 2013) sin ubicación en las fuentes leídas; Mujica y Porrúa (Chebataroff 1960) desaparecidos."},
        "quebradas_documentadas": QUEBRADAS,
        "estadisticas_por_capa": est,
        "otros": otros,
        "mapbiomas_departamentos_km2": mapbiomas_departamentos(a.mapbiomas),
        "nota_mapbiomas": "Áreas por departamento y clase de MapBiomas Uruguay (colección 4), 1985 y 2025, en km² (unidad deducida por los totales). No se compara con las capas históricas (métodos distintos).",
    }
    cm.guardar_json(os.path.join(PAIS, "ecosistemas.json"), doc, compacto=False)
    ruta_meta = os.path.join(MVD, "meta.json")
    meta = json.load(open(ruta_meta))
    meta["ecosistemas"] = {"codificacion": doc["formato"], "leyenda_y_fuentes": "assets/mapas/uruguay/ecosistemas.json", **ciudad}
    cm.guardar_json(ruta_meta, meta, compacto=False)
    print("Listo.", flush=True)


if __name__ == "__main__":
    main()
