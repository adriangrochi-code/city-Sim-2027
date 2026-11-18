#!/usr/bin/env python3
"""Mapa del país (escala país): el Uruguay entero a ~1 km para el modo «Mapa del país».

Usa la MISMA grilla que la geología del país (`assets/datos/uruguay/pais.json`: 0,0107° de
longitud × 0,009° de latitud, ~1 km, 510 × 557 celdas desde 58,5° O y 30,05° S), así que todas
las capas del país (relieve, agua, departamentos, monte, bañados, geología) coinciden celda a
celda.

Fuentes (todas se guardan en la caché; con la caché completa se arma sin red):
  - Relieve: MDT de 30 m de la IDE Uruguay (WCS `raster__mdt_pais_30m_4326`), pedido ya reducido
    por el servidor (`scaleFactor=0.1`, ~300 m) en recortes de 1°. El agua y lo que está fuera
    del país vienen sin dato.
  - Agua: polígonos de hidrografía del MTOP (WFS `dnv_sgcm:hidro_pg`, de más de 0,5 km²): Río
    Uruguay, Río de la Plata, Océano Atlántico, Laguna Merín, lagunas costeras, embalses.
  - Departamentos: MTOP (WFS `geoportal_capas_base:departamentos`).
  - Ríos: cursos navegables y flotables del MTOP (WFS `rec_hidrograficos:cursos_nav_flot`).
  - Ferrocarril: vías principales activas e inactivas y estaciones de AFE (WFS del MTOP).
  - Puertos de hoy: MTOP (WFS `inf_tte_ttelog_fluvial:puertos`), solo como referencia.
  - Localidades (polígonos, para ubicar los pueblos históricos): MTOP `dnv_sgcm:localidades_polig`.
  - Bañados: IDE (WFS `hidrografia:banado_a`).
  - Monte nativo: DGF 2021 (SNIG, ArcGIS REST), polígonos de 30 ha o más, simplificados.
  - Historia (pueblos con su año de fundación, caminos reales, vías férreas por año, puertos,
    estancias y saladeros de muestra, crecientes y sequías): tablas a mano con fuentes en
    `historia_pais.py`.

Salida (en --salida, por omisión assets/mapas/uruguay):
  relieve.bin   uint16 LE, filas de norte a sur, metros = valor / 10 - 100 (0,1 m; agua < 0).
                En los embalses el relieve es el del valle antes de la represa (interpolado).
  capas.bin     8 bytes por celda: 0 agua natural (0..255, fracción de la celda), 1 embalse
                (índice + 1 en `embalses` de mapa.json, 0 = no), 2 departamento (1..19, 0 =
                fuera), 3 bañado (0..255), 4 monte nativo (0..255), 5 río (0..255: cauce de un
                río con nombre que cruza la celda), 6 pendiente (‰, saturada a 255), 7 libre.
  mapa.json     grilla, departamentos (contorno simplificado y capital), ríos con nombre,
                embalses con año, vías férreas (con año de habilitación), estaciones, puertos de
                hoy, lagunas con nombre, fuentes y atribución.
  historia.json pueblos, caminos, puertos históricos, estancias, saladeros y eventos (ver
                historia_pais.py) con sus coordenadas en la grilla.
Y la vista previa docs/capturas/mapa-pais-datos.jpg.

Uso:
  python3 tools/mapa_real/construir_pais.py --cache /ruta/a/la/cache

Ver docs/sistemas/mapa-pais.md.
"""

import argparse
import json
import math
import os
import sys
import urllib.parse
from collections import deque

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import construir_mapa as cm  # noqa: E402
import historia_pais as hp  # noqa: E402

RAIZ = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")

# --- Grilla (la de geologia_pais.bin) ----------------------------------------------------------
LON0, LAT0 = -58.50, -30.05
DLON, DLAT = 0.0107, 0.009
NX, NY = 510, 557
SUB = 4                      # submuestras por lado para fracciones (agua, monte, bañado)

IDE_ESCALA = 0.1             # 30 m → ~300 m por píxel del recorte
MTOP_WFS = ("https://geoservicios.mtop.gub.uy/geoserver/wfs?service=WFS&version=2.0.0&request=GetFeature"
            "&outputFormat=application/json&srsName=EPSG:4326")
CAPAS_MTOP = {
    "departamentos": ("geoportal_capas_base:departamentos", None),
    "agua": ("dnv_sgcm:hidro_pg", "area>500000"),
    "rios": ("rec_hidrograficos:cursos_nav_flot", None),
    "via_activa": ("inf_tte_ttelog_ferroviario:via_prin_act", None),
    "via_inactiva": ("inf_tte_ttelog_ferroviario:via_prin_inactivas", None),
    "estaciones": ("inf_tte_ttelog_ferroviario:afe_estaciones", None),
    "estaciones_inactivas": ("inf_tte_ttelog_ferroviario:afe_estaciones_inactivas", None),
    "puertos": ("inf_tte_ttelog_fluvial:puertos", None),
    "localidades": ("dnv_sgcm:localidades_polig", None),
}
MONTE_MIN_HA = 30

# Embalses (nombre en hidro_pg → nombre en el juego, año de la represa, fuente). Antes de ese año
# el lugar es valle con su río. Salto Grande es parte del polígono del Río Uruguay: se marca
# aparte (ver `embalse_salto_grande`).
EMBALSES = {
    "DE RINCÓN DEL BONETE": ("Rincón del Bonete (Gabriel Terra)", 1945),
    "DE BAYGORRIA": ("Baygorria", 1960),
    "DE PASO DEL PALMAR": ("Palmar (Constitución)", 1982),
    "INDIA MUERTA": ("India Muerta", 1980),
    "DE PASO SEVERINO": ("Paso Severino", 1987),
    "DEL CANELÓN GRANDE": ("Canelón Grande", 1956),
}
SALTO_GRANDE = ("Salto Grande", 1979)

# Departamentos: nombre en el juego y capital (id de historia_pais.PUEBLOS). Año de creación:
# los 6 de 1816 y los siguientes según la ley de cada uno (ver historia_pais.DEPARTAMENTOS).


def grilla_lonlat(sub=1):
    """Lon y lat de las muestras (sub × sub por celda, centradas)."""
    j = (np.arange(NX * sub) + 0.5) / sub - 0.5
    i = (np.arange(NY * sub) + 0.5) / sub - 0.5
    lon = LON0 + j * DLON
    lat = LAT0 - i * DLAT
    return np.meshgrid(lon, lat)


def px(lon, lat, sub=1):
    """Coordenadas de píxel (PIL) de un punto, para rasterizar a `sub` muestras por celda."""
    return ((lon - LON0) / DLON + 0.5) * sub, ((LAT0 - lat) / DLAT + 0.5) * sub


def celda(lon, lat):
    """Coordenadas de grilla (columna, fila) en celdas, con decimales."""
    return (lon - LON0) / DLON, (LAT0 - lat) / DLAT


def km_por_celda(lat=-32.5):
    kx, ky = cm.escalas(lat)
    return DLON * kx / 1000.0, DLAT * ky / 1000.0


# --- Descargas ---------------------------------------------------------------------------------
def bajar_mtop(cache, clave):
    capa, filtro = CAPAS_MTOP[clave]
    ruta = os.path.join(cache, f"mtop_{clave}.geojson")
    if os.path.exists(ruta):
        with open(ruta) as f:
            return json.load(f)["features"]
    # Capas chicas: de una vez (algunas capas del MTOP no aceptan paginar sin orden).
    url = f"{MTOP_WFS}&typeNames={capa}"
    if filtro:
        url += "&CQL_FILTER=" + urllib.parse.quote(filtro)
    feats = json.loads(cm._bajar(url, timeout=600, intentos=6)).get("features", [])
    with open(ruta, "w") as f:
        json.dump({"type": "FeatureCollection", "features": feats}, f)
    print(f"  {clave}: {len(feats)} objetos", flush=True)
    return feats


def bajar_ide_wfs(cache, capa):
    ruta = os.path.join(cache, "ide_" + capa.replace(":", "__") + "_pais.geojson")
    if os.path.exists(ruta):
        with open(ruta) as f:
            return json.load(f)["features"]
    feats, inicio, pagina = [], 0, 200
    while True:
        url = (f"{cm.IDE_WFS}&typeNames={capa}&outputFormat=application/json&srsName=EPSG:4326"
               f"&count={pagina}&startIndex={inicio}")
        fs = json.loads(cm._bajar(url, timeout=300, intentos=6)).get("features", [])
        feats += fs
        if len(fs) < pagina:
            break
        inicio += pagina
    with open(ruta, "w") as f:
        json.dump({"type": "FeatureCollection", "features": feats}, f)
    return feats


def bajar_monte(cache):
    ruta = os.path.join(cache, f"snig_monte_nativo_pais_{MONTE_MIN_HA}ha.geojson")
    if os.path.exists(ruta):
        with open(ruta) as f:
            return json.load(f)["features"]
    base = cm.SNIG_BOSQUE.replace("where=1%3D1", f"where=HA%3E%3D{MONTE_MIN_HA}") + "&maxAllowableOffset=0.002"
    feats, inicio, pagina = [], 0, 500
    while True:
        url = f"{base}&resultOffset={inicio}&resultRecordCount={pagina}"
        fs = json.loads(cm._bajar(url, timeout=300, intentos=6)).get("features", [])
        feats += fs
        print(f"    {len(feats)} polígonos de monte", flush=True)
        if len(fs) < pagina:
            break
        inicio += len(fs)
    with open(ruta, "w") as f:
        json.dump({"type": "FeatureCollection", "features": feats}, f)
    return feats


def recortes_mdt(cache):
    """Recortes de 1° del MDT de la IDE reducidos por el servidor: (arreglo, lon0, lat0, dlon, dlat)."""
    out = []
    for la in range(-36, -29):
        for lo in range(-59, -52):
            r_s, r_n = max(la, -34.99696), min(la + 1, -30.07781)
            r_o, r_e = max(lo, -58.38600), min(lo + 1, -53.02400)
            if r_s >= r_n or r_o >= r_e:
                continue
            ruta = os.path.join(cache, f"pais_mdt_{la}_{lo}_{IDE_ESCALA}.tif")
            if not os.path.exists(ruta):
                url = cm.IDE_WCS.format(lat_s=f"{r_s:.5f}", lat_n=f"{r_n:.5f}", lon_w=f"{r_o:.5f}", lon_e=f"{r_e:.5f}")
                url += f"&scaleFactor={IDE_ESCALA}"
                try:
                    datos = cm._bajar(url, timeout=300, intentos=4)
                except RuntimeError as e:
                    print(f"    {e}", flush=True)
                    datos = b""
                if not datos.startswith((b"II", b"MM")):
                    datos = b""
                with open(ruta, "wb") as f:
                    f.write(datos)
                print(f"    recorte {la} {lo}: {len(datos) // 1024} KB", flush=True)
            if os.path.getsize(ruta) > 0:
                im = Image.open(ruta)
                t = im.tag_v2[34264]
                out.append((np.asarray(im, dtype=np.float32), t[3], t[7], t[0], -t[5]))
    return out


# --- Geometría ---------------------------------------------------------------------------------
def anillos(g):
    """[[exterior, huecos…], …] en lon/lat."""
    if not g:
        return []
    if g["type"] == "Polygon":
        return [g["coordinates"]]
    if g["type"] == "MultiPolygon":
        return g["coordinates"]
    return []


def lineas(g):
    if not g:
        return []
    if g["type"] == "LineString":
        return [g["coordinates"]]
    if g["type"] == "MultiLineString":
        return g["coordinates"]
    return []


def rasterizar(feats, sub, valor=1, img=None, mode="L"):
    img = img or Image.new(mode, (NX * sub, NY * sub), 0)
    d = ImageDraw.Draw(img)
    for f in feats:
        v = valor(f) if callable(valor) else valor
        if not v:
            continue
        for pol in anillos(f.get("geometry")):
            if not pol or len(pol[0]) < 3:
                continue
            d.polygon([px(p[0], p[1], sub) for p in pol[0]], fill=v)
            for h in pol[1:]:
                if len(h) >= 3:
                    d.polygon([px(p[0], p[1], sub) for p in h], fill=0)
    return img


def fraccion(img, sub):
    a = np.asarray(img, dtype=np.float32)
    return a.reshape(NY, sub, NX, sub).mean(axis=(1, 3))


def bonito(n):
    """Nombre con mayúsculas normales y las tildes que la capa pierde a veces."""
    n = n.strip().title()
    for a, b in ((" De ", " de "), (" Del ", " del "), (" La ", " la "), (" Las ", " las "), (" Los ", " los "),
                 ("Rio ", "Río "), ("Vizca-No", "Vizcaíno"), ("Jos+", "José")):
        n = n.replace(a, b)
    return n


def simplificar(coords, tol):
    pts = cm.rdp([(p[0], p[1]) for p in coords], tol)
    return pts


def a_celda_lista(pts, dec=2):
    out = []
    for lon, lat in pts:
        c, r = celda(lon, lat)
        out.append([round(c, dec), round(r, dec)])
    return out


def unir_lineas(segs, tol=1e-4):
    """Une tramos que se tocan en sus extremos (los ríos vienen cortados en muchos tramos)."""
    segs = [list(map(tuple, s)) for s in segs if len(s) >= 2]
    out = []
    while segs:
        cur = segs.pop()
        cambio = True
        while cambio:
            cambio = False
            for k, s in enumerate(segs):
                if math.dist(cur[-1], s[0]) < tol:
                    cur += s[1:]
                elif math.dist(cur[-1], s[-1]) < tol:
                    cur += s[::-1][1:]
                elif math.dist(cur[0], s[-1]) < tol:
                    cur = s[:-1] + cur
                elif math.dist(cur[0], s[0]) < tol:
                    cur = s[::-1][:-1] + cur
                else:
                    continue
                segs.pop(k)
                cambio = True
                break
        out.append(cur)
    return out


# --- Relieve -------------------------------------------------------------------------------------
def relieve(cache, tierra):
    """Altura media de cada celda (m) y si tuvo dato. Muestras de ~250 m (4 × 4 por celda)."""
    rec = recortes_mdt(cache)
    lon, lat = grilla_lonlat(SUB)
    h = np.zeros(lon.shape, np.float32)
    ok = np.zeros(lon.shape, bool)
    for a, lon0, lat0, dl, dt in rec:
        fi = (lat0 - lat) / dt - 0.5
        fj = (lon - lon0) / dl - 0.5
        den = (fi >= 0) & (fj >= 0) & (fi < a.shape[0] - 1) & (fj < a.shape[1] - 1) & ~ok
        if not den.any():
            continue
        i0 = np.floor(fi[den]).astype(int)
        j0 = np.floor(fj[den]).astype(int)
        ty, tx = fi[den] - i0, fj[den] - j0
        v = [a[i0, j0], a[i0, j0 + 1], a[i0 + 1, j0], a[i0 + 1, j0 + 1]]
        bueno = np.ones(i0.shape, bool)
        for k in v:
            bueno &= k > -1000
        val = (v[0] * (1 - tx) + v[1] * tx) * (1 - ty) + (v[2] * (1 - tx) + v[3] * tx) * ty
        idx = np.flatnonzero(den)
        h.flat[idx] = np.where(bueno, val, 0.0)
        ok.flat[idx] = bueno
    hs = (h * ok).reshape(NY, SUB, NX, SUB).sum(axis=(1, 3))
    ns = ok.reshape(NY, SUB, NX, SUB).sum(axis=(1, 3))
    alt = np.where(ns > 0, hs / np.maximum(ns, 1), np.nan).astype(np.float32)
    print(f"  relieve: {len(rec)} recortes, {int((ns > 0).sum())} celdas con dato, máx {np.nanmax(alt):.0f} m", flush=True)
    # Tierra sin dato (embalses, lagunas rellenadas, bordes): se rellena con los vecinos.
    falta = tierra & np.isnan(alt)
    alt = rellenar(alt, falta)
    return alt


def rellenar(alt, falta, iters=400):
    """Rellena las celdas `falta` (y las NaN) promediando vecinos con dato (difusión)."""
    a = alt.copy()
    conocido = ~np.isnan(a)
    a[~conocido] = 0.0
    hueco = ~conocido
    for _ in range(iters):
        p = np.pad(a, 1, mode="edge")
        k = np.pad(conocido.astype(np.float32), 1, mode="edge")
        suma = (p[:-2, 1:-1] * k[:-2, 1:-1] + p[2:, 1:-1] * k[2:, 1:-1] + p[1:-1, :-2] * k[1:-1, :-2] + p[1:-1, 2:] * k[1:-1, 2:])
        cuenta = k[:-2, 1:-1] + k[2:, 1:-1] + k[1:-1, :-2] + k[1:-1, 2:]
        nuevo = hueco & (cuenta > 0)
        a[nuevo] = suma[nuevo] / cuenta[nuevo]
        conocido = conocido | nuevo
        hueco = ~conocido
        if not hueco.any():
            break
    return a


def distancia_bfs(fuente):
    """Distancia (celdas, vecindad 4) a la celda `fuente` más cercana."""
    dist = np.full(fuente.shape, 10 ** 6, np.int32)
    q = deque()
    for i, j in zip(*np.nonzero(fuente)):
        dist[i, j] = 0
        q.append((i, j))
    while q:
        i, j = q.popleft()
        for di, dj in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            a, b = i + di, j + dj
            if 0 <= a < NY and 0 <= b < NX and dist[a, b] > dist[i, j] + 1:
                dist[a, b] = dist[i, j] + 1
                q.append((a, b))
    return dist


# --- Armado ----------------------------------------------------------------------------------
def construir(cache, salida, previa):
    os.makedirs(cache, exist_ok=True)
    os.makedirs(salida, exist_ok=True)
    fuentes = []
    print("Capas del MTOP…", flush=True)
    m = {k: bajar_mtop(cache, k) for k in CAPAS_MTOP}
    for k, (capa, filtro) in CAPAS_MTOP.items():
        fuentes.append({"dato": k, "servicio": "WFS " + MTOP_WFS.split("?")[0], "capa": capa, "filtro": filtro, "objetos": len(m[k])})
    print("Bañados de la IDE…", flush=True)
    banados = bajar_ide_wfs(cache, "hidrografia:banado_a")
    fuentes.append({"dato": "bañados", "servicio": "WFS " + cm.IDE_WFS.split("?")[0], "capa": "hidrografia:banado_a", "objetos": len(banados)})
    print("Monte nativo (DGF 2021, SNIG)…", flush=True)
    monte = bajar_monte(cache)
    fuentes.append({"dato": "monte nativo", "servicio": "ArcGIS REST " + cm.SNIG_BOSQUE.split("?")[0], "filtro": f"HA >= {MONTE_MIN_HA}", "objetos": len(monte)})

    # Departamentos (id 1..19 por orden alfabético).
    deptos = sorted({f["properties"]["nombre"].strip() for f in m["departamentos"]})
    did = {n: k + 1 for k, n in enumerate(deptos)}
    dep_img = rasterizar(m["departamentos"], SUB, lambda f: did[f["properties"]["nombre"].strip()])
    dep_fino = np.asarray(dep_img)
    # El departamento de la celda: el de más submuestras.
    dep = np.zeros((NY, NX), np.uint8)
    cuenta = np.zeros((NY, NX), np.int32)
    for k in range(1, len(deptos) + 1):
        c = (dep_fino == k).reshape(NY, SUB, NX, SUB).sum(axis=(1, 3))
        mejor = c > cuenta
        dep[mejor] = k
        cuenta[mejor] = c[mejor]
    pais = dep > 0

    # Agua: polígonos de hidro_pg (el contorno de los departamentos excluye el agua grande).
    def es_embalse(f):
        return (f["properties"].get("nombre") or "").strip() in EMBALSES
    # Sin nombre y de menos de 3 km²: casi siempre tajamares y represas de riego modernas (el
    # este arrocero); no se dibujan.
    def natural(f):
        pr = f["properties"]
        return not es_embalse(f) and ((pr.get("nombre") or "").strip() or (pr.get("area") or 0) >= 3e6)
    agua_nat = fraccion(rasterizar([f for f in m["agua"] if natural(f)], SUB, 255), SUB)
    emb_idx = {n: k + 1 for k, n in enumerate(EMBALSES)}
    emb_img = rasterizar([f for f in m["agua"] if es_embalse(f)], SUB, lambda f: emb_idx[f["properties"]["nombre"].strip()])
    emb_fino = np.asarray(emb_img)
    embalse = np.zeros((NY, NX), np.uint8)
    for k in emb_idx.values():
        c = (emb_fino == k).reshape(NY, SUB, NX, SUB).mean(axis=(1, 3))
        embalse[c >= 0.5] = k
    # Los pedazos sin nombre de un embalse (la capa lo corta en varios polígonos): el agua a
    # menos de 3 celdas de un embalse es parte de él.
    for _ in range(3):
        cerca = np.zeros_like(embalse)
        for di, dj in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            cerca = np.maximum(cerca, np.roll(np.roll(embalse, di, 0), dj, 1))
        nuevo = (embalse == 0) & (cerca > 0) & (agua_nat >= 128)
        embalse[nuevo] = cerca[nuevo]
        agua_nat[nuevo] = 0
    # Fuera del país y sin agua: tierra extranjera (Argentina, Brasil). Agua grande sin
    # polígono (mar abierto lejos de la costa): lo que queda fuera del país y lejos de tierra
    # extranjera lo decide el MDT (sin dato) y la posición (sur y este).
    agua = agua_nat / 255.0
    rios_l = [f for f in m["rios"] if (f["properties"].get("nombre") or "").strip() not in ("OCÉANO ATLÁNTICO", "RÍO DE LA PLATA")]

    # Salto Grande: celdas del Río Uruguay entre 30,8° y 31,3° S a más de 700 m del eje.
    eje_uy = [ln for f in m["rios"] if (f["properties"].get("nombre") or "").strip() == "RÍO URUGUAY" for ln in lineas(f["geometry"])]
    eje_img = Image.new("L", (NX * SUB, NY * SUB), 0)
    d = ImageDraw.Draw(eje_img)
    for ln in eje_uy:
        d.line([px(p[0], p[1], SUB) for p in ln], fill=1, width=1)
    eje = fraccion(eje_img, SUB) > 0
    lon_c, lat_c = grilla_lonlat(1)
    dist_eje = distancia_bfs(eje)
    sg = (agua > 0.5) & (lat_c < -30.80) & (lat_c > -31.28) & (lon_c > -57.95) & (dist_eje >= 1) & ~pais
    emb_nombres = list(EMBALSES.values()) + [SALTO_GRANDE]
    embalse[sg] = len(emb_nombres)
    agua[sg] = 0.0
    print(f"  embalses: {[(n, int((embalse == k + 1).sum())) for k, n in enumerate(emb_nombres)]}", flush=True)

    # Relieve.
    print("Relieve (MDT de la IDE)…", flush=True)
    tierra = (agua < 0.5) & ((dep > 0) | (embalse > 0))
    alt = relieve(cache, tierra)
    # Fuera del país (Argentina, Brasil): sin MDT; una llanura baja (el mapa la dibuja apagada).
    extranjera = (agua < 0.5) & ~pais & (embalse == 0)
    alt_ext = rellenar(np.where(pais, alt, np.nan).astype(np.float32), extranjera)
    alt = np.where(extranjera, np.minimum(alt_ext, 60.0) * 0.5 + 15.0, alt)
    # Agua: profundidad según la distancia a la tierra (de −1 a −20 m).
    es_agua = (agua >= 0.5) & (embalse == 0)
    dist = distancia_bfs(~es_agua)
    prof = -np.minimum(1.0 + dist * 1.5, 20.0)
    alt = np.where(es_agua, prof, np.maximum(alt, 0.5))
    # Embalses: el valle antes de la represa (la cota se interpola desde la orilla y se baja
    # suave hacia el río).
    hay_emb = embalse > 0
    alt_emb = rellenar(np.where(hay_emb | es_agua, np.nan, alt).astype(np.float32), hay_emb)
    alt = np.where(hay_emb, np.maximum(alt_emb - 4.0, 1.0), alt)
    alt = np.nan_to_num(alt, nan=10.0)

    # Ríos con nombre rasterizados (cauce en la celda).
    rio_img = Image.new("L", (NX * SUB, NY * SUB), 0)
    d = ImageDraw.Draw(rio_img)
    rios_out = []
    for f in rios_l:
        nombre = f["properties"]["nombre"].strip()
        segs = unir_lineas(lineas(f["geometry"]))
        for ln in segs:
            d.line([px(p[0], p[1], SUB) for p in ln], fill=255, width=2)
        largo = 0.0
        polis = []
        for ln in segs:
            s = simplificar(ln, 0.004)
            if len(s) >= 2:
                polis.append(a_celda_lista(s))
                kx, ky = km_por_celda(ln[0][1])
                largo += sum(math.hypot((b[0] - a[0]) / DLON * kx, (b[1] - a[1]) / DLAT * ky) for a, b in zip(ln, ln[1:]))
        rios_out.append({"nombre": bonito(nombre),
                         "navegable": f["properties"].get("regimen") == "De hecho o por su naturaleza",
                         "largo_km": round(largo), "lineas": polis})
    rio = np.clip(fraccion(rio_img, SUB) * 4.0, 0, 255).astype(np.uint8)
    rio[es_agua] = 0

    # Bañados y monte.
    ban = fraccion(rasterizar(banados, SUB, 255), SUB)
    mon = fraccion(rasterizar(monte, SUB, 255), SUB)
    ban[~pais] = 0
    mon[~pais] = 0
    # Pendiente (‰) en la celda.
    gy, gx = np.gradient(alt)
    kx, ky = km_por_celda()
    pend = np.hypot(gx / (kx * 1000.0), gy / (ky * 1000.0)) * 1000.0
    pend[es_agua] = 0

    capas = np.zeros((NY, NX, 8), np.uint8)
    capas[..., 0] = np.clip(agua * 255, 0, 255).astype(np.uint8)
    capas[..., 1] = embalse
    capas[..., 2] = dep
    capas[..., 3] = np.clip(ban, 0, 255).astype(np.uint8)
    capas[..., 4] = np.clip(mon, 0, 255).astype(np.uint8)
    capas[..., 5] = rio
    capas[..., 6] = np.clip(pend, 0, 255).astype(np.uint8)
    with open(os.path.join(salida, "capas.bin"), "wb") as f:
        f.write(capas.tobytes())
    rel = np.clip(np.round((alt + 100.0) * 10.0), 0, 65535).astype("<u2")
    with open(os.path.join(salida, "relieve.bin"), "wb") as f:
        f.write(rel.tobytes())

    # Departamentos: contorno simplificado (el anillo más grande) y centro para el rótulo.
    deptos_out = []
    for n in deptos:
        fs = [f for f in m["departamentos"] if f["properties"]["nombre"].strip() == n]
        polis = [pol[0] for f in fs for pol in anillos(f["geometry"])]
        polis.sort(key=lambda r: -len(r))
        cont = [a_celda_lista(simplificar(r, 0.006), 1) for r in polis[:1]]
        ii, jj = np.nonzero(dep == did[n])
        deptos_out.append({"id": did[n], "nombre": hp.nombre_depto(n), "celdas": int(len(ii)),
                           "centro": [round(float(jj.mean()), 1), round(float(ii.mean()), 1)], "contorno": cont})

    # Lagunas con nombre (para rótulos).
    lagunas = []
    for f in m["agua"]:
        nom = (f["properties"].get("nombre") or "").strip()
        a = f["properties"].get("area") or 0
        if not nom or nom in EMBALSES or nom in ("URUGUAY", "DE LA PLATA", "ATLANTICO") or a < 15e6:
            continue
        pols = anillos(f["geometry"])
        if not pols:
            continue
        r = max((p[0] for p in pols), key=len)
        cx = sum(p[0] for p in r) / len(r)
        cy = sum(p[1] for p in r) / len(r)
        nombre = "Laguna Merín" if "MERIN" in nom or "MER-N" in nom else bonito("Laguna " + nom)
        if nombre not in [x["nombre"] for x in lagunas]:
            lagunas.append({"nombre": nombre, "pos": [round(v, 1) for v in celda(cx, cy)], "km2": round(a / 1e6)})

    # Ferrocarril: líneas (activas e inactivas) con el año de cada tramo (historia_pais).
    vias = []
    for clave in ("via_activa", "via_inactiva"):
        for f in m[clave]:
            p = f["properties"]
            linea = (p.get("nom_linea") or p.get("nombre") or "").strip()
            for ln in lineas(f["geometry"]):
                s = simplificar(ln, 0.003)
                if len(s) < 2:
                    continue
                anio, nota = hp.anio_via(linea, s)
                vias.append({"linea": linea or "sin nombre", "activa": clave == "via_activa", "anio": anio, "nota": nota, "puntos": a_celda_lista(s)})
    estaciones = []
    for clave in ("estaciones", "estaciones_inactivas"):
        for f in m[clave]:
            g = f.get("geometry")
            if not g:
                continue
            c = g["coordinates"] if g["type"] == "Point" else g["coordinates"][0]
            nom = (f["properties"].get("nom_estaci") or f["properties"].get("nom_est") or "").strip()
            estaciones.append({"nombre": nom.title(), "activa": clave == "estaciones", "pos": [round(v, 2) for v in celda(c[0], c[1])]})
    puertos_hoy = []
    for f in m["puertos"]:
        g = f.get("geometry")
        if g:
            c = g["coordinates"]
            puertos_hoy.append({"nombre": f["properties"].get("nombre"), "tipo": f["properties"].get("tipo"), "pos": [round(v, 2) for v in celda(c[0], c[1])]})

    # Localidades de hoy (centro del polígono) para ubicar los pueblos de la historia.
    localidades = {}
    for f in m["localidades"]:
        p = f["properties"]
        nom = (p.get("nom_loc") or "").strip().upper()
        pols = anillos(f["geometry"])
        if not nom or not pols:
            continue
        r = max((pp[0] for pp in pols), key=len)
        cx = sum(q[0] for q in r) / len(r)
        cy = sum(q[1] for q in r) / len(r)
        area = p.get("area") or 0
        if nom not in localidades or localidades[nom][2] < area:
            localidades[nom] = (cx, cy, area, (p.get("nom_depto") or "").strip())

    hist = hp.armar(localidades, celda, alt, dep, deptos, es_agua, rio)
    with open(os.path.join(salida, "historia.json"), "w") as f:
        json.dump(hist, f, ensure_ascii=False, indent=1)

    kx, ky = km_por_celda()
    ciudad_n, ciudad_o = cm.LAT_N, cm.LON_W
    ciudad_s, ciudad_e = cm.a_geo(cm.MAP_SIZE, cm.MAP_SIZE)
    meta = {
        "descripcion": "Mapa del país (escala país): el Uruguay a ~1 km. Ver docs/sistemas/mapa-pais.md.",
        "grilla": {"lon_oeste": LON0, "lat_norte": LAT0, "dlon": DLON, "dlat": DLAT, "columnas": NX, "filas": NY,
                   "km_x": round(kx, 4), "km_y": round(ky, 4),
                   "muestra": "la celda (c, f) es el punto lon = lon_oeste + c·dlon, lat = lat_norte − f·dlat (la misma grilla que geologia_pais.bin)"},
        "relieve": {"archivo": "relieve.bin", "codificacion": "uint16 LE, metros = valor / 10 − 100", "max_m": round(float(alt.max()), 1)},
        "capas": {"archivo": "capas.bin", "bytes_por_celda": 8,
                  "canales": ["agua natural (0..255)", "embalse (índice + 1)", "departamento (1..19)", "bañado (0..255)", "monte nativo (0..255)", "río con nombre (0..255)", "pendiente (‰)", "libre"]},
        "ciudad": {"lat_norte": ciudad_n, "lon_oeste": ciudad_o, "lat_sur": round(ciudad_s, 5), "lon_este": round(ciudad_e, 5),
                   "celda_no": [round(v, 2) for v in celda(ciudad_o, ciudad_n)], "celda_se": [round(v, 2) for v in celda(ciudad_e, ciudad_s)],
                   "nota": "rectángulo del mapa de la ciudad (24 × 24 km, assets/mapas/montevideo)"},
        "departamentos": deptos_out,
        "embalses": [{"indice": k + 1, "nombre": n, "anio": a, "celdas": int((embalse == k + 1).sum())} for k, (n, a) in enumerate(emb_nombres)],
        "rios": rios_out,
        "lagunas": lagunas,
        "vias": vias,
        "estaciones": estaciones,
        "puertos_hoy": puertos_hoy,
        "conteos": {"tierra_pais": int(pais.sum()), "agua": int(es_agua.sum()), "embalse": int(hay_emb.sum()),
                    "banado": int((ban > 64).sum()), "monte": int((mon > 64).sum())},
        "fuentes": fuentes + [{"dato": "relieve", "servicio": "WCS " + cm.IDE_WCS.split("?")[0], "capa": "raster__mdt_pais_30m_4326", "escala": IDE_ESCALA}],
        "atribucion": ("Relieve: Modelo Digital del Terreno del Uruguay (IDE Uruguay, Presidencia de la República). "
                       "Bañados: IDE Uruguay (hidrografía 1:50.000). Departamentos, agua, ríos navegables, vías férreas, "
                       "estaciones, puertos y localidades: Ministerio de Transporte y Obras Públicas (geoservicios.mtop.gub.uy). "
                       "Monte nativo: Cartografía de Bosque Nativo 2021 (MGAP-DGF, SNIG). Licencia de Datos Abiertos del Uruguay."),
    }
    with open(os.path.join(salida, "mapa.json"), "w") as f:
        json.dump(meta, f, ensure_ascii=False, separators=(",", ":"))
    print(f"Listo: {NX} × {NY} celdas · {int(pais.sum())} km² de tierra del país · {len(rios_out)} ríos · {len(vias)} tramos de vía · "
          f"{len(hist['pueblos'])} pueblos · {len(hist['caminos'])} caminos", flush=True)
    if previa:
        vista_previa(alt, es_agua, embalse, dep, ban, mon, rio, hist, meta)


def vista_previa(alt, agua, embalse, dep, ban, mon, rio, hist, meta):
    """Imagen de control: relieve sombreado, agua, embalses, bañados, monte, ríos, pueblos."""
    gy, gx = np.gradient(alt)
    luz = np.clip(0.75 - (gx - gy) * 0.02, 0.35, 1.0)
    t = np.clip(alt / 300.0, 0, 1)
    r = (120 + 90 * t) * luz
    g = (150 + 40 * t) * luz
    b = (90 + 30 * t) * luz
    img = np.stack([r, g, b], -1)
    img[dep == 0] = img[dep == 0] * 0.55 + 60
    img[mon > 80] = img[mon > 80] * 0.6 + np.array([10, 50, 15])
    img[ban > 80] = img[ban > 80] * 0.6 + np.array([40, 60, 50])
    img[rio > 40] = [60, 110, 180]
    img[embalse > 0] = [110, 150, 200]
    img[agua] = [40, 80, 140]
    im = Image.fromarray(np.clip(img, 0, 255).astype(np.uint8)).resize((NX * 2, NY * 2), Image.NEAREST)
    d = ImageDraw.Draw(im)
    for c in hist["caminos"]:
        pts = [(p[0] * 2, p[1] * 2) for p in c["puntos"]]
        d.line(pts, fill=(150, 90, 40), width=2)
    for v in meta["vias"]:
        if v["anio"] and v["anio"] <= 1900:
            d.line([(p[0] * 2, p[1] * 2) for p in v["puntos"]], fill=(40, 40, 40), width=1)
    for p in hist["pueblos"]:
        x, y = p["pos"][0] * 2, p["pos"][1] * 2
        col = (200, 30, 30) if p["anio"] <= 1800 else (230, 200, 40)
        d.ellipse((x - 3, y - 3, x + 3, y + 3), fill=col, outline=(0, 0, 0))
        d.text((x + 4, y - 6), f"{p['nombre']} {p['anio']}", fill=(0, 0, 0))
    ruta = os.path.join(RAIZ, "docs", "capturas", "mapa-pais-datos.jpg")
    im.save(ruta, quality=88)
    print(f"Vista previa: {ruta}", flush=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cache", required=True)
    ap.add_argument("--salida", default=os.path.join(RAIZ, "assets", "mapas", "uruguay"))
    ap.add_argument("--sin-previa", action="store_true")
    a = ap.parse_args()
    construir(a.cache, a.salida, not a.sin_previa)


if __name__ == "__main__":
    main()
