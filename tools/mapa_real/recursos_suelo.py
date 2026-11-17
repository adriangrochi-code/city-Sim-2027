"""
Recursos del suelo del mapa real de Montevideo: calidad de la tierra (índice de productividad
CONEAT y aptitud para cultivos), geología (DINAMIGE) y materiales con tipo (granito, gneis,
balasto, canto rodado, arena, arcilla roja, arcilla gris, tosca, conchilla, caliza y turba).
Lo llama `construir_mapa.py` (paso «Recursos del suelo») o se corre solo:

  python3 tools/mapa_real/construir_mapa.py --cache CARPETA --solo-recursos

que usa las alturas, biomas, arroyos, costa y rocosidad ya generados en assets/mapas/montevideo/
y lo que haya en la caché (sin red, si la caché está completa). Ver
docs/sistemas/recursos-del-suelo.md y docs/sistemas/recursos-de-uruguay.md.

Salida (todas en la grilla de altura.bin / contexto.bin, filas de norte a sur, salvo materiales):
  suelos.bin / suelos_contexto.bin          uint8: índice del grupo CONEAT en suelos.json
                                            (bits 0-6, 1..127) + 128 si es una ESTIMACIÓN
                                            (zona sin dato CONEAT: la ciudad actual, rellenada
                                            con el grupo más cercano de la misma geología);
                                            0 = agua.
  productividad.bin / _contexto.bin         uint8: índice de productividad CONEAT (media
                                            nacional 100) del grupo; 0 en el agua.
  geologia.bin / geologia_contexto.bin      uint8: índice de la unidad geológica en suelos.json.
  materiales.bin / materiales_contexto.bin  11 bytes por muestra, uno por material (orden de
                                            MATERIALES), 0..255 = ley 0..1; materiales.bin en una
                                            grilla de 40 m (601 × 601), el entorno en una de
                                            200 m (371 × 371) desde −25 km.
  suelos.json                               tabla de grupos (IP, aptitud, cultivos de verano,
                                            limitantes), unidades geológicas y fuentes.
"""
import json
import os
import urllib.parse

import numpy as np
from PIL import Image, ImageDraw

import construir_mapa as cm
import recursos_uruguay as ru

# --- Fuentes -------------------------------------------------------------------------------
# Productividad por grupo CONEAT (campo PROD_SUE) y grupo (SC), MGAP-RENARE: servicio ArcGIS del
# visualizador CONEAT. La capa del SNIG (MapasBase/Renare_Coneat) solo trae SC y APTITUD.
URL_CONSULTA = "https://dgrn.mgap.gub.uy/arcgis/rest/services/CONEAT/ConsultaConeat/MapServer/0/query"
# Aptitud para cultivos de verano de secano por grupo (capa 11) y el grupo de cada polígono
# (capa 12), del mismo servidor.
URL_APT_CULTIVO = "https://dgrn.mgap.gub.uy/arcgis/rest/services/TEMATICOS/IntConeat/MapServer/11/query"
URL_GRUPOS_INT = "https://dgrn.mgap.gub.uy/arcgis/rest/services/TEMATICOS/IntConeat/MapServer/12/query"
# Carta geológica (DINAMIGE, MIEM): unidades geológicas, mapa base 2017.
URL_GEOLOGIA = ("https://geoportal.miem.gub.uy/arcgis1091/rest/services/Dinamige_GeoS/"
                "Mapa_Base_Unidades_Geologicas_2017_GeoS/MapServer/2/query")
# Catastro minero con la sustancia de cada título: ver recursos_uruguay.py (Consulta Minera
# 2026 de DINAMIGE). El servicio público CatastroMineroGeoS que se usaba antes solo daba la
# geometría.
# Actividad minera abandonada (canteras y pozos viejos), hidrografía de la IDE.
CAPAS_MINERA_IDE = ["hidrografia:actividad_minera_abandonada_a", "hidrografia_urbana:actividad_minera_abandonada_a"]

# Respaldo si el servidor no responde y no hay caché: valores leídos del servicio el 17/11/2026
# (consulta agrupada por SC y PROD_SUE en la ventana del mapa).
IP_RESPALDO = {
    "03.10": 9, "03.2": 131, "03.3": 96, "03.40": 96, "03.41": 158, "03.51": 175, "03.52": 53,
    "07.1": 4, "07.2": 0, "09.4": 57, "10.10": 44, "10.11": 210, "10.3": 140, "10.5": 236,
    "10.6a": 206, "10.6b": 131, "10.7": 131, "10.8a": 105, "10.8b": 184, "11.10": 114,
    "11.9": 201, "3.30": 18, "3.53": 70, "4.2": 61, "5.02b": 88, "6.4": 127, "9.42": 35,
}
APT_CULTIVO_RESPALDO = {
    "03.10": ("N2", "No Aptas", ""), "03.2": ("N1(i)(d)", "Marginales", "Inundable, drenaje"),
    "03.3": ("N2", "No Aptas", ""), "03.40": ("N1(i)(d)", "Marginales", "Inundable, drenaje"),
    "03.41": ("N1(i)(d)", "Marginales", "Inundable, drenaje"),
    "03.51": ("S3(i)(d)(s)", "Media", "Inundable, drenaje, sodicidad"), "03.52": ("N2", "No Aptas", ""),
    "07.1": ("N2", "No Aptas", ""), "07.2": ("N2", "No Aptas", ""),
    "09.4": ("N1(e)(m)", "Marginales", "Baja resistencia a la erosion, Baja capacidad de almacenaje de agua"),
    "10.10": ("S2(e)", "Aptas", "Baja resistencia a la erosion"), "10.11": ("S1", "Muy Aptas", ""),
    "10.3": ("S2(e)(m)", "Aptas", "Baja resistencia a la erosion, Baja capacidad de almacenaje de agua"),
    "10.5": ("S1", "Muy Aptas", ""), "10.6a": ("S2(e)", "Aptas", "Baja resistencia a la erosion"),
    "10.6b": ("S3(e)(m)", "Media", "Baja resistencia a la erosion, Baja capacidad de almacenaje de agua"),
    "10.7": ("S2(e)(m)", "Aptas", "Baja resistencia a la erosion, Baja capacidad de almacenaje de agua"),
    "10.8a": ("S3(e)(m)", "Media", "Baja resistencia a la erosion, Baja capacidad de almacenaje de agua"),
    "10.8b": ("S2(e)", "Aptas", "Baja resistencia a la erosion"),
    "11.10": ("S3(e)(m)", "Media", "Baja resistencia a la erosion, Baja capacidad de almacenaje de agua"),
    "11.9": ("S2(e)", "Aptas", "Baja resistencia a la erosion"), "3.30": ("N2", "No Aptas", ""),
    "3.53": ("N1(d)", "Marginales", "drenaje"),
    "4.2": ("N1(e)(m)", "Marginales", "Baja resistencia a la erosion, Baja capacidad de almacenaje de agua"),
    "5.02b": ("N2", "No Aptas", ""),
    "6.4": ("S3(e)(m)", "Media", "Baja resistencia a la erosion, Baja capacidad de almacenaje de agua"),
    "9.42": ("N2", "No Aptas", ""),
}

# Clase de material de cada unidad geológica (código DINAMIGE): «roca» (basamento cristalino),
# «loess» (limos y lodolitas: barro de ladrillo), «aluvion» (aluviones y sedimentos mixtos
# del Holoceno), «arena» y «arenisca» (areniscas y conglomerados).
GEO_CLASE = {
    3: "roca", 4: "roca", 5: "roca", 11: "roca", 12: "roca", 13: "roca", 41: "roca",
    65: "loess", 70: "loess", 74: "loess",
    75: "aluvion", 77: "aluvion",
    73: "arena", 76: "arena",
    60: "arenisca", 63: "arenisca", 69: "arenisca",
}
# Materiales con tipo (canales de materiales.bin, en este orden; los ids son los del catálogo
# assets/datos/recursos_uruguay.json y el enum `Material` del motor).
MATERIALES = ["granito", "gneis", "balasto", "canto_rodado", "arena", "arcilla_roja", "arcilla_gris", "tosca",
              "conchilla", "caliza", "turba"]
MAT = {m: k for k, m in enumerate(MATERIALES)}
# Lo que da cada unidad geológica de la ventana (pesos de diseño 0..1, ESTIMADOS de la
# descripción litológica de la carta 1:500.000 y de la memoria de la carta geológica de
# Montevideo; ver docs/sistemas/recursos-de-uruguay.md). No hay ninguna unidad calcárea: la
# caliza queda en 0 en todo el mapa.
GEO_MAT = {
    3: {"granito": 0.50, "gneis": 0.35, "balasto": 0.35},           # Migmatitas y granitos de Suárez
    4: {"gneis": 0.75, "balasto": 0.30},                            # Ortogneises
    5: {"gneis": 0.80, "balasto": 0.30},                            # Formación Montevideo
    11: {"granito": 0.75, "balasto": 0.45},                         # Granitos transamazónicos
    12: {"granito": 0.80, "balasto": 0.45},                         # Granitoides tardíos
    13: {"gneis": 0.60, "granito": 0.15, "balasto": 0.30},          # Zonas de cizalla (milonitas)
    41: {"granito": 0.85, "balasto": 0.80},                         # Granito de La Paz (alterado: balasto)
    60: {"arena": 0.45, "balasto": 0.35, "canto_rodado": 0.25, "arcilla_roja": 0.20},   # Migues
    63: {"arena": 0.45, "balasto": 0.45, "canto_rodado": 0.35, "arcilla_roja": 0.15},   # Mercedes
    69: {"arena": 0.55, "balasto": 0.40, "canto_rodado": 0.35, "arcilla_gris": 0.20},   # Raigón
    65: {"arcilla_roja": 0.55, "tosca": 0.60},                      # Fray Bentos («tosca parda»)
    70: {"arcilla_roja": 0.75, "tosca": 0.50},                      # Libertad (lodolitas con carbonato)
    74: {"arcilla_roja": 0.70, "tosca": 0.30, "arcilla_gris": 0.15},  # Dolores
    73: {"arena": 0.80, "arcilla_gris": 0.15},                      # Chuy
    76: {"arena": 0.85},                                            # Arenas litorales y fluviales
    75: {"arena": 0.45, "arcilla_gris": 0.45, "canto_rodado": 0.25, "conchilla": 0.35},  # Villa Soriano
    77: {"arcilla_gris": 0.55, "arena": 0.30, "canto_rodado": 0.20, "turba": 0.15},      # Actual (aluviones, turbas)
}
GEO_MAT_SIN_DATO = {"arcilla_roja": 0.3, "arena": 0.1}


def _consulta(url, params):
    """GET a un servicio ArcGIS REST con reintentos (los servidores cortan seguido)."""
    params = dict(params)
    params.setdefault("f", "json")
    return json.loads(cm._bajar(url + "?" + urllib.parse.urlencode(params), timeout=120, intentos=8))


def _ventana():
    lo, la_s, le, la_n = cm.bbox_geo()
    return f"{lo},{la_s},{le},{la_n}"


def _cache_json(cache, nombre, bajar):
    ruta = os.path.join(cache, nombre)
    if os.path.exists(ruta):
        with open(ruta) as f:
            return json.load(f)
    d = bajar()
    with open(ruta, "w") as f:
        json.dump(d, f)
    return d


def ip_por_grupo(cache):
    """{SC: IP} del campo PROD_SUE (MGAP), con respaldo si no hay red ni caché."""
    def bajar():
        d = _consulta(URL_CONSULTA, {
            "where": "1=1", "geometry": _ventana(), "geometryType": "esriGeometryEnvelope", "inSR": 4326,
            "spatialRel": "esriSpatialRelIntersects", "groupByFieldsForStatistics": "SC,PROD_SUE",
            "outStatistics": json.dumps([{"statisticType": "count", "onStatisticField": "OBJECTID", "outStatisticFieldName": "n"}]),
        })
        return {f["attributes"]["SC"]: f["attributes"]["PROD_SUE"] for f in d.get("features", [])}
    try:
        return _cache_json(cache, "coneat_ip_grupos.json", bajar), "servicio"
    except Exception as e:  # noqa: BLE001
        print(f"  sin el servicio de productividad ({e}): uso la tabla de respaldo", flush=True)
        return dict(IP_RESPALDO), "respaldo"


def aptitud_cultivo(cache):
    """{SC: (código, aptitud, limitantes)} para cultivos de verano de secano."""
    def bajar():
        q = {"geometry": _ventana(), "geometryType": "esriGeometryEnvelope", "inSR": 4326, "returnGeometry": "false"}
        grupos = _consulta(URL_GRUPOS_INT, {**q, "outFields": "OBJECTID,SC"})
        apt = _consulta(URL_APT_CULTIVO, {**q, "outFields": "*"})
        sc = {f["attributes"]["OBJECTID"]: f["attributes"]["SC"] for f in grupos.get("features", [])}
        out = {}
        for f in apt.get("features", []):
            a = f["attributes"]
            g = sc.get(a.get("gis.SIGRENARE.SUE_GruposConeat.OBJECTID"))
            if g:
                lim = ", ".join(s.strip() for s in (a.get("gis.SIGRENARE.Ap_Cverano.limitante") or "").split(",") if s.strip())
                out[g] = (a.get("gis.SIGRENARE.Ap_Cverano.Cgo"), a.get("gis.SIGRENARE.Ap_Cverano.Aptitud"), lim)
        return out
    try:
        return _cache_json(cache, "coneat_aptitud_cultivo.json", bajar), "servicio"
    except Exception as e:  # noqa: BLE001
        print(f"  sin el servicio de aptitud para cultivos ({e}): uso la tabla de respaldo", flush=True)
        return dict(APT_CULTIVO_RESPALDO), "respaldo"


def _geojson_paginado(url, params, pagina=1000):
    feats, offset = [], 0
    while True:
        d = _consulta(url, {**params, "f": "geojson", "resultOffset": offset, "resultRecordCount": pagina})
        fs = d.get("features", [])
        feats += fs
        if len(fs) < pagina and not d.get("exceededTransferLimit"):
            break
        offset += len(fs)
        if not fs:
            break
    return feats


def geologia(cache):
    """Unidades geológicas de la carta de DINAMIGE (polígonos GeoJSON) o None."""
    def bajar():
        return _geojson_paginado(URL_GEOLOGIA, {
            "where": "1=1", "geometry": _ventana(), "geometryType": "esriGeometryEnvelope", "inSR": 4326, "outSR": 4326,
            "spatialRel": "esriSpatialRelIntersects", "outFields": "Codigo,Unidad,Periodo,Epoca,Descripcion", "returnGeometry": "true",
        })
    try:
        return _cache_json(cache, f"dinamige_geologia_2017_{int(cm.OUTER)}.json", bajar)
    except Exception as e:  # noqa: BLE001
        print(f"  sin la carta geológica ({e})", flush=True)
        return None


def mineria(cache):
    """Polígonos de minería con lo que se saca: los títulos de la Consulta Minera 2026 de
    DINAMIGE (con sus materiales del catálogo; [] si el título no dice la sustancia) y la
    actividad minera abandonada de la IDE (sin sustancia). Lista de (fuente, materiales,
    geometría GeoJSON) dentro de la ventana del mapa y su entorno."""

    lo, la_s, le, la_n = cm.bbox_geo()
    out = []
    for t in ru.titulos(cache, ru.catalogo()):
        if lo <= t["lon"] <= le and la_s <= t["lat"] <= la_n:
            out.append((t["capa"], t["materiales"], t["geometria"]))
    for capa in CAPAS_MINERA_IDE:
        try:
            out += [("abandonada", [], f.get("geometry")) for f in cm.bajar_capa_wfs(cache, capa)]
        except Exception as e:  # noqa: BLE001
            print(f"  sin {capa} ({e})", flush=True)
    return out


# --- Utilidades de grilla -------------------------------------------------------------------
def _vecinos_min(a, veces):
    """Mínimo en una ventana de (2·veces+1)² (filtros de 3 × 3 repetidos)."""
    a = a.copy()
    for _ in range(veces):
        p = np.pad(a, 1, mode="edge")
        a = np.minimum.reduce([p[i:i + a.shape[0], j:j + a.shape[1]] for i in range(3) for j in range(3)])
    return a


def _cercania(m, celdas):
    """1 en la máscara, bajando linealmente a 0 a `celdas` celdas (dilataciones de 3 × 3)."""
    out = m.astype(np.float32)
    cur = m.copy()
    for k in range(1, celdas + 1):
        p = np.pad(cur, 1)
        nxt = np.logical_or.reduce([p[i:i + m.shape[0], j:j + m.shape[1]] for i in range(3) for j in range(3)])
        nuevo = nxt & ~cur
        out[nuevo] = 1.0 - k / (celdas + 1)
        cur = nxt
    return out


def _rellenar(idx, conocido, clase, pasos):
    """Rellena las celdas sin dato con el valor de la vecina conocida más cercana, primero
    dentro de la misma clase geológica y después sin restricción (propagación determinista)."""
    idx = idx.copy()
    conocido = conocido.copy()
    for restringido in (True, False):
        for _ in range(pasos):
            falta = ~conocido
            if not falta.any():
                return idx, conocido
            cambio = False
            for di, dj in ((-1, 0), (1, 0), (0, -1), (0, 1)):
                src = np.roll(np.roll(idx, di, 0), dj, 1)
                ok = np.roll(np.roll(conocido, di, 0), dj, 1) & falta
                if restringido:
                    ok &= np.roll(np.roll(clase, di, 0), dj, 1) == clase
                if ok.any():
                    idx[ok] = src[ok]
                    conocido = conocido | ok
                    falta = ~conocido
                    cambio = True
            if not cambio:
                break
    return idx, conocido


def _lineas_arroyo(arroyos, desde, paso, n, ancho_min_m):
    """Máscara de los cauces (líneas con su ancho, al menos `ancho_min_m`)."""
    img = Image.new("L", (n, n), 0)
    d = ImageDraw.Draw(img)
    for a in arroyos:
        pts = [((p[0] - desde) / paso, (p[1] - desde) / paso) for p in a]
        if len(pts) < 2:
            continue
        w = max(ancho_min_m, max(p[2] for p in a))
        d.line(pts, fill=255, width=max(1, int(round(w / paso))))
    return np.asarray(img) > 0


def _costa_rocosa(tramos, desde, paso, n, alto):
    """Roca de la orilla (0..1) en la franja de tierra a menos de ~150 m del agua, del tramo de
    costa.json más cercano (líneas gruesas sobre la grilla)."""
    img = Image.new("L", (n, n), 0)
    d = ImageDraw.Draw(img)
    for t in sorted(tramos, key=lambda t: t["rocas"]):
        v = int(round(t["rocas"] * 255))
        if v <= 0:
            continue
        a, b = t["a"], t["b"]
        d.line([((a[0] - desde) / paso, (a[1] - desde) / paso), ((b[0] - desde) / paso, (b[1] - desde) / paso)],
               fill=v, width=max(1, int(round(400.0 / paso))))
    r = np.asarray(img, dtype=np.float32) / 255.0
    agua = alto <= 0.0
    cerca = _cercania(agua, max(1, int(round(150.0 / paso))))
    return r * cerca * (alto > 0.0)


# --- Construcción ---------------------------------------------------------------------------
def construir(cache, salida, h, h_o, biomas, biomas_o, arroyos_main, arroyos_ctx, tramos, rocas, rocas_o, coneat):
    """Calcula y escribe los rasters de recursos y `suelos.json`. Devuelve el resumen para
    meta.json."""
    ips, fuente_ip = ip_por_grupo(cache)
    apts, fuente_apt = aptitud_cultivo(cache)
    geo = geologia(cache)
    minas = mineria(cache)
    print(f"  {len(ips)} grupos con IP ({fuente_ip}), {len(apts)} con aptitud de cultivo ({fuente_apt}),"
          f" {0 if geo is None else len(geo)} polígonos geológicos, {len(minas)} polígonos de minería", flush=True)

    # Tablas: grupos CONEAT (en orden de código) y unidades geológicas.
    aptitud_gral = {}
    for ft in coneat:
        pr = ft.get("properties") or {}
        if pr.get("SC"):
            aptitud_gral.setdefault(pr["SC"], pr.get("APTITUD"))
    codigos = sorted(set(aptitud_gral) | set(ips))
    grupos = []
    for sc in codigos:
        cod, apt, lim = apts.get(sc, (None, None, ""))
        grupos.append({"sc": sc, "ip": ips.get(sc), "aptitud": aptitud_gral.get(sc), "cultivo_verano": cod,
                       "cultivo_verano_aptitud": apt, "limitantes": lim, "rocosidad": round(cm.rocosidad_grupo(sc, aptitud_gral.get(sc)), 3)})
    idx_de = {g["sc"]: k + 1 for k, g in enumerate(grupos)}
    unidades = {}
    for ft in geo or []:
        pr = ft.get("properties") or {}
        c = pr.get("Codigo")
        if c is not None and c not in unidades:
            unidades[c] = {"codigo": c, "unidad": pr.get("Unidad"), "periodo": " ".join(x for x in (pr.get("Periodo"), pr.get("Epoca")) if x and x.strip()),
                           "descripcion": pr.get("Descripcion"), "clase": GEO_CLASE.get(c),
                           "materiales": GEO_MAT.get(c, GEO_MAT_SIN_DATO)}
    lista_geo = [unidades[c] for c in sorted(unidades)]
    gidx = {u["codigo"]: k + 1 for k, u in enumerate(lista_geo)}

    resultados = {}
    grillas = (("main", 0.0, cm.STEP, int(cm.MAP_SIZE / cm.STEP) + 1, h, biomas, arroyos_main, rocas),
               ("ctx", -cm.OUTER, cm.OUTER_STEP, int((cm.MAP_SIZE + 2 * cm.OUTER) / cm.OUTER_STEP) + 1, h_o, biomas_o, arroyos_main + arroyos_ctx, rocas_o))
    for nombre, desde, paso, n, alto, bio, arr, roc in grillas:
        tierra = alto > 0.0
        # Grupos CONEAT (los polígonos se pintan en orden: no se superponen).
        img = Image.new("L", (n, n), 0)
        for ft in coneat:
            sc = (ft.get("properties") or {}).get("SC")
            if sc in idx_de:
                cm.rasterizar(cm._poligonos(ft.get("geometry")), desde, paso, n, valor=idx_de[sc], img=img, borde=False)
        grupo = np.asarray(img, dtype=np.uint8).copy()
        # Geología.
        img = Image.new("L", (n, n), 0)
        for ft in geo or []:
            c = (ft.get("properties") or {}).get("Codigo")
            if c in gidx:
                cm.rasterizar(cm._poligonos(ft.get("geometry")), desde, paso, n, valor=gidx[c], img=img, borde=False)
        gunid = np.asarray(img, dtype=np.uint8).copy()
        # La geología sin dato (el borde de la carta, el agua) toma la de la vecina.
        gunid, _ = _rellenar(gunid, gunid > 0, np.zeros((n, n), dtype=np.uint8), 60)
        codigo_geo = np.zeros((n, n), dtype=np.int32)
        for u in lista_geo:
            codigo_geo[gunid == gidx[u["codigo"]]] = u["codigo"]
        clase_nombres = ["roca", "loess", "aluvion", "arena", "arenisca"]
        clase = np.zeros((n, n), dtype=np.uint8)
        for k, cn in enumerate(clase_nombres):
            for c, cc in GEO_CLASE.items():
                if cc == cn:
                    clase[codigo_geo == c] = k + 1
        # Zonas sin CONEAT (la ciudad actual): el grupo de la vecina más cercana con la misma
        # clase geológica. Se marca como estimación.
        con_dato = grupo > 0
        relleno, alcanzado = _rellenar(grupo, con_dato, clase, int(8000 / paso))
        estimado = tierra & ~con_dato & alcanzado
        grupo_final = np.where(tierra, relleno, 0).astype(np.uint8)
        suelos = grupo_final | (estimado.astype(np.uint8) * 128)
        ip_tabla = np.zeros(256, dtype=np.float32)
        for g in grupos:
            ip_tabla[idx_de[g["sc"]]] = g["ip"] if g["ip"] is not None else 100.0
        ip = np.where(tierra & (grupo_final > 0), ip_tabla[grupo_final], 0.0)
        # Materiales con tipo: la unidad geológica de cada celda, CONEAT, relieve, costa,
        # arroyos y la minería conocida (ver docs/sistemas/recursos-de-uruguay.md).
        mat = np.zeros((n, n, len(MATERIALES)), dtype=np.float32)
        for c in set(codigo_geo.ravel().tolist()):
            m = codigo_geo == c
            for nom, w in GEO_MAT.get(c, GEO_MAT_SIN_DATO).items():
                mat[..., MAT[nom]][m] = w
        sc_de = np.array([""] + [g["sc"] for g in grupos], dtype=object)
        zona = np.array([s.split(".")[0].lstrip("0") for s in sc_de], dtype=object)
        zona_celda = zona[grupo_final]
        G, N = MAT["granito"], MAT["gneis"]
        roca_base = np.maximum(mat[..., G], mat[..., N])
        sobre_roca = roca_base > 0.3
        # Qué roca es (granito o gneis) donde hay piedra: la proporción de la unidad; sobre
        # sedimentos (la costa rocosa sobre la carta gruesa), gneis, la roca de la costa.
        frac_granito = np.where(roca_base > 0, mat[..., G] / np.maximum(mat[..., G] + mat[..., N], 1e-6), 0.0)
        # Piedra: el basamento; los afloramientos (rocosidad CONEAT) y las colinas de suelo
        # superficial (grupos 1, 2, 4, 5) la dejan a mano. Sobre sedimentos, nada.
        extra = 0.6 * np.clip(roc / 0.3, 0, 1) * (roca_base > 0)
        extra += np.where(np.isin(zona_celda, ["1", "2", "4", "5"]) & (roca_base > 0), 0.15, 0.0)
        costa_roca = _costa_rocosa(tramos, desde, paso, n, alto)
        piedra = np.maximum(roca_base + extra, costa_roca)
        frac_granito = np.where((roca_base <= 0) & (costa_roca > 0), 0.0, frac_granito)
        granito = piedra * frac_granito
        gneis = piedra * (1.0 - frac_granito)
        # Balasto: la roca alterada bajo el suelo (no donde aflora sana) y las areniscas
        # conglomerádicas.
        balasto = mat[..., MAT["balasto"]] * (1.0 - 0.5 * np.clip(roc / 0.3, 0, 1))
        # Arcillas: la roja (lodolitas) en las lomadas; la gris en los bajos junto a los arroyos
        # (aluvión fino), en los suelos de planicie (grupos 03, hidromórficos) y en los bañados.
        cauce = _lineas_arroyo(arr, desde, paso, n, 3.0)
        cerca_arroyo = _cercania(cauce, max(1, int(round(300.0 / paso))))
        bajo = (alto - _vecinos_min(alto, max(1, int(round(300.0 / paso))))) < 4.0
        vega = cerca_arroyo * bajo * ~sobre_roca
        banado = bio == 2
        roja = mat[..., MAT["arcilla_roja"]] * (1.0 - 0.4 * vega)
        roja = np.where(zona_celda == "7", roja * 0.3, roja)
        gris = mat[..., MAT["arcilla_gris"]] + 0.35 * vega + np.where(zona_celda == "3", 0.2, 0.0) + 0.3 * banado
        gris = np.where(zona_celda == "7", gris * 0.3, gris)
        # Tosca: en los limos de las lomadas, no en los bajos húmedos.
        tosca = mat[..., MAT["tosca"]] * (1.0 - 0.8 * vega) * ~banado
        # Arena: playas y bancos (IDE), médanos detrás de las playas, grupos 07 (arenas), la
        # geología arenosa y la orilla de los arroyos anchos.
        playa = np.isin(bio, [1, 3])
        arena = np.maximum(mat[..., MAT["arena"]], 0.95 * playa)
        arena = np.maximum(arena, 0.7 * _cercania(playa, max(1, int(round(400.0 / paso)))) * tierra)
        arena = np.maximum(arena, np.where(zona_celda == "7", 0.9, 0.0))
        cauce_ancho = _lineas_arroyo([a for a in arr if max(p[2] for p in a) >= 8.0], desde, paso, n, 8.0)
        arena = np.maximum(arena, 0.4 * _cercania(cauce_ancho, max(1, int(round(40.0 / paso)))))
        # Canto rodado: el lecho de los arroyos que bajan del basamento y las unidades con cantos.
        roca_cerca = _cercania(sobre_roca, max(1, int(round(1500.0 / paso))))
        lecho = _cercania(_lineas_arroyo(arr, desde, paso, n, 4.0), max(1, int(round(40.0 / paso))))
        canto = np.maximum(mat[..., MAT["canto_rodado"]], 0.75 * lecho * roca_cerca)
        # Conchilla: la Formación Villa Soriano (estimado); turba: bañados y aluviones bajos.
        conchilla = mat[..., MAT["conchilla"]]
        turba = np.maximum(0.55 * banado, mat[..., MAT["turba"]] * vega)
        caliza = mat[..., MAT["caliza"]]
        canales = {"granito": granito, "gneis": gneis, "balasto": balasto, "canto_rodado": canto, "arena": arena,
                   "arcilla_roja": roja, "arcilla_gris": gris, "tosca": tosca, "conchilla": conchilla, "caliza": caliza, "turba": turba}
        out = np.stack([canales[m] for m in MATERIALES], axis=-1)
        # Minería conocida: cada título marca lo que dice que saca (a 200 m del polígono); la
        # piedra partida («roca») es granito o gneis según la unidad; los títulos sin sustancia
        # y los pozos abandonados, lo que da la geología (piedra sobre roca; arcilla o tosca
        # sobre limos; arena sobre arenas).
        cerca = max(1, int(round(200.0 / paso)))
        mina = np.zeros((n, n), dtype=bool)
        por_material = {}
        for _fuente, mats, g in minas:
            for m in (mats or ["?"]):
                por_material.setdefault(m, []).extend(cm._poligonos(g))
        for m, polis in por_material.items():
            mk = np.asarray(cm.rasterizar(polis, desde, paso, n, borde=True)) > 0
            if not mk.any():
                continue
            mina |= mk
            k = _cercania(mk, cerca)
            if m in ("roca", "?"):
                if m == "?":
                    # Sin sustancia: según la geología del lugar.
                    sed_arena = ~sobre_roca & (mat[..., MAT["arena"]] > mat[..., MAT["arcilla_roja"]])
                    sed_limo = ~sobre_roca & ~sed_arena
                    out[..., MAT["arena"]] = np.maximum(out[..., MAT["arena"]], 0.85 * k * sed_arena)
                    out[..., MAT["arcilla_roja"]] = np.maximum(out[..., MAT["arcilla_roja"]], 0.75 * k * sed_limo)
                    out[..., MAT["tosca"]] = np.maximum(out[..., MAT["tosca"]], 0.75 * k * sed_limo)
                es_g = frac_granito >= 0.5
                out[..., G] = np.maximum(out[..., G], 0.95 * k * sobre_roca * es_g)
                out[..., N] = np.maximum(out[..., N], 0.95 * k * sobre_roca * ~es_g)
            elif m in MAT:
                out[..., MAT[m]] = np.maximum(out[..., MAT[m]], 0.9 * k)
        materiales = np.clip(out, 0.0, 1.0) * tierra[..., None]
        resultados[nombre] = {"suelos": suelos, "ip": ip, "geologia": np.where(tierra | (gunid > 0), gunid, 0).astype(np.uint8),
                              "materiales": materiales, "estimado": estimado, "tierra": tierra, "mina": mina}

    # Materiales a la mitad de resolución (bastan para elegir canteras): el mapa a 40 m, el
    # entorno a 200 m.
    mm = resultados["main"]["materiales"][::2, ::2, :]
    mc = resultados["ctx"]["materiales"][::2, ::2, :]
    def u8(v):
        return np.clip(np.round(v * 255.0), 0, 255).astype(np.uint8)
    for nombre, suf in (("main", ""), ("ctx", "_contexto")):
        r = resultados[nombre]
        with open(os.path.join(salida, f"suelos{suf}.bin"), "wb") as f:
            f.write(r["suelos"].astype(np.uint8).tobytes())
        with open(os.path.join(salida, f"productividad{suf}.bin"), "wb") as f:
            f.write(np.clip(np.round(r["ip"]), 0, 255).astype(np.uint8).tobytes())
        with open(os.path.join(salida, f"geologia{suf}.bin"), "wb") as f:
            f.write(r["geologia"].tobytes())
        with open(os.path.join(salida, f"materiales{suf}.bin"), "wb") as f:
            f.write(u8(mm if nombre == "main" else mc).tobytes())
        # Los cuatro canales genéricos de antes quedan reemplazados por los materiales.
        viejo = os.path.join(salida, f"minerales{suf}.bin")
        if os.path.exists(viejo):
            os.remove(viejo)
    main = resultados["main"]
    tierra = main["tierra"]
    tierra_m = tierra[::2, ::2]
    resumen = {
        "archivos": {
            "suelos": "suelos.bin / suelos_contexto.bin: uint8, índice del grupo en suelos.json (bits 0-6) + 128 si es estimación; 0 = agua",
            "productividad": "productividad.bin / productividad_contexto.bin: uint8, índice de productividad CONEAT (media nacional 100); 0 en el agua",
            "geologia": "geologia.bin / geologia_contexto.bin: uint8, índice de la unidad en suelos.json (geologia); 0 = sin dato",
            "materiales": f"materiales.bin (grilla de 40 m, {mm.shape[0]} × {mm.shape[0]}) / materiales_contexto.bin (grilla de 200 m desde {-cm.OUTER:.0f} m, {mc.shape[0]} × {mc.shape[0]}): {len(MATERIALES)} bytes por muestra, en el orden de «materiales»; 0..255 = ley 0..1",
        },
        "materiales": MATERIALES,
        "materiales_paso_m": cm.STEP * 2, "materiales_muestras": int(mm.shape[0]),
        "materiales_contexto_paso_m": cm.OUTER_STEP * 2, "materiales_contexto_muestras": int(mc.shape[0]),
        "fuente_ip": fuente_ip, "fuente_aptitud_cultivo": fuente_apt,
        "fraccion_estimada": round(float(main["estimado"].sum()) / max(1, int(tierra.sum())), 3),
        "ip_medio_tierra": round(float(main["ip"][tierra].mean()), 1),
        "celdas_con_mineria": int(main["mina"].sum()),
        "titulos_mineros_en_la_ventana": len(minas),
        "media_materiales": {k: round(float(mm[..., i][tierra_m].mean()), 3) for i, k in enumerate(MATERIALES)},
        "fraccion_con_material": {k: round(float((mm[..., i][tierra_m] >= 0.15).mean()), 3) for i, k in enumerate(MATERIALES)},
    }
    cm.guardar_json(os.path.join(salida, "suelos.json"), {
        "descripcion": "Grupos CONEAT (MGAP-RENARE) con su índice de productividad (PROD_SUE) y aptitud para cultivos de verano de secano, y unidades geológicas (DINAMIGE, mapa base 2017) presentes en el mapa. Índices de suelos.bin y geologia.bin. Ver docs/sistemas/recursos-del-suelo.md.",
        "grupos": grupos,
        "geologia": lista_geo,
        "fuentes": {
            "productividad": URL_CONSULTA + " (groupByFieldsForStatistics=SC,PROD_SUE)",
            "aptitud_cultivo": URL_APT_CULTIVO + " + " + URL_GRUPOS_INT,
            "geologia": URL_GEOLOGIA,
            "mineria": [ru.URL_CONSULTA_MINERA.format(capa="{2,3,4,5}")] + ["WFS " + cm.IDE_WFS.split("?")[0] + " " + c for c in CAPAS_MINERA_IDE],
            "materiales": "pesos por unidad geológica (GEO_MAT en tools/mapa_real/recursos_suelo.py) y catálogo assets/datos/recursos_uruguay.json",
        },
        "resumen": resumen,
    }, compacto=False)
    vista_previa(resultados["main"], grupos, lista_geo, h)
    return resumen


def vista_previa(r, grupos, lista_geo, h):
    """Imágenes de control en docs/capturas (no las usa el juego)."""
    base = os.path.join(os.path.dirname(__file__), "..", "..", "docs", "capturas")
    os.makedirs(base, exist_ok=True)
    tierra = r["tierra"]
    gris = np.clip(h / 80.0, 0, 1)[..., None]
    fondo = np.concatenate([0.55 + 0.3 * gris, 0.6 + 0.3 * gris, 0.62 + 0.3 * gris], axis=-1)
    agua = np.array([0.45, 0.58, 0.75])

    def guardar(nombre, rgb):
        rgb = np.where(tierra[..., None], rgb, agua)
        Image.fromarray((np.clip(rgb, 0, 1) * 255).astype(np.uint8)).save(os.path.join(base, nombre), quality=85)

    # Productividad: de pardo (0) a verde oscuro (≥ 200); lo estimado, más pálido.
    t = np.clip(r["ip"] / 200.0, 0, 1)[..., None]
    rgb = (1 - t) * np.array([0.78, 0.62, 0.40]) + t * np.array([0.15, 0.55, 0.15])
    rgb = np.where(r["estimado"][..., None], rgb * 0.55 + 0.45, rgb)
    guardar("recursos-productividad.jpg", rgb)
    # Materiales: el color del que tiene más ley en cada celda (los mismos de la capa del juego).
    m = r["materiales"]
    rgb = fondo * 0.7
    k = np.argmax(m, axis=-1)
    a = np.clip(np.max(m, axis=-1), 0, 1)[..., None]
    col = np.array([COLOR_MATERIAL[x] for x in MATERIALES])[k]
    rgb = rgb * (1 - a * 0.85) + col * a * 0.85
    guardar("recursos-minerales.jpg", rgb)


# Colores de cada material (los mismos que `Material::color` en el motor).
COLOR_MATERIAL = {
    "granito": (0.85, 0.45, 0.55), "gneis": (0.40, 0.42, 0.50), "balasto": (0.62, 0.45, 0.30),
    "canto_rodado": (0.35, 0.55, 0.85), "arena": (0.98, 0.86, 0.40), "arcilla_roja": (0.80, 0.30, 0.12),
    "arcilla_gris": (0.50, 0.58, 0.62), "tosca": (0.92, 0.62, 0.18), "conchilla": (0.97, 0.97, 0.92),
    "caliza": (0.92, 0.92, 0.80), "turba": (0.22, 0.16, 0.10),
}
