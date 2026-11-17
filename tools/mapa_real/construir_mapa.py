#!/usr/bin/env python3
"""
Construye el mapa real de Montevideo para el prototipo (y luego para el motor de Rust).

Fuentes (ver docs/sistemas/mapa-real.md, atribución):
  --fuente ide    (por omisión) el Modelo Digital del Terreno del Uruguay de 30 m de la
                  Infraestructura de Datos Espaciales (IDE Uruguay), servicio WCS. Mide el suelo
                  desnudo, sin edificios ni árboles: la Ciudad Vieja da 20 m y no 33.
  --fuente aws    los "Terrain Tiles" públicos de AWS (terrarium, Mapzen/Tilezen: SRTM,
                  GMTED2010, ETOPO1…), que miden la superficie con techos y copas.

  --hidrografia ide  (por omisión con --fuente ide) hidrografía oficial de la IDE por WFS:
                  cursos de agua, playas, bañados, áreas húmedas, bancos de arena, lagunas,
                  islas, cerros y cuchillas (capas nacional 1:50.000 y urbana). Las capas se
                  guardan como GeoJSON en --cache.
  --hidrografia relieve  arroyos calculados del relieve (como antes) e islas a mano.

Salida en assets/mapas/montevideo/:
  altura.bin        alturas del mapa jugable: uint16 little-endian, fila por fila (norte → sur),
                    columna por columna (oeste → este); metros = valor / 200 − 50.
  contexto.bin      lo mismo, más grueso, para el anillo exterior del terreno.
  biomas.bin        biomas oficiales, uint8 en la grilla de altura.bin: 0 nada, 1 playa,
                    2 bañado / área húmeda, 3 banco de arena, 4 laguna, 5 isla.
  biomas_contexto.bin  lo mismo en la grilla de contexto.bin.
  arroyos.json      cauces del mapa jugable: {"arroyos": [[[x, z, ancho], …], …],
                    "nombres": [nombre o null, …], "fuente": ["ide" | "relieve", …]}.
  arroyos_contexto.json  cauces del entorno con nombre o anchos (solo el motor los usa).
  nombres.json      arroyos, cerros, cuchillas e islas con nombre, en metros locales (rótulos).
  meta.json         origen geográfico, escala, tamaños, codificación, fuentes y atribución.
Además guarda docs/capturas/mapa-real-relieve.jpg como imagen de control (no la usa el juego).

Coordenadas locales: x hacia el este y z hacia el sur, en metros, con (0, 0) en la esquina
noroeste del mapa jugable (como el resto del juego).

Uso:
  python3 tools/mapa_real/construir_mapa.py --cache /ruta/vacia/para/tiles
"""
import argparse
import heapq
import io
import json
import math
import os
import struct
import sys
import urllib.parse
import urllib.request

import numpy as np
from PIL import Image

# --- Ventana del mapa -------------------------------------------------------------------
MAP_SIZE = 24000.0      # metros
STEP = 20.0             # resolución del mapa jugable
OUTER = 25000.0         # entorno: lo que se ve desde el mapa (el motor dibuja ~24 km alrededor)
OUTER_STEP = 100.0
LAT_N = -34.778         # borde norte
LON_W = -56.325         # borde oeste
ZOOM = 13
URL = "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{z}/{x}/{y}.png"

IDE_WCS = ("https://mapas.ide.uy/geoserver-raster/wcs?service=WCS&version=2.0.1&request=GetCoverage"
           "&coverageId=raster__mdt_pais_30m_4326&format=image/tiff"
           "&subset=Lat({lat_s},{lat_n})&subset=Long({lon_w},{lon_e})")
# Datos vectoriales de la IDE (GeoServer). Se piden ya reproyectados a EPSG:4326.
IDE_WFS = "https://mapas.ide.uy/geoserver-vectorial/wfs?service=WFS&version=2.0.0&request=GetFeature"

# Puntos de referencia (lat, lon).
PLAZA_MATRIZ = (-34.9068, -56.2036)
CERRO = (-34.8886, -56.2602)

# Islas que el modelo del terreno no trae (el MDT deja el agua sin dato): nombre, centro x, z
# (m locales), largo, ancho, altura (m) y giro (grados). La posición sale del relieve SRTM,
# donde sí aparecen; el tamaño es aproximado.
ISLAS = [
    ("Isla de las Ratas (San José)", 8294.0, 12280.0, 180.0, 110.0, 5.0, 30.0),
    # Frente a Carrasco, ya en el entorno: islote bajo y rocoso (faro desde el siglo XIX).
    ("Isla de Flores", 36054.0, 18493.0, 800.0, 300.0, 6.0, 70.0),
]

# Umbral de cuenca para considerar un arroyo (km²).
# Con 0,6 km² entran las cañadas: los arroyitos que cruzan toda la campaña uruguaya.
ARROYO_KM2 = 0.6
ARROYO_MIN_LEN = 250.0


def ancho_cauce(km2):
    """Ancho del agua (m) según la cuenca: una cañada de 0,6 km² ≈ 2,6 m; un arroyo de
    50 km² ≈ 11 m; el Miguelete o el Pantanoso (más de 100 km²) ≈ 15–19 m."""
    return 1.5 + 1.4 * math.sqrt(km2)


def escalas(lat0):
    """Metros por grado de latitud y de longitud a la latitud lat0."""
    p = math.radians(lat0)
    ky = 111132.954 - 559.822 * math.cos(2 * p) + 1.175 * math.cos(4 * p)
    kx = 111412.84 * math.cos(p) - 93.5 * math.cos(3 * p)
    return kx, ky


LAT0 = LAT_N - (MAP_SIZE / 2) / 110950.0
KX, KY = escalas(LAT0)


def a_local(lat, lon):
    return (lon - LON_W) * KX, (LAT_N - lat) * KY


def a_geo(x, z):
    return LAT_N - z / KY, LON_W + x / KX


# --- Tiles ---------------------------------------------------------------------------------
def pixel_global(lat, lon, z=ZOOM):
    n = 2 ** z * 256
    px = (lon + 180.0) / 360.0 * n
    py = (1.0 - math.asinh(math.tan(math.radians(lat))) / math.pi) / 2.0 * n
    return px, py


def bajar_tiles(cache, x0, x1, y0, y1):
    tiles = {}
    for ty in range(y0, y1 + 1):
        for tx in range(x0, x1 + 1):
            ruta = os.path.join(cache, f"{ZOOM}_{tx}_{ty}.png")
            if not os.path.exists(ruta):
                url = URL.format(z=ZOOM, x=tx, y=ty)
                with urllib.request.urlopen(url, timeout=60) as r:
                    datos = r.read()
                with open(ruta, "wb") as f:
                    f.write(datos)
            img = np.asarray(Image.open(ruta).convert("RGB"), dtype=np.float64)
            tiles[(tx, ty)] = img[:, :, 0] * 256.0 + img[:, :, 1] + img[:, :, 2] / 256.0 - 32768.0
    return tiles


def mosaico(cache):
    lat_s = LAT_N - (MAP_SIZE + OUTER) / KY
    lat_n = LAT_N + OUTER / KY
    lon_w = LON_W - OUTER / KX
    lon_e = LON_W + (MAP_SIZE + OUTER) / KX
    px0, py0 = pixel_global(lat_n, lon_w)
    px1, py1 = pixel_global(lat_s, lon_e)
    x0, x1 = int(px0 // 256), int(px1 // 256)
    y0, y1 = int(py0 // 256), int(py1 // 256)
    tiles = bajar_tiles(cache, x0, x1, y0, y1)
    alto = np.zeros(((y1 - y0 + 1) * 256, (x1 - x0 + 1) * 256))
    for (tx, ty), t in tiles.items():
        alto[(ty - y0) * 256:(ty - y0 + 1) * 256, (tx - x0) * 256:(tx - x0 + 1) * 256] = t
    return alto, x0 * 256, y0 * 256, len(tiles)


def muestrear(alto, ox, oy, xs, zs):
    """Alturas en la grilla local (xs, zs) por interpolación bilineal en el mosaico."""
    zz, xx = np.meshgrid(zs, xs, indexing="ij")
    lat = LAT_N - zz / KY
    lon = LON_W + xx / KX
    n = 2 ** ZOOM * 256
    px = (lon + 180.0) / 360.0 * n - ox
    py = (1.0 - np.arcsinh(np.tan(np.radians(lat))) / np.pi) / 2.0 * n - oy
    px -= 0.5
    py -= 0.5
    i0 = np.clip(np.floor(py).astype(int), 0, alto.shape[0] - 2)
    j0 = np.clip(np.floor(px).astype(int), 0, alto.shape[1] - 2)
    fy = np.clip(py - i0, 0, 1)
    fx = np.clip(px - j0, 0, 1)
    a = alto[i0, j0] * (1 - fx) + alto[i0, j0 + 1] * fx
    b = alto[i0 + 1, j0] * (1 - fx) + alto[i0 + 1, j0 + 1] * fx
    return a * (1 - fy) + b * fy


# --- MDT de la IDE Uruguay -------------------------------------------------------------------
def bajar_mdt(cache):
    """Recorte del MDT nacional (EPSG:4326, float32, -32767 = sin dato: río y mar)."""
    lat_s = LAT_N - (MAP_SIZE + OUTER) / KY - 0.01
    lat_n = LAT_N + OUTER / KY + 0.01
    lon_w = LON_W - OUTER / KX - 0.01
    lon_e = LON_W + (MAP_SIZE + OUTER) / KX + 0.01
    ruta = os.path.join(cache, f"mdt_ide_mvd_{int(OUTER)}.tif")
    if not os.path.exists(ruta):
        url = IDE_WCS.format(lat_s=f"{lat_s:.5f}", lat_n=f"{lat_n:.5f}", lon_w=f"{lon_w:.5f}", lon_e=f"{lon_e:.5f}")
        datos = _bajar(url, timeout=300)
        with open(ruta, "wb") as f:
            f.write(datos)
    im = Image.open(ruta)
    a = np.asarray(im, dtype=np.float64)
    # Georreferencia (ModelTransformationTag): lon = lon0 + j·d, lat = lat0 − i·d.
    t = im.tag_v2[34264]
    return a, t[3], t[7], t[0]


def _bajar(url, timeout=240, intentos=10):
    """GET con reintentos: los servidores de la IDE cortan conexiones a menudo."""
    import time
    ultimo = None
    for k in range(intentos):
        try:
            with urllib.request.urlopen(url, timeout=timeout) as r:
                return r.read()
        except Exception as e:  # noqa: BLE001 (red inestable: cualquier error se reintenta)
            ultimo = e
            print(f"    reintento {k + 1}: {e}", flush=True)
            time.sleep(3 + 2 * k)
    raise RuntimeError(f"no se pudo bajar {url}: {ultimo}")


def bbox_geo():
    """Rectángulo (lon_o, lat_s, lon_e, lat_n) del mapa más el entorno."""
    return (LON_W - OUTER / KX - 0.005, LAT_N - (MAP_SIZE + OUTER) / KY - 0.005,
            LON_W + (MAP_SIZE + OUTER) / KX + 0.005, LAT_N + OUTER / KY + 0.005)


def bajar_capa_wfs(cache, capa):
    """Todos los objetos de una capa WFS de la IDE dentro del mapa y su entorno, en EPSG:4326
    (lon, lat[, z]), paginando de a 5000. Se guarda como GeoJSON en la caché."""
    ruta = os.path.join(cache, "wfs_" + capa.replace(":", "__") + f"_{int(OUTER)}.geojson")
    if os.path.exists(ruta):
        with open(ruta) as f:
            return json.load(f)["features"]
    lo, la_s, le, la_n = bbox_geo()
    feats, vistos, inicio, pagina = [], set(), 0, 5000
    while True:
        url = (f"{IDE_WFS}&typeNames={capa}&outputFormat=application/json&srsName=EPSG:4326"
               f"&bbox={lo:.5f},{la_s:.5f},{le:.5f},{la_n:.5f},EPSG:4326"
               f"&count={pagina}&startIndex={inicio}")
        d = json.loads(_bajar(url))
        nuevos = [f for f in d.get("features", []) if f.get("id") not in vistos]
        for f in nuevos:
            vistos.add(f.get("id"))
        feats += nuevos
        if len(d.get("features", [])) < pagina:
            break
        inicio += pagina
    with open(ruta, "w") as f:
        json.dump({"type": "FeatureCollection", "features": feats}, f)
    return feats


def muestrear_mdt(mdt, xs, zs):
    a, lon0, lat0, d = mdt
    zz, xx = np.meshgrid(zs, xs, indexing="ij")
    lat = LAT_N - zz / KY
    lon = LON_W + xx / KX
    fi = (lat0 - lat) / d - 0.5
    fj = (lon - lon0) / d - 0.5
    i0 = np.clip(np.floor(fi).astype(int), 0, a.shape[0] - 2)
    j0 = np.clip(np.floor(fj).astype(int), 0, a.shape[1] - 2)
    fy = np.clip(fi - i0, 0, 1)
    fx = np.clip(fj - j0, 0, 1)
    v = [a[i0, j0], a[i0, j0 + 1], a[i0 + 1, j0], a[i0 + 1, j0 + 1]]
    nod = np.zeros(zz.shape, bool)
    for k in v:
        nod |= k < -1000
    # Sin dato = agua. Cerca de la costa se interpola solo con las muestras de tierra.
    w = [(1 - fx) * (1 - fy), fx * (1 - fy), (1 - fx) * fy, fx * fy]
    num = sum(np.where(k > -1000, k, 0.0) * wk for k, wk in zip(v, w))
    den = sum(np.where(k > -1000, wk, 0.0) for k, wk in zip(v, w))
    h = np.where(den > 0.5, num / np.maximum(den, 1e-9), -1.0)
    return h


def suavizar_suave(h):
    """Media de 3×3 solo en tierra: quita el escalonado del MDT (que viene de curvas de nivel)
    sin aplastar el relieve ni mover la costa."""
    from numpy.lib.stride_tricks import sliding_window_view as ventanas
    p = np.pad(h, 1, mode="edge")
    media = ventanas(p, (3, 3)).mean(axis=(-2, -1))
    out = h.copy()
    ok = (h > 0.5) & (media > 0.5)
    out[ok] = media[ok]
    return out


# --- Arroyos desde el relieve ------------------------------------------------------------
VEC = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]


def rellenar(h, agua):
    """Priority-flood con épsilon: elimina pozos para que el agua siempre baje al río."""
    ny, nx = h.shape
    lleno = np.full_like(h, np.inf)
    visto = np.zeros(h.shape, bool)
    cola = []
    for i in range(ny):
        for j in range(nx):
            if agua[i, j] or i in (0, ny - 1) or j in (0, nx - 1):
                lleno[i, j] = h[i, j]
                visto[i, j] = True
                heapq.heappush(cola, (h[i, j], i, j))
    while cola:
        e, i, j = heapq.heappop(cola)
        for di, dj in VEC:
            a, b = i + di, j + dj
            if 0 <= a < ny and 0 <= b < nx and not visto[a, b]:
                visto[a, b] = True
                lleno[a, b] = max(h[a, b], e + 1e-4)
                heapq.heappush(cola, (lleno[a, b], a, b))
    return lleno


def arroyos(h, step):
    agua = h <= 0.3
    lleno = rellenar(h, agua)
    ny, nx = h.shape
    # Dirección D8: el vecino más bajo (pendiente máxima).
    dest = np.full(h.shape, -1, dtype=np.int64)
    for i in range(1, ny - 1):
        fila = lleno[i]
        for j in range(1, nx - 1):
            if agua[i, j]:
                continue
            mejor, best = 0.0, -1
            e = fila[j]
            for di, dj in VEC:
                d = (e - lleno[i + di, j + dj]) / (math.sqrt(2) if di and dj else 1.0)
                if d > mejor:
                    mejor, best = d, (i + di) * nx + (j + dj)
            dest[i, j] = best
    # Acumulación, de lo más alto a lo más bajo.
    orden = np.argsort(-lleno, axis=None)
    acc = np.ones(nx * ny)
    flat_dest = dest.ravel()
    for k in orden:
        d = flat_dest[k]
        if d >= 0:
            acc[d] += acc[k]
    umbral = ARROYO_KM2 * 1e6 / (step * step)
    es = (acc >= umbral) & ~agua.ravel()
    # Cabeceras: celdas de arroyo sin otra celda de arroyo que les desagüe.
    tiene_aporte = np.zeros(nx * ny, bool)
    for k in np.nonzero(es)[0]:
        d = flat_dest[k]
        if d >= 0:
            tiene_aporte[d] = True
    usados = np.zeros(nx * ny, bool)
    lineas = []
    for k in np.nonzero(es & ~tiene_aporte)[0]:
        pts = []
        c = k
        while c >= 0:
            i, j = divmod(int(c), nx)
            pts.append((j * step, i * step, ancho_cauce(acc[c] * step * step / 1e6)))
            if usados[c] or agua.ravel()[c]:
                break
            usados[c] = True
            c = flat_dest[c]
        if len(pts) > 1:
            lineas.append(pts)
    # Ordena para que los tramos más largos (los cauces principales) se tracen primero.
    return lineas, acc.reshape(h.shape)


def rdp(pts, tol):
    """Ramer–Douglas–Peucker sin recursión (los cauces oficiales tienen miles de vértices)."""
    if len(pts) < 3:
        return list(pts)
    arr = np.array([p[:2] for p in pts], dtype=np.float64)
    queda = np.zeros(len(pts), bool)
    queda[0] = queda[-1] = True
    pila = [(0, len(pts) - 1)]
    while pila:
        i0, i1 = pila.pop()
        if i1 - i0 < 2:
            continue
        a, b = arr[i0], arr[i1]
        ab = b - a
        n = np.linalg.norm(ab)
        v = arr[i0 + 1:i1] - a
        if n < 1e-9:
            d = np.linalg.norm(v, axis=1)
        else:
            d = np.abs(ab[0] * v[:, 1] - ab[1] * v[:, 0]) / n
        k = int(np.argmax(d))
        if d[k] > tol:
            k += i0 + 1
            queda[k] = True
            pila.append((i0, k))
            pila.append((k, i1))
    return [p for p, q in zip(pts, queda) if q]


def suavizar(pts, veces=2):
    for _ in range(veces):
        if len(pts) < 3:
            return pts
        out = [pts[0]]
        for p, q in zip(pts[:-1], pts[1:]):
            out.append(tuple(0.75 * a + 0.25 * b for a, b in zip(p, q)))
            out.append(tuple(0.25 * a + 0.75 * b for a, b in zip(p, q)))
        out.append(pts[-1])
        pts = out
    return pts


def largo(pts):
    return sum(math.dist(p[:2], q[:2]) for p, q in zip(pts[:-1], pts[1:]))


# --- Sitio de fundación --------------------------------------------------------------------
def sitio_fundacion(h, step):
    """Busca cerca de la Plaza Matriz un lugar firme y llano para la plaza mayor (≈100 m)."""
    tx, tz = a_local(*PLAZA_MATRIZ)
    mejor = None
    r = 50.0
    for dz in np.arange(-400, 401, 20):
        for dx in np.arange(-400, 401, 20):
            x, z = tx + dx, tz + dz
            i, j = int(round(z / step)), int(round(x / step))
            k = int(r / step)
            if i - k < 0 or j - k < 0 or i + k >= h.shape[0] or j + k >= h.shape[1]:
                continue
            bloque = h[i - k:i + k + 1, j - k:j + k + 1]
            if bloque.min() < 2.0:
                continue
            rango = float(bloque.max() - bloque.min())
            costo = rango + math.hypot(dx, dz) / 100.0
            if mejor is None or costo < mejor[0]:
                mejor = (costo, x, z, rango)
    return mejor


def limpiar_agua(h, step):
    """Separa el río de los pozos del relieve y suaviza el fondo.

    - El agua conectada con el borde sur (el Río de la Plata) es río: su fondo se reemplaza
      por una pendiente suave según la distancia a la costa, porque la batimetría de los
      Terrain Tiles cerca de la costa trae bandas horizontales.
    - Las celdas bajas que NO conectan con el río son pozos del modelo de elevación (muy
      comunes en zona urbana): se rellenan a tierra firme.
    """
    from collections import deque
    ny, nx = h.shape
    bajo = h <= 0.3
    rio = np.zeros(h.shape, bool)
    q = deque()
    for j in range(nx):
        if bajo[ny - 1, j]:
            rio[ny - 1, j] = True
            q.append((ny - 1, j))
    while q:
        i, j = q.popleft()
        for di, dj in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            a, b = i + di, j + dj
            if 0 <= a < ny and 0 <= b < nx and bajo[a, b] and not rio[a, b]:
                rio[a, b] = True
                q.append((a, b))
    pozos = bajo & ~rio
    h = h.copy()
    h[pozos] = np.maximum(h[pozos], 1.5)
    # Distancia a la costa (en celdas) dentro del río, por BFS desde la tierra.
    dist = np.full(h.shape, -1, dtype=np.int32)
    q = deque()
    for i in range(ny):
        for j in range(nx):
            if not rio[i, j]:
                dist[i, j] = 0
                q.append((i, j))
    while q:
        i, j = q.popleft()
        for di, dj in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            a, b = i + di, j + dj
            if 0 <= a < ny and 0 <= b < nx and dist[a, b] < 0:
                dist[a, b] = dist[i, j] + 1
                q.append((a, b))
    d_m = dist.astype(np.float64) * step
    h[rio] = -np.minimum(0.4 + d_m[rio] * 0.004, 7.5)
    return h


def suavizar_tierra(h):
    """Quita los picos del modelo de elevación en tierra.

    SRTM mide la superficie (techos, copas de árboles), no el suelo: en la zona urbana
    aparecen saltos de 10 a 20 m entre celdas vecinas que en el juego se ven como
    pendientes de roca. Mediana de 5×5 (100 m) y luego media de 3×3, solo en tierra, sin
    mover la costa.
    """
    from numpy.lib.stride_tricks import sliding_window_view as ventanas
    tierra = h > 0.5
    p = np.pad(h, 2, mode="edge")
    med = np.median(ventanas(p, (5, 5)), axis=(-2, -1))
    p = np.pad(med, 1, mode="edge")
    media = ventanas(p, (3, 3)).mean(axis=(-2, -1))
    out = h.copy()
    ok = tierra & (media > 0.5)
    out[ok] = media[ok]
    return out


def agregar_islas(h, xs, islas=None):
    """Meseta suave por isla (de la lista ISLAS): borde de piedra y arena que baja al agua."""
    zz, xx = np.meshgrid(xs, xs, indexing="ij")
    for _, cx, cz, largo, ancho, alto, giro in (ISLAS if islas is None else islas):
        a = math.radians(giro)
        dx, dz = xx - cx, zz - cz
        u = (dx * math.cos(a) + dz * math.sin(a)) / (largo / 2)
        v = (-dx * math.sin(a) + dz * math.cos(a)) / (ancho / 2)
        r = np.sqrt(u * u + v * v)
        t = np.clip((1.25 - r) / 0.5, 0, 1)
        isla = -0.6 + (alto + 0.6) * t * t * (3 - 2 * t)
        h = np.where(r < 1.25, np.maximum(h, isla), h)
    return h


# --- Hidrografía oficial (IDE Uruguay, WFS) ---------------------------------------------------
# Códigos de biomas.bin (uint8, misma grilla que altura.bin / contexto.bin).
BIOMA_NADA, BIOMA_PLAYA, BIOMA_BANADO, BIOMA_BANCO, BIOMA_LAGUNA, BIOMA_ISLA = range(6)
CODIGOS_BIOMA = {"0": "sin dato", "1": "playa", "2": "bañado / área húmeda", "3": "banco de arena",
                 "4": "laguna / lago (agua)", "5": "isla (tierra)"}

# Capas usadas. La hidrografía nacional (escala 1:50.000) cubre todo el país salvo las zonas
# urbanas, que están en la capa urbana (más detallada); se usan las dos y se complementan.
CAPAS_IDE = {
    "cursos": ["hidrografia:eje_curso_de_agua_l", "hidrografia_urbana:eje_curso_de_agua_l"],
    "cauces_anchos": ["hidrografia:curso_de_agua_a", "hidrografia_urbana:curso_de_agua_a"],
    "playas": ["hidrografia:playa_a", "hidrografia_urbana:playa_a"],
    "banados": ["hidrografia:banado_a", "hidrografia_urbana:banado_a",
                "hidrografia:area_humeda_a", "hidrografia_urbana:area_humeda_a"],
    "bancos": ["hidrografia:banco_de_arena_a", "hidrografia_urbana:banco_de_arena_a"],
    # Las lagunas de la capa urbana son casi todas artificiales (parques, canteras): no se usan.
    "lagunas": ["hidrografia:lago_laguna_a"],
    # Las islas de la capa urbana son islotes dentro de los arroyos: no se usan.
    "islas": ["hidrografia:isla_a"],
    "cerros": ["ideuy:cerros_ide"],
    "cuchillas": ["ideuy:cuchillas_y_sierras"],
}
# No se usan (obras modernas): embalse_tajamar_a (tajamares y represas), canal_l, eje_canal_l.

# Lagos que no existían en 1726 (rectángulo x0, z0, x1, z1 en m locales).
LAGOS_MODERNOS = [
    (23500.0, 5500.0, 32500.0, 10800.0, "Lagos de Carrasco y Lomas de Solymar: canteras de arena del siglo XX"),
    (20800.0, 10700.0, 21800.0, 11700.0, "lago del Parque Rivera (1930)"),
    (7600.0, 10200.0, 8800.0, 11300.0, "piletas junto a la refinería de La Teja"),
]
LAGO_MIN_HA = 1.0
ANCHO_MAX_CAUCE = 60.0
ARROYO_RELIEVE_MIN_KM2 = 1.0     # arroyos del relieve que rellenan los huecos de la capa oficial
HUECO_OFICIAL_M = 300.0          # ... donde no hay ningún cauce oficial a menos de esto


def _local_arr(coords):
    a = np.asarray(coords, dtype=np.float64)[:, :2]
    return np.stack([(a[:, 0] - LON_W) * KX, (LAT_N - a[:, 1]) * KY], axis=1)


def _poligonos(g):
    """Polígonos de una geometría GeoJSON: lista de [anillo exterior, huecos…] en m locales."""
    if g is None:
        return []
    if g["type"] == "Polygon":
        return [[_local_arr(r) for r in g["coordinates"] if len(r) >= 3]]
    if g["type"] == "MultiPolygon":
        return [[_local_arr(r) for r in p if len(r) >= 3] for p in g["coordinates"]]
    return []


def _lineas(g):
    if g is None:
        return []
    if g["type"] == "LineString":
        return [_local_arr(g["coordinates"])]
    if g["type"] == "MultiLineString":
        return [_local_arr(c) for c in g["coordinates"] if len(c) >= 2]
    if g["type"] in ("Point", "MultiPoint"):
        c = g["coordinates"]
        return [_local_arr([c] if g["type"] == "Point" else c)]
    return []


# --- Suelos CONEAT: afloramientos de roca ----------------------------------------------------
CONEAT_URL = "https://web.snig.gub.uy/arcgisserver/rest/services/MapasBase/Renare_Coneat/MapServer/0/query"

# Rocosidad aproximada (fracción de la superficie con roca a la vista) según la zona CONEAT
# (primer número del grupo). El servicio solo trae el código del grupo y su aptitud; la
# descripción de cada grupo (con su porcentaje de rocosidad) está en las publicaciones de
# RENARE/MGAP. Esta tabla resume esas descripciones a grandes rasgos; ver
# docs/sistemas/suelos-y-roca.md.
ROCOSIDAD_ZONA = {
    "1": 0.35,   # sierras rocosas (Minas, Lavalleja): afloramientos y bochas de granito
    "2": 0.30,   # sierras del este
    "4": 0.12,   # colinas sobre el basamento cristalino del sur: suelos superficiales
    "5": 0.04,   # lomadas del basamento cristalino
    "8": 0.05,
    "9": 0.02,
    "11": 0.05,
    "12": 0.25,  # basalto superficial
    "13": 0.10,
}


def rocosidad_grupo(sc, aptitud):
    zona = (sc or "").split(".")[0].lstrip("0") or "0"
    if (sc or "").startswith("03"):
        return 0.0  # planicies bajas y bañados
    r = ROCOSIDAD_ZONA.get(zona, 0.0)
    a = (aptitud or "").lower()
    if "no aptas" in a:
        r *= 1.5
    elif "marginal" in a:
        r *= 1.2
    return min(r, 0.6)


def bajar_coneat(cache):
    """Polígonos CONEAT de la ventana (mapa + entorno), en GeoJSON, de a 1000."""
    ruta = os.path.join(cache, f"coneat_{int(OUTER)}.json")
    if os.path.exists(ruta):
        with open(ruta) as f:
            return json.load(f)
    lat_s = LAT_N - (MAP_SIZE + OUTER) / KY
    lat_n = LAT_N + OUTER / KY
    lon_w = LON_W - OUTER / KX
    lon_e = LON_W + (MAP_SIZE + OUTER) / KX
    feats, offset = [], 0
    while True:
        q = urllib.parse.urlencode({
            "where": "1=1", "geometry": f"{lon_w},{lat_s},{lon_e},{lat_n}", "geometryType": "esriGeometryEnvelope",
            "inSR": 4326, "outSR": 4326, "spatialRel": "esriSpatialRelIntersects", "outFields": "SC,APTITUD",
            "returnGeometry": "true", "f": "geojson", "resultOffset": offset, "resultRecordCount": 1000,
        })
        for intento in range(5):
            try:
                with urllib.request.urlopen(CONEAT_URL + "?" + q, timeout=120) as r:
                    d = json.load(r)
                break
            except Exception as e:  # el servidor corta conexiones de vez en cuando
                if intento == 4:
                    raise
                print(f"  reintento CONEAT ({e})", flush=True)
        fs = d.get("features", [])
        feats.extend(fs)
        if len(fs) < 1000:
            break
        offset += len(fs)
    with open(ruta, "w") as f:
        json.dump(feats, f)
    return feats


def rocas_coneat(cache):
    """Rasters de rocosidad (0 a 255 = 0 a 100 %) en las grillas del mapa y del entorno."""
    feats = bajar_coneat(cache)
    print(f"  CONEAT: {len(feats)} polígonos", flush=True)
    grillas = []
    for desde, paso, n in ((0.0, STEP, int(MAP_SIZE / STEP) + 1), (-OUTER, OUTER_STEP, int((MAP_SIZE + 2 * OUTER) / OUTER_STEP) + 1)):
        img = Image.new("L", (n, n), 0)
        for ft in feats:
            pr = ft.get("properties") or {}
            v = int(round(rocosidad_grupo(pr.get("SC"), pr.get("APTITUD")) * 255))
            if v > 0:
                rasterizar(_poligonos(ft.get("geometry")), desde, paso, n, valor=v, img=img, borde=False)
        grillas.append(np.asarray(img, dtype=np.uint8))
    grupos = {}
    for ft in feats:
        sc = (ft.get("properties") or {}).get("SC")
        grupos[sc] = grupos.get(sc, 0) + 1
    return grillas[0], grillas[1], grupos


def area_perimetro(anillo):
    x, z = anillo[:, 0], anillo[:, 1]
    a = 0.5 * abs(np.dot(x, np.roll(z, -1)) - np.dot(z, np.roll(x, -1)))
    p = float(np.sum(np.hypot(np.diff(x, append=x[:1]), np.diff(z, append=z[:1]))))
    return a, p


def rasterizar(poligonos, desde, paso, n, valor=1, modo="L", img=None, borde=None):
    """Pinta polígonos (con huecos) en una grilla de n × n muestras: la muestra (i, j) está en
    x = desde + j·paso, z = desde + i·paso. En la grilla gruesa (o con borde=True) se pinta
    también el contorno, así las playas angostas no desaparecen."""
    if borde is None:
        borde = paso > 50.0
    from PIL import ImageDraw
    if img is None:
        img = Image.new(modo, (n, n), 0)
    d = ImageDraw.Draw(img)
    lim0, lim1 = desde - paso, desde + paso * n
    for anillos in poligonos:
        if not anillos:
            continue
        ext = anillos[0]
        if ext[:, 0].max() < lim0 or ext[:, 0].min() > lim1 or ext[:, 1].max() < lim0 or ext[:, 1].min() > lim1:
            continue
        for k, r in enumerate(anillos):
            pts = [((x - desde) / paso, (z - desde) / paso) for x, z in r]
            if k == 0:
                d.polygon(pts, fill=valor, outline=valor if borde else None)
            else:
                d.polygon(pts, fill=0)
    return img


def dilatar(m, celdas):
    from PIL import ImageFilter
    img = Image.fromarray(m.astype(np.uint8) * 255)
    for _ in range(celdas):
        img = img.filter(ImageFilter.MaxFilter(3))
    return np.asarray(img) > 127


def prof_dentro(m, max_celdas):
    """Celdas hasta el borde de la máscara (1 en el borde), con tope."""
    from PIL import ImageFilter
    img = Image.fromarray(m.astype(np.uint8) * 255)
    d = m.astype(np.float64)
    for _ in range(max_celdas):
        img = img.filter(ImageFilter.MinFilter(3))
        a = np.asarray(img) > 127
        if not a.any():
            break
        d += a
    return d


def nombre_lindo(nom):
    """'ARROYO DE LAS PIEDRAS' → 'Arroyo de las Piedras'."""
    if not nom:
        return None
    chicas = {"de", "del", "la", "las", "los", "el", "y"}
    pal = nom.strip().lower().split()
    out = [p if (k > 0 and p in chicas) else p[:1].upper() + p[1:] for k, p in enumerate(pal)]
    s = " ".join(out)
    return s.replace("Rio ", "Río ")


def cargar_hidrografia(cache):
    datos = {}
    for rol, capas in CAPAS_IDE.items():
        datos[rol] = []
        for c in capas:
            fs = bajar_capa_wfs(cache, c)
            print(f"  {c}: {len(fs)} objetos", flush=True)
            datos[rol] += [(c, f) for f in fs]
    return datos


def lago_moderno(anillos):
    """Lagos que se descartan: los de la lista LAGOS_MODERNOS y los charcos de menos de
    LAGO_MIN_HA (casi siempre tajamares o piletas que la capa no distingue)."""
    if area_perimetro(anillos[0])[0] < LAGO_MIN_HA * 1e4:
        return True
    cx, cz = anillos[0][:, 0].mean(), anillos[0][:, 1].mean()
    return any(x0 <= cx <= x1 and z0 <= cz <= z1 for x0, z0, x1, z1, _ in LAGOS_MODERNOS)


def mascaras(datos, desde, paso, n):
    """Máscaras por clase y el raster de biomas de una grilla."""
    def m(rol, filtro=None):
        polis = []
        for _, f in datos[rol]:
            for p in _poligonos(f["geometry"]):
                if filtro is None or filtro(p):
                    polis.append(p)
        return np.asarray(rasterizar(polis, desde, paso, n)) > 0
    isla = m("islas")
    if paso > 50.0:
        # En la grilla de 100 m una isla chica (la de Flores: tres islotes de unos 200 m de
        # ancho) queda como una fila de celdas sueltas que el motor no llega a dibujar de lejos:
        # se engrosa una celda.
        chicas = m("islas", lambda p: area_perimetro(p[0])[0] < 50e4)
        isla |= dilatar(chicas, 1)
    out = {
        "isla": isla,
        "banado": m("banados"),
        "playa": m("playas"),
        "banco": m("bancos"),
        "laguna": m("lagunas", lambda p: not lago_moderno(p)),
    }
    b = np.zeros((n, n), np.uint8)
    b[out["isla"]] = BIOMA_ISLA
    b[out["banado"]] = BIOMA_BANADO
    b[out["playa"]] = BIOMA_PLAYA
    b[out["banco"]] = BIOMA_BANCO
    b[out["laguna"]] = BIOMA_LAGUNA
    return out, b


def islas_oficiales(h, isla, paso, alto=4.0):
    """Levanta las islas oficiales que el MDT deja como agua: domo bajo que sube del agua.
    Donde el MDT ya tiene tierra se respeta su altura."""
    dm = prof_dentro(isla, 8) * paso
    domo = -0.6 + (alto + 0.6) * np.clip(dm / 60.0, 0, 1) ** 2 * (3 - 2 * np.clip(dm / 60.0, 0, 1))
    domo = np.maximum(domo, 0.8)
    out = h.copy()
    sel = isla & (h <= 0.3)
    out[sel] = np.maximum(h[sel], domo[sel])
    return out


def lagunas_oficiales(h, datos, desde, paso, n):
    """Las lagunas bajas (a nivel del río) se hunden bajo el agua; las altas quedan llanas y el
    motor pinta su superficie como agua de arroyo (código 4 de biomas.bin)."""
    h = h.copy()
    bajas = altas = 0
    for _, f in datos["lagunas"]:
        for p in _poligonos(f["geometry"]):
            if lago_moderno(p):
                continue
            mk = np.asarray(rasterizar([p], desde, paso, n)) > 0
            if not mk.any():
                continue
            nivel = float(np.percentile(h[mk], 20))
            if nivel <= 1.0:
                h[mk] = np.minimum(h[mk], -0.5)
                bajas += 1
            else:
                h[mk] = nivel - 0.2
                altas += 1
    return h, bajas, altas


def tramos_oficiales(datos):
    """Tramos de cauce: (puntos (n, 2) en m locales, nombre o None, hyc)."""
    tramos = []
    for capa, f in datos["cursos"]:
        p = f["properties"]
        urbana = capa.startswith("hidrografia_urbana")
        # cat 3 = cauce natural, 5 = brazo de río (Santa Lucía), 7 = tramos cortos de conexión
        # (alcantarillas, cruces). En la capa urbana, cat 7 son las cunetas de las calles.
        if p.get("cat") not in ((3, 5) if urbana else (3, 5, 7)):
            continue
        nom = p.get("nam")
        nom = None if nom in (None, "", "NULL") else nom.strip()
        if nom and nom.upper().startswith("CANAL"):
            continue  # canales modernos (Canal Malvín, Canal Manga)
        for l in _lineas(f["geometry"]):
            if len(l) >= 2:
                tramos.append((l, nom, p.get("hyc"), urbana))
    # Donde las dos capas se superponen (el Pantanoso, el Miguelete…) se queda la nacional: se
    # descartan los tramos urbanos que corren a menos de 25 m de un cauce nacional.
    from PIL import ImageDraw
    paso, desde = 10.0, -OUTER
    n = int((MAP_SIZE + 2 * OUTER) / paso) + 1
    img = Image.new("L", (n, n), 0)
    d = ImageDraw.Draw(img)
    for l, _, _, urb in tramos:
        if not urb:
            d.line([((x - desde) / paso, (z - desde) / paso) for x, z in l], fill=255, width=5)
    nac = np.asarray(img) > 0
    out = []
    for l, nom, hyc, urb in tramos:
        if urb:
            # Muestras cada ~5 m a lo largo del tramo.
            dens = []
            for a, b in zip(l[:-1], l[1:]):
                k = max(1, int(np.hypot(*(b - a)) / 5.0))
                dens += [a + (b - a) * (s / k) for s in range(k)]
            dens.append(l[-1])
            dens = np.array(dens)
            i = np.clip(np.round((dens[:, 1] - desde) / paso).astype(int), 0, n - 1)
            j = np.clip(np.round((dens[:, 0] - desde) / paso).astype(int), 0, n - 1)
            if nac[i, j].mean() > 0.5:
                continue
        out.append((l, nom, hyc))
    print(f"  {len(tramos)} tramos de cauce, {len(tramos) - len(out)} urbanos repetidos", flush=True)
    return out


def unir_tramos(tramos):
    """Une los tramos (la capa viene cortada cada pocos cientos de metros) en polilíneas
    continuas, sin pasar por las confluencias ni mezclar nombres."""
    from collections import defaultdict
    clave = lambda p: (int(round(p[0])), int(round(p[1])))  # noqa: E731
    ext = defaultdict(list)
    for k, (l, _, _) in enumerate(tramos):
        ext[clave(l[0])].append(k)
        ext[clave(l[-1])].append(k)
    usado = [False] * len(tramos)
    out = []
    for k, (l, nom, hyc) in enumerate(tramos):
        if usado[k]:
            continue
        usado[k] = True
        pts = [tuple(p) for p in l]
        hycs = [hyc] * len(pts)
        for lado in (1, 0):
            while True:
                extremo = pts[-1] if lado else pts[0]
                lista = ext[clave(extremo)]
                libres = [c for c in lista if not usado[c]]
                if len(lista) != 2 or len(libres) != 1 or tramos[libres[0]][1] != nom:
                    break
                c = libres[0]
                usado[c] = True
                lc = [tuple(p) for p in tramos[c][0]]
                if clave(lc[0]) != clave(extremo):
                    lc = lc[::-1]
                if lado:
                    pts += lc[1:]
                    hycs += [tramos[c][2]] * (len(lc) - 1)
                else:
                    lc = lc[::-1]
                    pts = lc[:-1] + pts
                    hycs = [tramos[c][2]] * (len(lc) - 1) + hycs
        out.append((pts, nom, hycs))
    return out


def raster_anchos(datos, desde, paso, n):
    """Ancho (m) de los cauces que la IDE dibuja como polígono (los arroyos anchos), estimado
    por polígono como 2·área / perímetro. Se omite el polígono del Río de la Plata."""
    img = Image.new("I", (n, n), 0)
    for _, f in datos["cauces_anchos"]:
        for p in _poligonos(f["geometry"]):
            a, per = area_perimetro(p[0])
            if a > 5e6 or per <= 0:
                continue
            w = min(2.0 * a / per, ANCHO_MAX_CAUCE)
            rasterizar([p[:1]], desde, paso, n, valor=int(w * 10), img=img, borde=True)
    return np.asarray(img, dtype=np.float64) / 10.0


class Grilla:
    """Valores (cuenca, ancho de polígono) en una grilla regular, con máximo en un vecindario."""

    def __init__(self, v, desde, paso):
        self.v, self.desde, self.paso = v, desde, paso

    def max_cerca(self, x, z, r):
        i = int(round((z - self.desde) / self.paso))
        j = int(round((x - self.desde) / self.paso))
        n = self.v.shape[0]
        if i < -r or j < -r or i >= n + r or j >= n + r:
            return None
        a = self.v[max(i - r, 0):min(i + r + 1, n), max(j - r, 0):min(j + r + 1, n)]
        return float(a.max()) if a.size else None


def anchos_oficiales(pts, hycs, cuenca_main, cuenca_ctx, anchos_main, anchos_ctx):
    """Ancho del agua en cada punto: el del polígono del cauce si lo hay; si no, según la cuenca
    acumulada en el relieve (como los arroyos calculados). Los intermitentes, más angostos."""
    ws = []
    for (x, z), hyc in zip(pts, hycs):
        dentro = 0.0 <= x <= MAP_SIZE and 0.0 <= z <= MAP_SIZE
        if dentro:
            km2 = (cuenca_main.max_cerca(x, z, 2) or 0.0) * STEP * STEP / 1e6
            wp = anchos_main.max_cerca(x, z, 1) or 0.0
        else:
            km2 = (cuenca_ctx.max_cerca(x, z, 1) or 0.0) * OUTER_STEP * OUTER_STEP / 1e6
            wp = anchos_ctx.max_cerca(x, z, 1) or 0.0
        w = ancho_cauce(max(km2, 0.05))
        if hyc == 6:
            w *= 0.7
        ws.append(min(max(w, wp, 1.5), ANCHO_MAX_CAUCE))
    # Suavizado a lo largo del cauce: sin saltos de ancho entre vértices.
    if len(ws) >= 5:
        a = np.convolve(np.pad(np.array(ws), 2, mode="edge"), np.ones(5) / 5, mode="valid")
        ws = list(a)
    return ws


def arroyos_relieve_faltantes(lineas_dem, oficial_main):
    """Arroyos calculados del relieve donde la capa oficial no tiene ningún cauce: en la ciudad
    actual casi todos están entubados y no figuran, pero en 1726 corrían a cielo abierto
    (Arroyo de los Pocitos, Arroyo Seco, Quitacalzones…). Cada tramo se alarga aguas abajo
    hasta tocar un cauce oficial."""
    lejos = ~dilatar(oficial_main, int(HUECO_OFICIAL_M / STEP))
    cerca = dilatar(oficial_main, 2)
    w_min = ancho_cauce(ARROYO_RELIEVE_MIN_KM2)
    n = lejos.shape[0]
    out = []
    for pts in lineas_dem:
        tramo = []
        for p in pts:
            i, j = min(int(round(p[1] / STEP)), n - 1), min(int(round(p[0] / STEP)), n - 1)
            if lejos[i, j]:
                tramo.append(p)
            elif tramo:
                tramo.append(p)
                if cerca[i, j]:
                    out.append(tramo)
                    tramo = []
        if tramo:
            out.append(tramo)
    return [t for t in out if len(t) > 1 and largo(t) >= 500.0 and max(q[2] for q in t) >= w_min]


def guardar_u16(ruta, h):
    v = np.clip(np.round((h + 50.0) * 200.0), 0, 65535).astype("<u2")
    with open(ruta, "wb") as f:
        f.write(v.tobytes())


def guardar_json(ruta, datos, compacto=True):
    with open(ruta, "w") as f:
        if compacto:
            json.dump(datos, f, ensure_ascii=False, separators=(",", ":"))
        else:
            json.dump(datos, f, ensure_ascii=False, indent=2)


def punto_medio(pts):
    """Punto a mitad de camino (por largo) de una polilínea."""
    total = largo(pts)
    acum = 0.0
    for p, q in zip(pts[:-1], pts[1:]):
        d = math.dist(p[:2], q[:2])
        if acum + d >= total / 2 and d > 0:
            t = (total / 2 - acum) / d
            return p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t
        acum += d
    return pts[0][0], pts[0][1]


def en_ventana(pts, margen):
    return any(-margen <= p[0] <= MAP_SIZE + margen and -margen <= p[1] <= MAP_SIZE + margen for p in pts)


def hidrografia_ide(args, h, h_o, xs, xs_o):
    """Paso --hidrografia ide: islas, lagunas, biomas, arroyos y nombres oficiales.
    Devuelve las alturas corregidas y lo que hay que escribir."""
    print("Bajando la hidrografía de la IDE Uruguay (WFS)…", flush=True)
    datos = cargar_hidrografia(args.cache)
    n, n_o = len(xs), len(xs_o)
    mk, bio = mascaras(datos, 0.0, STEP, n)
    mk_o, bio_o = mascaras(datos, -OUTER, OUTER_STEP, n_o)
    # Islas: las oficiales; las manuales solo si la IDE no trae ninguna cerca.
    h = islas_oficiales(h, mk["isla"], STEP)
    h_o = islas_oficiales(h_o, mk_o["isla"], OUTER_STEP)
    manuales = []
    nombres_islas = []
    for isla in ISLAS:
        nom, cx, cz = isla[:3]
        i, j = int(round((cz + OUTER) / OUTER_STEP)), int(round((cx + OUTER) / OUTER_STEP))
        cubierta = mk_o["isla"][max(i - 3, 0):i + 4, max(j - 3, 0):j + 4].any()
        if 0 <= cx <= MAP_SIZE and 0 <= cz <= MAP_SIZE:
            i, j = int(round(cz / STEP)), int(round(cx / STEP))
            cubierta = mk["isla"][max(i - 15, 0):i + 16, max(j - 15, 0):j + 16].any()
        print(f"  {nom}: {'en la capa oficial' if cubierta else 'no está en la capa: se agrega a mano'}", flush=True)
        nombres_islas.append({"nombre": nom, "tipo": "isla", "x": cx, "z": cz,
                              "en_mapa": bool(0 <= cx <= MAP_SIZE and 0 <= cz <= MAP_SIZE),
                              "fuente": "IDE (isla_a); nombre a mano" if cubierta else "a mano"})
        if not cubierta:
            manuales.append(isla)
    if manuales:
        h = agregar_islas(h, xs, manuales)
        h_o = agregar_islas(h_o, xs_o, manuales)
    # Lagunas.
    h, lb, la = lagunas_oficiales(h, datos, 0.0, STEP, n)
    h_o, _, _ = lagunas_oficiales(h_o, datos, -OUTER, OUTER_STEP, n_o)
    print(f"  lagunas en el mapa: {lb} bajo el agua, {la} llanas (agua de arroyo)", flush=True)
    for k, v in mk.items():
        print(f"  {k}: {int(v.sum())} celdas de 20 m ({v.sum() * STEP * STEP / 1e4:.0f} ha)", flush=True)

    # Cuenca acumulada (para el ancho) y arroyos del relieve (para los huecos de la capa).
    print("Calculando cuencas…", flush=True)
    lineas_dem, acc = arroyos(h, STEP)
    _, acc_o = arroyos(h_o, OUTER_STEP)
    cuenca, cuenca_o = Grilla(acc, 0.0, STEP), Grilla(acc_o, -OUTER, OUTER_STEP)
    anchos = Grilla(raster_anchos(datos, 0.0, STEP, n), 0.0, STEP)
    anchos_o = Grilla(raster_anchos(datos, -OUTER, OUTER_STEP, n_o), -OUTER, OUTER_STEP)

    print("Arroyos oficiales…", flush=True)
    lineas = unir_tramos(tramos_oficiales(datos))
    principal, contexto = [], []
    for pts, nom, hycs in lineas:
        ws = anchos_oficiales(pts, hycs, cuenca, cuenca_o, anchos, anchos_o)
        p3 = [(x, z, w) for (x, z), w in zip(pts, ws)]
        if en_ventana(p3, 1000.0):
            principal.append((rdp(p3, 3.0), nom, "ide"))
        elif en_ventana(p3, OUTER):
            contexto.append((rdp(p3, 8.0), nom, "ide"))
    # Máscara de los cauces oficiales en la grilla del mapa, para buscar los huecos.
    from PIL import ImageDraw
    img = Image.new("L", (n, n), 0)
    d = ImageDraw.Draw(img)
    for pts, _, _ in principal:
        d.line([(p[0] / STEP, p[1] / STEP) for p in pts], fill=1, width=1)
    oficial = np.asarray(img) > 0
    relleno = []
    for t in arroyos_relieve_faltantes(lineas_dem, oficial):
        p = suavizar(rdp(t, STEP * 0.75), 2)
        relleno.append(([(x, z, w) for x, z, w in p], None, "relieve"))
    print(f"  {len(principal)} cauces oficiales en el mapa, {len(contexto)} en el entorno, "
          f"{len(relleno)} del relieve en los huecos", flush=True)
    principal += relleno
    principal.sort(key=lambda a: -largo(a[0]))
    # En el entorno solo los cauces con nombre o anchos: los demás no se ven desde el mapa.
    contexto = [c for c in contexto if c[1] or max(p[2] for p in c[0]) >= 5.0]
    contexto.sort(key=lambda a: -largo(a[0]))

    def a_json(lista):
        return {
            "arroyos": [[[round(x, 1), round(z, 1), round(w, 1)] for x, z, w in pts] for pts, _, _ in lista],
            "nombres": [nombre_lindo(nom) for _, nom, _ in lista],
            "fuente": [f for _, _, f in lista],
        }

    # Nombres para rótulos: cada arroyo con nombre, en el punto medio de su tramo más largo.
    por_nombre = {}
    for pts, nom, _ in principal + contexto:
        if nom:
            e = por_nombre.setdefault(nom, {"largo": 0.0, "mejor": None})
            L = largo(pts)
            e["largo"] += L
            if e["mejor"] is None or L > largo(e["mejor"]):
                e["mejor"] = pts
    nombres = []
    for nom, e in sorted(por_nombre.items(), key=lambda kv: -kv[1]["largo"]):
        x, z = punto_medio(e["mejor"])
        tipo = nom.split()[0].lower().replace("rio", "río")
        nombres.append({"nombre": nombre_lindo(nom), "oficial": nom, "tipo": tipo, "x": round(x, 1), "z": round(z, 1),
                        "largo_m": round(e["largo"]), "en_mapa": bool(0 <= x <= MAP_SIZE and 0 <= z <= MAP_SIZE),
                        "fuente": "IDE (eje_curso_de_agua_l)"})
    for _, f in datos["cerros"]:
        for l in _lineas(f["geometry"]):
            x, z = l[0]
            nombres.append({"nombre": f["properties"].get("nombre"), "tipo": "cerro", "x": round(float(x), 1),
                            "z": round(float(z), 1), "altura_m": round(float(f["properties"].get("elev") or 0), 1),
                            "en_mapa": bool(0 <= x <= MAP_SIZE and 0 <= z <= MAP_SIZE), "fuente": "IDE (cerros_ide)"})
    for _, f in datos["cuchillas"]:
        pts = [tuple(map(float, p)) for l in _lineas(f["geometry"]) for p in l]
        dentro = [p for p in pts if -OUTER <= p[0] <= MAP_SIZE + OUTER and -OUTER <= p[1] <= MAP_SIZE + OUTER]
        if not dentro:
            continue
        x, z = punto_medio(dentro)
        nombres.append({"nombre": nombre_lindo(f["properties"].get("nombre")), "tipo": "cuchilla", "x": round(x, 1), "z": round(z, 1),
                        "linea": [[round(p[0]), round(p[1])] for p in rdp(dentro, 50.0)],
                        "en_mapa": any(0 <= p[0] <= MAP_SIZE and 0 <= p[1] <= MAP_SIZE for p in dentro),
                        "fuente": "IDE (cuchillas_y_sierras)"})
    nombres += nombres_islas

    conteo = {rol: len(v) for rol, v in datos.items()}
    salida = {
        "arroyos": a_json(principal), "arroyos_contexto": a_json(contexto),
        "biomas": bio, "biomas_contexto": bio_o, "nombres": nombres, "conteo": conteo,
        "mascaras": mk, "islas_manuales": [i[0] for i in manuales],
    }
    return h, h_o, salida


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cache", required=True, help="carpeta vacía o con tiles ya bajados")
    ap.add_argument("--fuente", choices=["ide", "aws"], default="ide", help="MDT de la IDE Uruguay (suelo) o Terrain Tiles de AWS (superficie)")
    ap.add_argument("--hidrografia", choices=["ide", "relieve"], default=None,
                    help="arroyos, playas, bañados, lagunas e islas de la IDE (por omisión con --fuente ide) "
                         "o arroyos calculados del relieve")
    ap.add_argument("--salida", default=os.path.join(os.path.dirname(__file__), "..", "..", "assets", "mapas", "montevideo"))
    args = ap.parse_args()
    if args.hidrografia is None:
        args.hidrografia = "ide" if args.fuente == "ide" else "relieve"
    os.makedirs(args.cache, exist_ok=True)
    os.makedirs(args.salida, exist_ok=True)

    xs = np.arange(0.0, MAP_SIZE + STEP / 2, STEP)
    xs_o = np.arange(-OUTER, MAP_SIZE + OUTER + OUTER_STEP / 2, OUTER_STEP)
    fuentes = []
    if args.fuente == "ide":
        print("Bajando el MDT de la IDE Uruguay…", flush=True)
        mdt = bajar_mdt(args.cache)
        print(f"  recorte {mdt[0].shape}, {mdt[3] * KY:.0f} m por muestra", flush=True)
        h = muestrear_mdt(mdt, xs, xs)
        h_o = muestrear_mdt(mdt, xs_o, xs_o)
        print(f"  mapa {h.shape} min {h.min():.1f} máx {h.max():.1f}", flush=True)
        h = limpiar_agua(h, STEP)
        h = suavizar_suave(h)
        h_o = limpiar_agua(h_o, OUTER_STEP)
        fuente = "MDT Uruguay 30 m (IDE Uruguay), servicio WCS raster__mdt_pais_30m_4326"
        atribucion = "Datos de elevación: Modelo Digital del Terreno del Uruguay, Infraestructura de Datos Espaciales (IDE Uruguay, Presidencia de la República)."
        fuentes.append({"capa": "raster__mdt_pais_30m_4326", "servicio": "WCS https://mapas.ide.uy/geoserver-raster/wcs",
                        "uso": "relieve (altura.bin, contexto.bin)"})
    else:
        print("Bajando tiles…", flush=True)
        alto, ox, oy, n = mosaico(args.cache)
        print(f"  {n} tiles, mosaico {alto.shape}", flush=True)
        h = muestrear(alto, ox, oy, xs, xs)
        h_o = muestrear(alto, ox, oy, xs_o, xs_o)
        print(f"  mapa {h.shape} min {h.min():.1f} máx {h.max():.1f}", flush=True)
        h = limpiar_agua(h, STEP)
        h = suavizar_tierra(h)
        h_o = limpiar_agua(h_o, OUTER_STEP)
        fuente = "AWS Terrain Tiles (terrarium), zoom %d" % ZOOM
        atribucion = "Datos de elevación: Terrain Tiles de Mapzen/Tilezen en AWS, que incluyen SRTM (NASA), GMTED2010 (USGS), ETOPO1 (NOAA) y otras fuentes."
        fuentes.append({"capa": "terrarium z%d" % ZOOM, "servicio": URL, "uso": "relieve"})

    hidro = None
    if args.hidrografia == "ide":
        h, h_o, hidro = hidrografia_ide(args, h, h_o, xs, xs_o)
        polis = [[tuple(p) for p in a] for a in hidro["arroyos"]["arroyos"]]
        atribucion += (" Hidrografía (cursos de agua, playas, bañados, áreas húmedas, bancos de arena, lagunas e islas),"
                       " cerros y cuchillas: IDE Uruguay, servicio WFS (hidrografía nacional 1:50.000 y urbana)."
                       " Licencia de Datos Abiertos del Uruguay.")
        for rol, capas in CAPAS_IDE.items():
            for c in capas:
                fuentes.append({"capa": c, "servicio": "WFS " + IDE_WFS.split("?")[0], "uso": rol})
    else:
        h = agregar_islas(h, xs)
        h_o = agregar_islas(h_o, xs_o)
        print("Calculando arroyos…", flush=True)
        lineas, _ = arroyos(h, STEP)
        polis = []
        for pts in lineas:
            p = suavizar(rdp(pts, STEP * 0.75), 2)
            if largo(p) >= ARROYO_MIN_LEN:
                polis.append([[round(x, 1), round(z, 1), round(w, 1)] for x, z, w in p])
        polis.sort(key=lambda p: -largo(p))
    rocas = None
    if args.fuente == "ide":
        print("Suelos CONEAT (rocosidad)…", flush=True)
        try:
            rocas = rocas_coneat(args.cache)
            atribucion += " Suelos: grupos CONEAT (RENARE, MGAP), servicio SNIG MapasBase/Renare_Coneat."
            fuentes.append({"capa": "Renare_Coneat", "servicio": CONEAT_URL, "uso": "rocosidad (afloramientos)"})
        except Exception as e:
            print(f"  sin CONEAT: {e}", flush=True)
    atribucion += " Ver docs/sistemas/mapa-real.md."
    print(f"  {len(polis)} arroyos en el mapa", flush=True)

    sitio = sitio_fundacion(h, STEP)
    cx, cz = a_local(*CERRO)
    meta = {
        "nombre": "Montevideo (geografía real)",
        "fuente": fuente,
        "atribucion": atribucion,
        "origen_noroeste": {"lat": LAT_N, "lon": LON_W},
        "metros_por_grado": {"lon": KX, "lat": KY},
        "tamano_m": MAP_SIZE,
        "paso_m": STEP,
        "muestras": int(h.shape[0]),
        "contexto": {"desde_m": -OUTER, "paso_m": OUTER_STEP, "muestras": int(h_o.shape[0])},
        "codificacion": "uint16 little-endian; metros = valor / 200 - 50; filas de norte a sur",
        "fundacion": {"x": round(sitio[1], 1), "z": round(sitio[2], 1), "desnivel_m": round(sitio[3], 2)},
        "cerro": {"x": round(cx, 1), "z": round(cz, 1), "altura_m": round(float(h[int(cz / STEP), int(cx / STEP)]), 1)},
        "hidrografia": args.hidrografia,
        "fuentes": fuentes,
    }
    if hidro:
        meta["biomas"] = {
            "archivo": "biomas.bin", "archivo_entorno": "biomas_contexto.bin",
            "codificacion": "uint8, misma grilla y orden que altura.bin / contexto.bin",
            "codigos": CODIGOS_BIOMA,
        }
        meta["licencia_ide"] = ("Licencia de Datos Abiertos del Uruguay (AGESIC), según AccessConstraints del "
                                "GetCapabilities del WFS de la IDE: https://www.gub.uy/agencia-gobierno-electronico-sociedad-informacion-conocimiento/"
                                "sites/agencia-gobierno-electronico-sociedad-informacion-conocimiento/files/documentos/publicaciones/licencia_de_datos_abiertos_0.pdf")
        meta["conteo_objetos_ide"] = hidro["conteo"]
        meta["islas_a_mano"] = hidro["islas_manuales"]
    guardar_u16(os.path.join(args.salida, "altura.bin"), h)
    guardar_u16(os.path.join(args.salida, "contexto.bin"), h_o)
    if hidro:
        guardar_json(os.path.join(args.salida, "arroyos.json"), hidro["arroyos"])
        guardar_json(os.path.join(args.salida, "arroyos_contexto.json"), hidro["arroyos_contexto"])
        guardar_json(os.path.join(args.salida, "nombres.json"), {"nombres": hidro["nombres"]}, compacto=False)
        with open(os.path.join(args.salida, "biomas.bin"), "wb") as f:
            f.write(hidro["biomas"].astype(np.uint8).tobytes())
        with open(os.path.join(args.salida, "biomas_contexto.bin"), "wb") as f:
            f.write(hidro["biomas_contexto"].astype(np.uint8).tobytes())
    else:
        guardar_json(os.path.join(args.salida, "arroyos.json"), {"arroyos": polis})
    if rocas is not None:
        with open(os.path.join(args.salida, "rocas.bin"), "wb") as f:
            f.write(rocas[0].tobytes())
        with open(os.path.join(args.salida, "rocas_contexto.bin"), "wb") as f:
            f.write(rocas[1].tobytes())
        meta["rocas"] = {
            "archivo": "rocas.bin", "archivo_entorno": "rocas_contexto.bin",
            "codificacion": "uint8, misma grilla que altura.bin / contexto.bin: rocosidad 0-255 = 0-100 % de roca a la vista",
            "fuente": "grupos CONEAT (SNIG), rocosidad aproximada por zona: ver docs/sistemas/suelos-y-roca.md",
            "grupos_en_la_ventana": rocas[2],
        }
    guardar_json(os.path.join(args.salida, "meta.json"), meta, compacto=False)
    # Vista previa: relieve sombreado con agua, biomas oficiales y arroyos.
    gris = np.clip(h / 80.0, 0, 1)
    rgb = np.stack([0.35 + 0.5 * gris, 0.45 + 0.45 * gris, 0.25 + 0.3 * gris], axis=-1)
    if hidro:
        b = hidro["biomas"]
        rgb[b == BIOMA_BANADO] = (0.30, 0.45, 0.35)
        rgb[b == BIOMA_PLAYA] = (0.93, 0.85, 0.55)
        rgb[b == BIOMA_BANCO] = (0.85, 0.80, 0.60)
        rgb[b == BIOMA_LAGUNA] = (0.30, 0.45, 0.75)
    rgb[h <= 0.0] = (0.55, 0.45, 0.32)
    img = Image.fromarray((rgb * 255).astype(np.uint8))
    from PIL import ImageDraw
    d = ImageDraw.Draw(img)
    fuentes_l = hidro["arroyos"]["fuente"] if hidro else ["relieve"] * len(polis)
    for p, fu in zip(polis, fuentes_l):
        d.line([(q[0] / STEP, q[1] / STEP) for q in p], fill=(40, 90, 200) if fu == "ide" else (200, 60, 160),
               width=max(1, int(max(q[2] for q in p) / 6)))
    d.ellipse([sitio[1] / STEP - 5, sitio[2] / STEP - 5, sitio[1] / STEP + 5, sitio[2] / STEP + 5], outline=(200, 0, 0), width=2)
    previa = os.path.join(os.path.dirname(__file__), "..", "..", "docs", "capturas", "mapa-real-relieve.jpg")
    img.convert("RGB").save(previa, quality=85)
    print(json.dumps({k: v for k, v in meta.items() if k not in ("fuentes",)}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
