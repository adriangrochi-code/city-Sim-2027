#!/usr/bin/env python3
"""
Construye el mapa real de Montevideo para el prototipo (y luego para el motor de Rust).

Fuente: "Terrain Tiles" públicos de AWS (formato terrarium, Mapzen/Tilezen), que combinan
SRTM, GMTED2010, ETOPO1, NED y otras fuentes. Ver docs/sistemas/mapa-real.md (atribución).

Salida en assets/mapas/montevideo/:
  altura.bin        alturas del mapa jugable: uint16 little-endian, fila por fila (norte → sur),
                    columna por columna (oeste → este); metros = valor / 200 − 50.
  contexto.bin      lo mismo, más grueso, para el anillo exterior del terreno.
  arroyos.json      arroyos calculados desde el relieve (polilíneas en metros locales).
  meta.json         origen geográfico, escala, tamaños, codificación y puntos de referencia.
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
import urllib.request

import numpy as np
from PIL import Image

# --- Ventana del mapa -------------------------------------------------------------------
MAP_SIZE = 24000.0      # metros
STEP = 20.0             # resolución del mapa jugable
OUTER = 6000.0          # anillo exterior (dos parcelas de 3 km)
OUTER_STEP = 100.0
LAT_N = -34.778         # borde norte
LON_W = -56.325         # borde oeste
ZOOM = 13
URL = "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{z}/{x}/{y}.png"

# Puntos de referencia (lat, lon).
PLAZA_MATRIZ = (-34.9068, -56.2036)
CERRO = (-34.8886, -56.2602)

# Umbral de cuenca para considerar un arroyo (km²).
ARROYO_KM2 = 6.0
ARROYO_MIN_LEN = 600.0


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
            pts.append((j * step, i * step))
            if usados[c] or agua.ravel()[c]:
                break
            usados[c] = True
            c = flat_dest[c]
        if len(pts) > 1:
            lineas.append(pts)
    # Ordena para que los tramos más largos (los cauces principales) se tracen primero.
    return lineas, acc.reshape(h.shape)


def rdp(pts, tol):
    if len(pts) < 3:
        return pts
    a, b = np.array(pts[0]), np.array(pts[-1])
    ab = b - a
    n = np.linalg.norm(ab)
    arr = np.array(pts)
    if n < 1e-9:
        d = np.linalg.norm(arr - a, axis=1)
    else:
        v = arr - a
        d = np.abs(ab[0] * v[:, 1] - ab[1] * v[:, 0]) / n
    k = int(np.argmax(d))
    if d[k] > tol:
        return rdp(pts[:k + 1], tol)[:-1] + rdp(pts[k:], tol)
    return [pts[0], pts[-1]]


def suavizar(pts, veces=2):
    for _ in range(veces):
        if len(pts) < 3:
            return pts
        out = [pts[0]]
        for p, q in zip(pts[:-1], pts[1:]):
            out.append((0.75 * p[0] + 0.25 * q[0], 0.75 * p[1] + 0.25 * q[1]))
            out.append((0.25 * p[0] + 0.75 * q[0], 0.25 * p[1] + 0.75 * q[1]))
        out.append(pts[-1])
        pts = out
    return pts


def largo(pts):
    return sum(math.dist(p, q) for p, q in zip(pts[:-1], pts[1:]))


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


def guardar_u16(ruta, h):
    v = np.clip(np.round((h + 50.0) * 200.0), 0, 65535).astype("<u2")
    with open(ruta, "wb") as f:
        f.write(v.tobytes())


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cache", required=True, help="carpeta vacía o con tiles ya bajados")
    ap.add_argument("--salida", default=os.path.join(os.path.dirname(__file__), "..", "..", "assets", "mapas", "montevideo"))
    args = ap.parse_args()
    os.makedirs(args.cache, exist_ok=True)
    os.makedirs(args.salida, exist_ok=True)

    print("Bajando tiles…", flush=True)
    alto, ox, oy, n = mosaico(args.cache)
    print(f"  {n} tiles, mosaico {alto.shape}", flush=True)

    xs = np.arange(0.0, MAP_SIZE + STEP / 2, STEP)
    h = muestrear(alto, ox, oy, xs, xs)
    xs_o = np.arange(-OUTER, MAP_SIZE + OUTER + OUTER_STEP / 2, OUTER_STEP)
    h_o = muestrear(alto, ox, oy, xs_o, xs_o)
    print(f"  mapa {h.shape} min {h.min():.1f} máx {h.max():.1f}", flush=True)
    h = limpiar_agua(h, STEP)
    h = suavizar_tierra(h)
    h_o = limpiar_agua(h_o, OUTER_STEP)

    print("Calculando arroyos…", flush=True)
    lineas, acc = arroyos(h, STEP)
    polis = []
    for pts in lineas:
        p = suavizar(rdp(pts, STEP * 0.75), 2)
        if largo(p) >= ARROYO_MIN_LEN:
            polis.append([[round(x, 1), round(z, 1)] for x, z in p])
    polis.sort(key=lambda p: -largo(p))
    print(f"  {len(polis)} tramos de arroyo", flush=True)

    sitio = sitio_fundacion(h, STEP)
    cx, cz = a_local(*CERRO)
    meta = {
        "nombre": "Montevideo (geografía real)",
        "fuente": "AWS Terrain Tiles (terrarium), zoom %d" % ZOOM,
        "atribucion": "Datos de elevación: Terrain Tiles de Mapzen/Tilezen en AWS, que incluyen SRTM (NASA), GMTED2010 (USGS), ETOPO1 (NOAA) y otras fuentes. Ver docs/sistemas/mapa-real.md.",
        "origen_noroeste": {"lat": LAT_N, "lon": LON_W},
        "metros_por_grado": {"lon": KX, "lat": KY},
        "tamano_m": MAP_SIZE,
        "paso_m": STEP,
        "muestras": int(h.shape[0]),
        "contexto": {"desde_m": -OUTER, "paso_m": OUTER_STEP, "muestras": int(h_o.shape[0])},
        "codificacion": "uint16 little-endian; metros = valor / 200 - 50; filas de norte a sur",
        "fundacion": {"x": round(sitio[1], 1), "z": round(sitio[2], 1), "desnivel_m": round(sitio[3], 2)},
        "cerro": {"x": round(cx, 1), "z": round(cz, 1), "altura_m": round(float(h[int(cz / STEP), int(cx / STEP)]), 1)},
    }
    guardar_u16(os.path.join(args.salida, "altura.bin"), h)
    guardar_u16(os.path.join(args.salida, "contexto.bin"), h_o)
    with open(os.path.join(args.salida, "arroyos.json"), "w") as f:
        json.dump({"arroyos": polis}, f, separators=(",", ":"))
    with open(os.path.join(args.salida, "meta.json"), "w") as f:
        json.dump(meta, f, ensure_ascii=False, indent=2)
    # Vista previa: relieve sombreado con agua azul y arroyos.
    gris = np.clip(h / 80.0, 0, 1)
    rgb = np.stack([0.35 + 0.5 * gris, 0.45 + 0.45 * gris, 0.25 + 0.3 * gris], axis=-1)
    rgb[h <= 0.3] = (0.55, 0.45, 0.32)
    img = Image.fromarray((rgb * 255).astype(np.uint8))
    from PIL import ImageDraw
    d = ImageDraw.Draw(img)
    for p in polis:
        d.line([(x / STEP, z / STEP) for x, z in p], fill=(40, 90, 200), width=2)
    d.ellipse([sitio[1] / STEP - 5, sitio[2] / STEP - 5, sitio[1] / STEP + 5, sitio[2] / STEP + 5], outline=(200, 0, 0), width=2)
    previa = os.path.join(os.path.dirname(__file__), "..", "..", "docs", "capturas", "mapa-real-relieve.jpg")
    img.convert("RGB").save(previa, quality=85)
    print(json.dumps(meta, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
