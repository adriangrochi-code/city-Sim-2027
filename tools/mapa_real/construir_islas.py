#!/usr/bin/env python3
"""Islas del Río de la Plata y del Atlántico alrededor del mapa, con su forma real.

Las grillas del relieve no alcanzan para las islas chicas: en la del entorno amplio (500 m) la
isla de Lobos o la Gorriti no ocupan ni una celda, y en la del entorno (100 m) la de Flores
quedó engrosada una celda para que no desapareciera. Este constructor guarda el contorno
oficial de cada isla (IDE Uruguay, WFS `hidrografia:isla_a`, la capa de islas de la
hidrografía nacional 1:50.000) y su altura (Copernicus DEM GLO-90, que a diferencia del MDT de
la IDE sí tiene datos sobre las islas) en `assets/mapas/montevideo/islas.json`; el motor
(`terrain.rs`) levanta cada isla sobre el agua siguiendo su contorno, en cualquier grilla.

La capa de la IDE no trae nombres: cada isla se identifica por su posición (los nombres van a
mano, con la fuente que los ubica). Las islas de un grupo se eligen por cercanía al punto de
referencia.

    python3 tools/mapa_real/construir_islas.py --cache /ruta/a/una/carpeta

La primera vez baja ~0,5 MB de la IDE y ~18 MB de Copernicus (cinco recortes de 1°).
"""

import argparse
import json
import math
import os
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import construir_mapa as cm  # noqa: E402

# (nombre, grupo, lon, lat de referencia, radio de búsqueda en m, recortar, nota, terreno)
# `recortar`: la grilla del entorno (100 m) o la del entorno amplio (500 m) ya tiene esta
# isla engrosada (una o dos celdas de tierra); el motor recorta lo que sobra fuera del contorno.
ISLAS = [
    ("Isla de Flores", "Flores", -55.926, -34.942, 1500.0, True,
     "Dos islas, frente a la costa este de Montevideo (en la grilla del entorno de 100 m)", True),
    ("Isla Gorriti", "Maldonado", -54.973, -34.956, 1200.0, True,
     "En la bahía de Maldonado, frente a Punta del Este (grilla del entorno amplio)", True),
    ("Isla de Lobos", "Maldonado", -54.884, -35.027, 1500.0, True,
     "Mar adentro de Punta del Este, con el islote vecino (grilla del entorno amplio)", True),
    # `terreno` = False: solo para el mapa del país. La de las Ratas ya está en la grilla del
    # mapa (20 m). Las de Colonia (en x ≈ −143 km) quedan fuera de la grilla del entorno amplio
    # (que empieza en x = −138 km): ahí el relieve ya no es real (se funde con una llanura) y no
    # tiene sentido levantarlas en el terreno.
    ("Isla de las Ratas", "Montevideo", -56.233, -34.889, 400.0, False,
     "En la bahía de Montevideo (ya en la grilla del mapa jugable)", False),
    ("Isla San Gabriel", "Colonia", -57.889, -34.474, 900.0, False,
     "Frente a Colonia del Sacramento", False),
    ("Isla Farallón", "Colonia", -57.918, -34.485, 500.0, False,
     "Al suroeste de San Gabriel", False),
]

# Recuadros (lon_o, lat_s, lon_e, lat_n) para pedir la capa por partes (la IDE no responde a
# un recuadro que cubra todo el Río de la Plata).
RECUADROS = {
    "Flores": (-56.0, -35.0, -55.85, -34.9),
    "Maldonado": (-55.2, -35.1, -54.8, -34.8),
    "Montevideo": (-56.30, -34.92, -56.18, -34.86),
    "Colonia": (-58.0, -34.6, -57.6, -34.35),
}

COPERNICUS = ("https://copernicus-dem-90m.s3.amazonaws.com/Copernicus_DSM_COG_30_{t}_00_DEM/"
              "Copernicus_DSM_COG_30_{t}_00_DEM.tif")
RECORTES = ["S35_00_W055", "S36_00_W055", "S35_00_W056", "S35_00_W057", "S35_00_W058"]


def islas_ide(cache, grupo):
    ruta = os.path.join(cache, f"islas_ide_{grupo}.json")
    if not os.path.exists(ruta):
        lo, la_s, le, la_n = RECUADROS[grupo]
        url = (f"{cm.IDE_WFS}&typeNames=hidrografia:isla_a&outputFormat=application/json&srsName=EPSG:4326"
               f"&bbox={lo},{la_s},{le},{la_n},EPSG:4326&count=2000")
        with open(ruta, "wb") as f:
            f.write(cm._bajar(url))
    with open(ruta) as f:
        return json.load(f)["features"]


def recortes(cache):
    out = []
    for t in RECORTES:
        ruta = os.path.join(cache, f"cop_{t}.tif")
        if not os.path.exists(ruta):
            try:
                datos = cm._bajar(COPERNICUS.format(t=t), intentos=2)
            except Exception as e:  # noqa: BLE001 (404 = recorte de mar)
                print(f"    {t}: {e}", flush=True)
                datos = b""
            with open(ruta, "wb") as f:
                f.write(datos)
        if os.path.getsize(ruta) > 0:
            im = Image.open(ruta)
            sc, tp = im.tag_v2[33550], im.tag_v2[33922]
            out.append((np.asarray(im, dtype=np.float32), tp[3], tp[4], sc[0], sc[1]))
    return out


def anillos(f):
    g = f["geometry"]
    polys = g["coordinates"] if g["type"] == "MultiPolygon" else [g["coordinates"]]
    return [p[0] for p in polys]


def area(pts):
    a = 0.0
    for i in range(len(pts)):
        x1, z1 = pts[i]
        x2, z2 = pts[(i + 1) % len(pts)]
        a += x1 * z2 - x2 * z1
    return abs(a) * 0.5


def dentro(x, z, pts):
    c = False
    j = len(pts) - 1
    for i in range(len(pts)):
        xi, zi = pts[i]
        xj, zj = pts[j]
        if (zi > z) != (zj > z) and x < (xj - xi) * (z - zi) / (zj - zi) + xi:
            c = not c
        j = i
    return c


def alturas(rec, anillo_geo):
    """Alturas de Copernicus (superficie) de los píxeles con el centro dentro del contorno; si
    no hay ninguno (isla más chica que un píxel), el más cercano al centro."""
    lons = [p[0] for p in anillo_geo]
    lats = [p[1] for p in anillo_geo]
    vals = []
    for a, lon0, lat0, dlon, dlat in rec:
        i0 = int((lat0 - max(lats)) / dlat) - 1
        i1 = int((lat0 - min(lats)) / dlat) + 2
        j0 = int((min(lons) - lon0) / dlon) - 1
        j1 = int((max(lons) - lon0) / dlon) + 2
        if i1 < 0 or j1 < 0 or i0 >= a.shape[0] or j0 >= a.shape[1]:
            continue
        for i in range(max(i0, 0), min(i1, a.shape[0])):
            for j in range(max(j0, 0), min(j1, a.shape[1])):
                lat = lat0 - (i + 0.5) * dlat
                lon = lon0 + (j + 0.5) * dlon
                if dentro(lon, lat, [(p[0], p[1]) for p in anillo_geo]):
                    vals.append(float(a[i, j]))
        if not vals:
            cl, ca = sum(lons) / len(lons), sum(lats) / len(lats)
            i = int((lat0 - ca) / dlat)
            j = int((cl - lon0) / dlon)
            if 0 <= i < a.shape[0] and 0 <= j < a.shape[1]:
                vals.append(float(a[i, j]))
    return vals


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--cache", required=True)
    ap.add_argument("--salida", default=os.path.join(os.path.dirname(__file__), "..", "..", "assets", "mapas", "montevideo", "islas.json"))
    args = ap.parse_args()
    os.makedirs(args.cache, exist_ok=True)
    rec = recortes(args.cache)
    por_grupo = {}
    out = []
    usadas = set()
    for nombre, grupo, lon, lat, radio, recortar, nota, terreno in ISLAS:
        if grupo not in por_grupo:
            por_grupo[grupo] = islas_ide(args.cache, grupo)
        cx, cz = cm.a_local(lat, lon)
        partes = []
        for f in por_grupo[grupo]:
            for k, an in enumerate(anillos(f)):
                pts = [cm.a_local(p[1], p[0]) for p in an]
                mx = sum(p[0] for p in pts) / len(pts)
                mz = sum(p[1] for p in pts) / len(pts)
                clave = (f.get("id"), k)
                if clave in usadas or math.hypot(mx - cx, mz - cz) > radio or area(pts) < 3000.0:
                    continue
                usadas.add(clave)
                simple = cm.rdp([(round(x, 1), round(z, 1)) for x, z in pts], 3.0)
                if simple[0] == simple[-1]:
                    simple = simple[:-1]
                hs = alturas(rec, an)
                alto = max(hs) if hs else 0.0
                partes.append({
                    "id_ide": f.get("id"),
                    "area_ha": round(area(pts) / 1e4, 1),
                    "alto_m": round(max(alto, 0.0), 1),
                    "alto_medio_m": round(max(sum(hs) / len(hs), 0.0), 1) if hs else 0.0,
                    "pixeles": len(hs),
                    "poligono": [[round(x, 1), round(z, 1)] for x, z in simple],
                    # Para el mapa del país (lon, lat), más simplificado (20 m).
                    "geo": [[round(cm.a_geo(x, z)[1], 6), round(cm.a_geo(x, z)[0], 6)] for x, z in cm.rdp(simple, 20.0)],
                })
        if not partes:
            print(f"  {nombre}: sin polígono en la capa de la IDE", flush=True)
            continue
        partes.sort(key=lambda p: -p["area_ha"])
        total = sum(p["area_ha"] for p in partes)
        print(f"  {nombre}: {len(partes)} partes, {total:.1f} ha, alto {max(p['alto_m'] for p in partes)} m "
              f"(Copernicus), centro {cx:.0f}, {cz:.0f}", flush=True)
        out.append({"nombre": nombre, "grupo": grupo, "x": round(cx, 1), "z": round(cz, 1), "recortar": recortar,
                    "terreno": terreno, "lon": lon, "lat": lat, "nota": nota, "partes": partes})
    doc = {
        "fuente": "Contornos: IDE Uruguay, WFS hidrografia:isla_a (hidrografía nacional 1:50.000), Licencia de "
                  "Datos Abiertos del Uruguay. Alturas: Copernicus DEM GLO-90 (superficie; © DLR e.V. 2010-2014 y "
                  "© Airbus Defence and Space GmbH 2014-2018, provisto bajo COPERNICUS por la Unión Europea y la ESA). "
                  "Nombres a mano, por posición. Ver docs/sistemas/mapa-real.md#islas.",
        "coordenadas": "metros locales del mapa (x este, z sur, origen en lat -34.778, lon -56.325)",
        "islas": out,
    }
    with open(args.salida, "w") as f:
        json.dump(doc, f, ensure_ascii=False, separators=(",", ":"))
    print(f"Escrito {os.path.relpath(args.salida)} ({os.path.getsize(args.salida) // 1024} KB)")


if __name__ == "__main__":
    main()
