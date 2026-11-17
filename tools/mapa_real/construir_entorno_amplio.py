#!/usr/bin/env python3
"""Entorno amplio del mapa real: el relieve de 300 × 300 km alrededor de Montevideo.

El mapa jugable mide 24 km (altura.bin, 20 m) y el entorno 74 km (contexto.bin, 100 m). Desde
lo alto, o mirando hacia afuera desde un borde, se veía la tierra terminar en una línea recta
contra el plano del río. Este paso agrega una tercera grilla, gruesa (500 m), que llega a unos
150 km del centro del mapa en todas las direcciones: Colonia y San José al oeste, Florida al
norte, Maldonado y Punta del Este al este y, cruzando el Río de la Plata, la costa argentina de
Magdalena, Punta Indio y la bahía de Samborombón.

Fuentes:
  - Uruguay: MDT nacional de 30 m de la IDE Uruguay (WCS raster__mdt_pais_30m_4326), pedido ya
    reducido por el servidor (scaleFactor) en recortes de medio grado. El agua (río, mar,
    lagunas) viene sin dato.
  - Argentina: Copernicus DEM GLO-90 (ESA, 90 m, superficie), recortes de 1° en
    copernicus-dem-90m.s3.amazonaws.com. El mar viene en 0 exacto. Solo se usa al sur del
    Río de la Plata (donde la IDE no tiene datos de tierra).
  - Cobertura: monte nativo (DGF 2021, SNIG, polígonos de 3 ha o más, simplificados) y bañados
    y áreas húmedas de la hidrografía nacional de la IDE. Si un servicio no responde se sigue
    sin esa capa (el motor usa entonces solo la altura).

Salida (en --salida, por omisión assets/mapas/montevideo):
  amplio.bin             uint16 LE, n × n, metros = valor / 100 - 50 (hasta 605 m: las sierras
                         de Maldonado pasan los 277 m que admite la codificación de altura.bin),
                         filas de norte a sur como altura.bin; agua < 0.
  amplio_cobertura.bin   2 bytes por muestra: bañado (0..255) y monte nativo (0..255).
  meta.json              se agrega la sección "amplio".

Uso:
  python3 tools/mapa_real/construir_entorno_amplio.py --cache /ruta/para/la/cache
"""

import argparse
import json
import math
import os
import sys
from collections import deque

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import construir_mapa as cm  # noqa: E402

# Grilla amplia: centrada en el centro del mapa jugable.
PASO = 500.0
RADIO = 150000.0
DESDE = cm.MAP_SIZE / 2 - RADIO          # -138 000 m
N = int(round(2 * RADIO / PASO)) + 1     # 601
SUB = 5                                  # muestras finas (100 m) por celda para promediar
ESCALA_IDE = 0.25                        # 30 m → ~120 m por píxel del recorte de la IDE
COPERNICUS = ("https://copernicus-dem-90m.s3.amazonaws.com/Copernicus_DSM_COG_30_{ns}{lat:02d}_00_{ew}{lon:03d}_00_DEM/"
              "Copernicus_DSM_COG_30_{ns}{lat:02d}_00_{ew}{lon:03d}_00_DEM.tif")
BOSQUE_MIN_HA = 3
# Rectángulo del MDT nacional (lat_s, lat_n, lon_o, lon_e), según DescribeCoverage.
IDE_COBERTURA = (-34.99696, -30.07781, -58.38600, -53.02400)


def bbox():
    """(lat_s, lat_n, lon_o, lon_e) de la grilla amplia, con un margen."""
    lat_n, lon_o = cm.a_geo(DESDE, DESDE)
    lat_s, lon_e = cm.a_geo(DESDE + PASO * (N - 1), DESDE + PASO * (N - 1))
    return lat_s - 0.02, lat_n + 0.02, lon_o - 0.02, lon_e + 0.02


def recortes_ide(cache):
    """Recortes de medio grado del MDT de la IDE (reducidos por el servidor). Devuelve una lista
    de (arreglo, lon0, lat0, dlon, dlat)."""
    lat_s, lat_n, lon_o, lon_e = bbox()
    out = []
    la = math.floor(lat_s * 2) / 2
    while la < lat_n:
        lo = math.floor(lon_o * 2) / 2
        while lo < lon_e:
            ruta = os.path.join(cache, f"amplio_ide_{la:.1f}_{lo:.1f}_{ESCALA_IDE}.tif")
            # Recorte dentro de la cobertura del MDT (fuera de ella el servidor da 404).
            r_s, r_n = max(la, IDE_COBERTURA[0]), min(la + 0.5, IDE_COBERTURA[1])
            r_o, r_e = max(lo, IDE_COBERTURA[2]), min(lo + 0.5, IDE_COBERTURA[3])
            if r_s >= r_n or r_o >= r_e:
                lo += 0.5
                continue
            if not os.path.exists(ruta):
                url = cm.IDE_WCS.format(lat_s=f"{r_s:.5f}", lat_n=f"{r_n:.5f}", lon_w=f"{r_o:.5f}", lon_e=f"{r_e:.5f}")
                url += f"&scaleFactor={ESCALA_IDE}"
                datos = cm._bajar(url, timeout=300)
                if not datos.startswith((b"II", b"MM")):
                    # Fuera de la cobertura (mar abierto): el servidor responde con un error.
                    datos = b""
                with open(ruta, "wb") as f:
                    f.write(datos)
            if os.path.getsize(ruta) > 0:
                im = Image.open(ruta)
                t = im.tag_v2[34264]
                out.append((np.asarray(im, dtype=np.float32), t[3], t[7], t[0], -t[5]))
            lo += 0.5
        la += 0.5
    return out


def recortes_copernicus(cache):
    """Recortes de 1° de Copernicus GLO-90 que tocan la grilla (los que no existen son mar)."""
    lat_s, lat_n, lon_o, lon_e = bbox()
    out = []
    for la in range(math.floor(lat_s), math.ceil(lat_n)):
        for lo in range(math.floor(lon_o), math.ceil(lon_e)):
            ns, ew = ("S", -la) if la < 0 else ("N", la), ("W", -lo) if lo < 0 else ("E", lo)
            url = COPERNICUS.format(ns=ns[0], lat=ns[1], ew=ew[0], lon=ew[1])
            ruta = os.path.join(cache, os.path.basename(url))
            if not os.path.exists(ruta):
                try:
                    import urllib.request
                    with urllib.request.urlopen(url, timeout=300) as r:
                        datos = r.read()
                except Exception as e:  # noqa: BLE001 (404 = recorte de mar)
                    print(f"    {os.path.basename(url)}: {e}", flush=True)
                    datos = b""
                with open(ruta, "wb") as f:
                    f.write(datos)
            if os.path.getsize(ruta) > 0:
                im = Image.open(ruta)
                sc, tp = im.tag_v2[33550], im.tag_v2[33922]
                out.append((np.asarray(im, dtype=np.float32), tp[3], tp[4], sc[0], sc[1]))
    return out


def muestrear(recortes, lat, lon, nodata):
    """Bilineal sobre los recortes (lat/lon regulares). Devuelve (altura, válido): una muestra
    vale si sus cuatro vecinos tienen dato."""
    h = np.zeros(lat.shape, np.float32)
    ok = np.zeros(lat.shape, bool)
    for a, lon0, lat0, dlon, dlat in recortes:
        fi = (lat0 - lat) / dlat - 0.5
        fj = (lon - lon0) / dlon - 0.5
        den = (fi >= 0) & (fj >= 0) & (fi < a.shape[0] - 1) & (fj < a.shape[1] - 1) & ~ok
        if not den.any():
            continue
        i0 = np.floor(fi[den]).astype(int)
        j0 = np.floor(fj[den]).astype(int)
        ty, tx = fi[den] - i0, fj[den] - j0
        v = [a[i0, j0], a[i0, j0 + 1], a[i0 + 1, j0], a[i0 + 1, j0 + 1]]
        bueno = np.ones(i0.shape, bool)
        for k in v:
            bueno &= ~nodata(k)
        val = (v[0] * (1 - tx) + v[1] * tx) * (1 - ty) + (v[2] * (1 - tx) + v[3] * tx) * ty
        idx = np.flatnonzero(den)
        h.flat[idx] = np.where(bueno, val, 0.0)
        ok.flat[idx] = bueno
        # Las muestras sin dato de este recorte pueden tener dato en el vecino: se dejan sin marcar.
    return h, ok


def profundidad(agua):
    """Fondo del agua según la distancia a la tierra (como limpiar_agua de construir_mapa)."""
    n = agua.shape[0]
    dist = np.full(agua.shape, -1, np.int32)
    q = deque()
    for i, j in zip(*np.nonzero(~agua)):
        dist[i, j] = 0
        q.append((i, j))
    while q:
        i, j = q.popleft()
        for di, dj in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            a, b = i + di, j + dj
            if 0 <= a < n and 0 <= b < n and dist[a, b] < 0:
                dist[a, b] = dist[i, j] + 1
                q.append((a, b))
    dist[dist < 0] = 10 ** 6
    return -np.minimum(0.4 + dist * PASO * 0.004, 7.5)


def componentes_chicas(agua, max_celdas):
    """Máscara de los grupos de agua (vecindad 4) de menos de `max_celdas` que no tocan el borde."""
    n = agua.shape[0]
    visto = np.zeros(agua.shape, bool)
    chicas = np.zeros(agua.shape, bool)
    for i0, j0 in zip(*np.nonzero(agua)):
        if visto[i0, j0]:
            continue
        grupo, q, borde = [], deque([(i0, j0)]), False
        visto[i0, j0] = True
        while q:
            i, j = q.popleft()
            grupo.append((i, j))
            borde |= i in (0, n - 1) or j in (0, n - 1)
            for di, dj in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                a, b = i + di, j + dj
                if 0 <= a < n and 0 <= b < n and agua[a, b] and not visto[a, b]:
                    visto[a, b] = True
                    q.append((a, b))
        if len(grupo) < max_celdas and not borde:
            for i, j in grupo:
                chicas[i, j] = True
    return chicas


def bajar_bosque_amplio(cache):
    ruta = os.path.join(cache, f"amplio_monte_nativo_{BOSQUE_MIN_HA}ha.geojson")
    if os.path.exists(ruta):
        with open(ruta) as f:
            return json.load(f)["features"]
    lat_s, lat_n, lon_o, lon_e = bbox()
    base = cm.SNIG_BOSQUE.replace("where=1%3D1", f"where=HA%3E%3D{BOSQUE_MIN_HA}") + "&maxAllowableOffset=0.0008"
    feats, inicio, pagina = [], 0, 1000
    while True:
        url = f"{base}&geometry={lon_o:.5f},{lat_s:.5f},{lon_e:.5f},{lat_n:.5f}&resultOffset={inicio}&resultRecordCount={pagina}"
        fs = json.loads(cm._bajar(url, timeout=180, intentos=4)).get("features", [])
        feats += fs
        print(f"    {len(feats)} polígonos de monte", flush=True)
        if len(fs) < pagina:
            break
        inicio += len(fs)
    with open(ruta, "w") as f:
        json.dump({"type": "FeatureCollection", "features": feats}, f)
    return feats


def bajar_wfs_amplio(cache, capa):
    ruta = os.path.join(cache, "amplio_wfs_" + capa.replace(":", "__") + ".geojson")
    if os.path.exists(ruta):
        with open(ruta) as f:
            return json.load(f)["features"]
    lat_s, lat_n, lon_o, lon_e = bbox()
    feats, inicio, pagina = [], 0, 500
    while True:
        url = (f"{cm.IDE_WFS}&typeNames={capa}&outputFormat=application/json&srsName=EPSG:4326"
               f"&bbox={lon_o:.5f},{lat_s:.5f},{lon_e:.5f},{lat_n:.5f},EPSG:4326&count={pagina}&startIndex={inicio}")
        fs = json.loads(cm._bajar(url, timeout=90, intentos=4)).get("features", [])
        feats += fs
        if len(fs) < pagina:
            break
        inicio += pagina
    with open(ruta, "w") as f:
        json.dump({"type": "FeatureCollection", "features": feats}, f)
    return feats


def poligonos(feats):
    out = []
    for f in feats:
        out += cm._poligonos(f.get("geometry"))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cache", required=True)
    ap.add_argument("--sin-hidrografia", action="store_true",
                    help="no pedir los bañados a la IDE (solo los de la caché y los del relieve)")
    ap.add_argument("--salida", default=os.path.join(os.path.dirname(__file__), "..", "..", "assets", "mapas", "montevideo"))
    args = ap.parse_args()
    os.makedirs(args.cache, exist_ok=True)
    fuentes = []

    print(f"Grilla amplia: {N} × {N} muestras de {PASO:.0f} m desde {DESDE:.0f} m", flush=True)
    print("MDT de la IDE (recortes reducidos)…", flush=True)
    ide = recortes_ide(args.cache)
    print(f"  {len(ide)} recortes con datos", flush=True)
    print("Copernicus GLO-90…", flush=True)
    cop = recortes_copernicus(args.cache)
    print(f"  {len(cop)} recortes", flush=True)

    # Muestras finas (100 m) que se promedian por celda.
    nf = N * SUB
    f = (np.arange(nf) - (SUB - 1) / 2) * (PASO / SUB) + DESDE
    zz, xx = np.meshgrid(f, f, indexing="ij")
    lat = cm.LAT_N - zz / cm.KY
    lon = cm.LON_W + xx / cm.KX
    del zz, xx
    h_ide, ok_ide = muestrear(ide, lat, lon, lambda k: k < -1000)
    h_cop, ok_cop = muestrear(cop, lat, lon, lambda k: ~np.isfinite(k))
    # Argentina: al sur y al oeste del Río de la Plata (en esa zona no hay tierra uruguaya).
    arg = (lat < -34.85) & (lon < -56.5)
    tierra_cop = ok_cop & (h_cop > 0.0) & arg & ~ok_ide
    tierra = ok_ide | tierra_cop
    h = np.where(ok_ide, h_ide, np.where(tierra_cop, h_cop, 0.0))
    del lat, lon, h_ide, h_cop
    frac = tierra.reshape(N, SUB, N, SUB).mean(axis=(1, 3))
    suma = np.where(tierra, h, 0.0).reshape(N, SUB, N, SUB).sum(axis=(1, 3))
    cuenta = tierra.reshape(N, SUB, N, SUB).sum(axis=(1, 3))
    alto = np.where(cuenta > 0, suma / np.maximum(cuenta, 1), 0.0)
    agua = frac < 0.5
    # Pozos de una o dos celdas (sin dato aislado del MDT): tierra.
    chicas = componentes_chicas(agua, 3)
    agua &= ~chicas
    tierra_c = ~agua
    # Tierra: al menos medio metro (la costa queda sobre el nivel del agua).
    alto = np.where(tierra_c, np.maximum(alto, 0.6), 0.0)
    # Las celdas de tierra sin muestras (pozos rellenados) toman la media de sus vecinas.
    for _ in range(4):
        hueco = tierra_c & (cuenta == 0)
        if not hueco.any():
            break
        p = np.pad(np.where(cuenta > 0, alto, 0.0), 1)
        c = np.pad((cuenta > 0).astype(float), 1)
        s = sum(p[1 + di:N + 1 + di, 1 + dj:N + 1 + dj] for di in (-1, 0, 1) for dj in (-1, 0, 1))
        k = sum(c[1 + di:N + 1 + di, 1 + dj:N + 1 + dj] for di in (-1, 0, 1) for dj in (-1, 0, 1))
        alto = np.where(hueco & (k > 0), s / np.maximum(k, 1), alto)
        cuenta = np.where(hueco & (k > 0), 1, cuenta)
    alto = np.where(agua, profundidad(agua), alto)
    print(f"  tierra {tierra_c.mean() * 100:.1f} %, altura máx {alto.max():.0f} m", flush=True)
    fuentes.append({"capa": "raster__mdt_pais_30m_4326 (scaleFactor %.2f)" % ESCALA_IDE,
                    "servicio": "WCS https://mapas.ide.uy/geoserver-raster/wcs", "uso": "relieve de Uruguay (amplio.bin)"})
    if cop:
        fuentes.append({"capa": "Copernicus DEM GLO-90", "servicio": "https://copernicus-dem-90m.s3.amazonaws.com",
                        "uso": "relieve de la costa argentina (amplio.bin)"})

    # Cobertura: bañados y monte nativo.
    banado = np.zeros((N, N), np.float32)
    monte = np.zeros((N, N), np.float32)
    # Cada capa por separado: el servidor de la IDE corta seguido y una capa que no baja no
    # frena a las demás (se sigue sin ella y queda anotado en meta.json).
    polis, capas_ok = [], []
    for capa in ("hidrografia:banado_a", "hidrografia:area_humeda_a"):
        try:
            ruta = os.path.join(args.cache, "amplio_wfs_" + capa.replace(":", "__") + ".geojson")
            if args.sin_hidrografia and not os.path.exists(ruta):
                continue
            polis += poligonos(bajar_wfs_amplio(args.cache, capa))
            capas_ok.append(capa)
        except Exception as e:  # noqa: BLE001
            print(f"  sin {capa}: {e}", flush=True)
    if capas_ok:
        banado = cm.cobertura(polis, DESDE, PASO, N, 4)
        fuentes.append({"capa": ", ".join(capas_ok), "servicio": "WFS " + cm.IDE_WFS.split("?")[0],
                        "uso": "bañados del entorno amplio (amplio_cobertura.bin)"})
        print(f"  bañados: {len(polis)} polígonos, {(banado > 0.3).sum()} celdas", flush=True)
    # Bañados del relieve (donde la capa oficial no llega, como la costa argentina): tierra baja
    # (menos de 4 m) y llana (menos de 2 m entre vecinas) a menos de 3 km del agua.
    p = np.pad(alto, 1, mode="edge")
    vec = np.stack([p[1 + di:N + 1 + di, 1 + dj:N + 1 + dj] for di in (-1, 0, 1) for dj in (-1, 0, 1)])
    llano = (vec.max(axis=0) - vec.min(axis=0)) < 2.0
    cerca = cm.dilatar(agua, int(3000 / PASO))
    bajo = tierra_c & (alto < 4.0) & llano & cerca
    banado = np.maximum(banado, np.where(bajo, 0.6, 0.0))
    print(f"  bañados del relieve: {bajo.sum()} celdas", flush=True)
    try:
        polis = poligonos(bajar_bosque_amplio(args.cache))
        monte = cm.cobertura(polis, DESDE, PASO, N, 4)
        fuentes.append({"capa": f"DGF/Monte_Nativo_2021 (capa 1, polígonos de {BOSQUE_MIN_HA} ha o más)",
                        "servicio": "ArcGIS REST " + cm.SNIG_BOSQUE.split("?")[0], "uso": "monte nativo del entorno amplio (amplio_cobertura.bin)"})
        print(f"  monte: {len(polis)} polígonos, {(monte > 0.3).sum()} celdas", flush=True)
    except Exception as e:  # noqa: BLE001
        print(f"  sin monte: {e}", flush=True)
    banado[agua] = 0.0
    monte[agua] = 0.0

    v = np.clip(np.round((alto + 50.0) * 100.0), 0, 65535).astype("<u2")
    with open(os.path.join(args.salida, "amplio.bin"), "wb") as fh:
        fh.write(v.tobytes())
    cob = np.stack([np.round(banado * 255), np.round(np.minimum(monte * 1.5, 1.0) * 255)], axis=-1).astype(np.uint8)
    with open(os.path.join(args.salida, "amplio_cobertura.bin"), "wb") as fh:
        fh.write(cob.tobytes())
    ruta_meta = os.path.join(args.salida, "meta.json")
    with open(ruta_meta) as fh:
        meta = json.load(fh)
    meta["amplio"] = {
        "desde_m": DESDE, "paso_m": PASO, "muestras": N,
        "archivo": "amplio.bin", "archivo_cobertura": "amplio_cobertura.bin",
        "codificacion": "amplio.bin: uint16 little-endian, metros = valor / 100 - 50, filas de norte a sur; amplio_cobertura.bin: 2 bytes por muestra (bañado, monte nativo), 0..255",
        "fuentes": fuentes,
        "nota": "Argentina (sur del Río de la Plata): Copernicus DEM GLO-90 de la ESA (superficie, no suelo). "
                "© DLR e.V. 2010-2014 y © Airbus Defence and Space GmbH 2014-2018, provisto bajo COPERNICUS por la "
                "Unión Europea y la ESA. Ver docs/sistemas/mapa-real.md.",
    }
    cm.guardar_json(ruta_meta, meta, compacto=False)
    print("Listo.", flush=True)


if __name__ == "__main__":
    main()
