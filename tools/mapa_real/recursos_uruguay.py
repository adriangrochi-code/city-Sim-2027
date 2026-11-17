"""
Recursos naturales del Uruguay a escala país: títulos mineros con su sustancia (catastro minero
de DINAMIGE, «Consulta Minera 2026»), unidades geológicas de todo el país y el catálogo
`assets/datos/recursos_uruguay.json` (qué hay, dónde, para qué, desde cuándo y con qué fuente).

Lo llama `recursos_suelo.py` (los títulos de la ventana del mapa de Montevideo, ya con su
material) y `construir_mapa.py --solo-recursos`; también se corre solo:

  python3 tools/mapa_real/recursos_uruguay.py --cache CARPETA

Sin red usa lo que haya en la caché; sin caché, no escribe nada (avisa).

Salida en assets/datos/uruguay/:
  pais.json            grilla del país (1 km aprox.), tabla de unidades geológicas con el
                       potencial de cada material (ESTIMADO, de los pesos del catálogo), conteos
                       de títulos por material y departamento (MEDIDO) y fuentes.
  geologia_pais.bin    uint8, filas de norte a sur: código de la unidad geológica + 1 (0 = fuera
                       del país o agua). La grilla está en pais.json.
  titulos_mineros.json un punto por título (centro del polígono, lon/lat), con sus materiales
                       del catálogo, sustancias tal como las escribe el catastro, tipo de título,
                       estado, departamento(s), año de otorgamiento y hectáreas. Sin nombres de
                       empresas ni datos de contacto.
Y la imagen de control docs/capturas/recursos-uruguay.jpg.

Ver docs/sistemas/recursos-de-uruguay.md.
"""
import argparse
import datetime
import json
import os
import re
import sys
import urllib.parse

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import construir_mapa as cm  # noqa: E402

RAIZ = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
CATALOGO = os.path.join(RAIZ, "assets", "datos", "recursos_uruguay.json")
SALIDA = os.path.join(RAIZ, "assets", "datos", "uruguay")

# Títulos mineros con sustancia (campo MINERALES): reservas mineras (2), pedimentos en trámite
# (3), pedimentos otorgados (4) y canteras de obra pública (5). El servicio público
# Dinamige_GeoS/CatastroMineroGeoS tiene las mismas capas pero sin la sustancia.
URL_CONSULTA_MINERA = "https://geoportal.miem.gub.uy/arcgis1091/rest/services/Dinamige_ConsMin/Consulta_Minera_2026/MapServer/{capa}/query"
CAPAS_CONSULTA = {2: "reserva", 3: "en_tramite", 4: "otorgado", 5: "cantera_obra_publica"}
# Solo lo que el juego necesita: nada de empresas, documentos ni teléfonos.
CAMPOS_CONSULTA = "OBJECTID,EXPEDIENTE,TEMAPRIMARIO,ESTADO_EXPEDIENTE,MINERALES,FECHA_OTORGAMIENTO,FECHA_VENCIMIENTO,PADRONES,COORDENADA_X,COORDENADA_Y,AREA"
# Unidades geológicas de todo el país (la misma capa que usa el mapa de Montevideo).
URL_GEOLOGIA = ("https://geoportal.miem.gub.uy/arcgis1091/rest/services/Dinamige_GeoS/"
                "Mapa_Base_Unidades_Geologicas_2017_GeoS/MapServer/2/query")
# Regiones mineras (el Distrito Gemológico Los Catalanes).
URL_REGIONES = "https://geoportal.miem.gub.uy/arcgis1091/rest/services/Dinamige_GeoS/GeologiaGeoS/MapServer/0/query"

# Grilla del país: ~1 km (0,009° de latitud; 0,0107° de longitud a 32,5° S).
PAIS_LON = (-58.50, -53.05)
PAIS_LAT = (-35.05, -30.05)
PAIS_DLON = 0.0107
PAIS_DLAT = 0.009

TIPO_TITULO = [
    ("CONCESION PARA EXPLOTAR", "concesion"),
    ("PERMISO DE EXPLORACION", "exploracion"),
    ("PERMISO DE PROSPECCION", "prospeccion"),
    ("SOLICITUD ART", "solicitud_art116"),
    ("CANTERAS DE OBRAS PUBLICAS", "cantera_obra_publica"),
    ("CANTERAS OBRAS PUBLICAS", "cantera_obra_publica"),
    ("CLASE II", "reserva"),
    ("ESCOMBRERA", "escombrera"),
]


def catalogo():
    with open(CATALOGO) as f:
        return json.load(f)


def _consulta(url, params):
    params = dict(params)
    params.setdefault("f", "json")
    return json.loads(cm._bajar(url + "?" + urllib.parse.urlencode(params), timeout=240, intentos=10))


def _paginado(url, params, pagina):
    feats, off = [], 0
    while True:
        d = _consulta(url, {**params, "f": "geojson", "resultOffset": off, "resultRecordCount": pagina, "orderByFields": "OBJECTID"})
        fs = d.get("features", [])
        feats += fs
        off += len(fs)
        if not fs or (len(fs) < pagina and not d.get("exceededTransferLimit")):
            return feats


def _cache(cache, nombre, bajar):
    ruta = os.path.join(cache, nombre)
    if os.path.exists(ruta):
        with open(ruta) as f:
            return json.load(f)
    d = bajar()
    with open(ruta, "w") as f:
        json.dump(d, f)
    return d


def titulos_crudos(cache):
    """{capa: [features GeoJSON]} de la Consulta Minera 2026, todo el país (caché)."""
    out = {}
    for capa in CAPAS_CONSULTA:
        try:
            out[capa] = _cache(cache, f"dinamige_consulta_minera_2026_{capa}.json", lambda c=capa: _paginado(
                URL_CONSULTA_MINERA.format(capa=c), {"where": "1=1", "outFields": CAMPOS_CONSULTA, "returnGeometry": "true",
                                                     "outSR": 4326, "geometryPrecision": 5}, 100))
        except Exception as e:  # noqa: BLE001
            print(f"  sin la Consulta Minera, capa {capa} ({e})", flush=True)
            out[capa] = []
    return out


def geologia_pais(cache):
    try:
        return _cache(cache, "dinamige_geologia_2017_pais.json", lambda: _paginado(URL_GEOLOGIA, {
            "where": "1=1", "outFields": "Codigo,Unidad,Periodo,Epoca,Descripcion", "returnGeometry": "true", "outSR": 4326,
            "maxAllowableOffset": 0.003, "geometryPrecision": 4}, 500))
    except Exception as e:  # noqa: BLE001
        print(f"  sin la carta geológica del país ({e})", flush=True)
        return None


def regiones_mineras(cache):
    try:
        return _cache(cache, "dinamige_regiones_mineras.json", lambda: _paginado(URL_REGIONES, {
            "where": "1=1", "outFields": "Categoria,Nombre,Materiales", "returnGeometry": "true", "outSR": 4326,
            "geometryPrecision": 4}, 100))
    except Exception as e:  # noqa: BLE001
        print(f"  sin las regiones mineras ({e})", flush=True)
        return []


# --- Clasificación ----------------------------------------------------------------------------
def _norm(s):
    return re.sub(r"\s+", " ", (s or "").strip().upper())


def tabla_sustancias(cat):
    """{sustancia normalizada: id del catálogo}."""
    t = {}
    for r in cat["recursos"]:
        for s in r.get("sustancias_catastro", []):
            t[_norm(s)] = r["id"]
    return t


# Sustancias del catastro que no son un recurso propio sino un producto de otro: la piedra
# partida y su polvo salen del granito o del gneis según la geología del lugar ("roca").
SUSTANCIA_ROCA = {"PIEDRA PARTIDA", "ARENA DE CANTERA (POLVO DE PIEDRA PARTIDA)", "POLVO DE PIEDRA PARTIDA", "PIEDRA BRUTA",
                  "PIEDRA EN BRUTO", "ROCA PARTIDA", "PIEDRA DESCARTE"}
# Nombres de departamento como los escribe el catastro (sin tildes) y como van en el juego.
DEPARTAMENTOS = {"San Jose": "San José", "Paysandu": "Paysandú", "Rio Negro": "Río Negro", "Tacuarembo": "Tacuarembó",
                 "Treinta Y Tres": "Treinta y Tres"}


def materiales_de(minerales, tabla):
    """Ids del catálogo de un campo MINERALES ("TOSCA, BALASTO") y las sustancias crudas."""
    ids, crudas = [], []
    for s in (minerales or "").split(","):
        s = _norm(s)
        if not s:
            continue
        crudas.append(s)
        i = "roca" if s in SUSTANCIA_ROCA else tabla.get(s)
        if i and i not in ids:
            ids.append(i)
    # El cuarzo que acompaña a las amatistas y ágatas (Artigas) es de geodas, no industrial.
    if "feldespato_cuarzo" in ids and ("amatista" in ids or "agata" in ids) and "FELDESPATO" not in crudas:
        ids.remove("feldespato_cuarzo")
    return ids, crudas


def tipo_titulo(tema):
    t = _norm(tema)
    for clave, tipo in TIPO_TITULO:
        if clave in t:
            return tipo
    return "otro"


def departamentos(padrones):
    return sorted({DEPARTAMENTOS.get(d.strip().title(), d.strip().title()) for d in re.findall(r"Departamento de ([A-ZÁÉÍÓÚÑ ]+?):", padrones or "")})


def _anillos(g):
    if not g:
        return []
    if g["type"] == "Polygon":
        return [g["coordinates"]]
    if g["type"] == "MultiPolygon":
        return g["coordinates"]
    return []


def _centro(g):
    pts = [p for pol in _anillos(g) for p in (pol[0] if pol else [])]
    if not pts:
        return None
    return sum(p[0] for p in pts) / len(pts), sum(p[1] for p in pts) / len(pts)


def titulos(cache, cat):
    """Lista de títulos con su material del catálogo (todas las capas)."""
    tabla = tabla_sustancias(cat)
    out = []
    for capa, fs in titulos_crudos(cache).items():
        for f in fs:
            pr = f.get("properties") or {}
            c = _centro(f.get("geometry"))
            if c is None:
                continue
            ids, crudas = materiales_de(pr.get("MINERALES"), tabla)
            fo = pr.get("FECHA_OTORGAMIENTO")
            anio = datetime.datetime.fromtimestamp(fo / 1000, datetime.timezone.utc).year if isinstance(fo, (int, float)) and fo > 0 else None
            out.append({
                "capa": CAPAS_CONSULTA[capa], "lon": round(c[0], 5), "lat": round(c[1], 5),
                "materiales": ids, "sustancias": crudas, "tipo": tipo_titulo(pr.get("TEMAPRIMARIO")) if capa != 5 else "cantera_obra_publica",
                "estado": (pr.get("ESTADO_EXPEDIENTE") or "").strip() or None, "departamentos": departamentos(pr.get("PADRONES")),
                "anio": anio, "ha": round((pr.get("AREA") or 0.0) / 10000.0, 1), "geometria": f.get("geometry"),
            })
    return out


# --- País -------------------------------------------------------------------------------------
def _grilla():
    nx = int(round((PAIS_LON[1] - PAIS_LON[0]) / PAIS_DLON)) + 1
    ny = int(round((PAIS_LAT[1] - PAIS_LAT[0]) / PAIS_DLAT)) + 1
    return nx, ny


def _px(lon, lat):
    return (lon - PAIS_LON[0]) / PAIS_DLON, (PAIS_LAT[1] - lat) / PAIS_DLAT


def potencial_unidades(cat):
    """{código de unidad: {material: peso}} de los pesos del catálogo (ESTIMADO)."""
    pot = {}
    for r in cat["recursos"]:
        for cod, w in (r.get("unidades_dinamige") or {}).items():
            pot.setdefault(int(cod), {})[r["id"]] = w
    return pot


def construir(cache, salida=SALIDA, previa=True):
    """Arma el dataset del país. Devuelve el resumen (o None si no hay datos)."""
    cat = catalogo()
    geo = geologia_pais(cache)
    tits = titulos(cache, cat)
    regiones = regiones_mineras(cache)
    if not geo and not tits:
        print("  sin datos del país (ni red ni caché): no escribo nada", flush=True)
        return None
    os.makedirs(salida, exist_ok=True)
    nx, ny = _grilla()
    unidades = {}
    img = Image.new("L", (nx, ny), 0)
    d = ImageDraw.Draw(img)
    for ft in geo or []:
        pr = ft.get("properties") or {}
        c = pr.get("Codigo")
        if c is None:
            continue
        unidades.setdefault(int(c), {"codigo": int(c), "unidad": pr.get("Unidad"), "periodo": " ".join(x for x in (pr.get("Periodo"), pr.get("Epoca")) if x and x.strip()), "descripcion": pr.get("Descripcion")})
        for pol in _anillos(ft.get("geometry")):
            if not pol or len(pol[0]) < 3:
                continue
            d.polygon([_px(*p[:2]) for p in pol[0]], fill=int(c) + 1)
            for hueco in pol[1:]:
                if len(hueco) >= 3:
                    d.polygon([_px(*p[:2]) for p in hueco], fill=0)
    geo_bin = np.asarray(img, dtype=np.uint8)
    with open(os.path.join(salida, "geologia_pais.bin"), "wb") as f:
        f.write(geo_bin.tobytes())
    pot = potencial_unidades(cat)
    for c, u in unidades.items():
        u["materiales"] = pot.get(c, {})
        u["celdas_km2"] = int((geo_bin == c + 1).sum())
    # Conteos de títulos por material y departamento (MEDIDO).
    conteo = {}
    for t in tits:
        for m in t["materiales"] or ["sin_dato"]:
            cm_ = conteo.setdefault(m, {"titulos": 0, "concesiones": 0, "departamentos": {}})
            cm_["titulos"] += 1
            if t["tipo"] in ("concesion", "cantera_obra_publica"):
                cm_["concesiones"] += 1
            for dep in t["departamentos"]:
                cm_["departamentos"][dep] = cm_["departamentos"].get(dep, 0) + 1
    for v in conteo.values():
        v["departamentos"] = dict(sorted(v["departamentos"].items(), key=lambda x: (-x[1], x[0])))
    sin_tipo = sorted({s for t in tits for s in t["sustancias"] if s not in tabla_sustancias(cat) and s not in SUSTANCIA_ROCA})
    pais = {
        "descripcion": "Recursos naturales del Uruguay a escala país (1 km aprox.) para el mapa futuro del país. Ver docs/sistemas/recursos-de-uruguay.md.",
        "grilla": {"lon_oeste": PAIS_LON[0], "lat_norte": PAIS_LAT[1], "dlon": PAIS_DLON, "dlat": PAIS_DLAT, "columnas": nx, "filas": ny,
                   "archivo": "geologia_pais.bin", "codificacion": "uint8 por celda, filas de norte a sur: código DINAMIGE de la unidad + 1; 0 = fuera del país o agua"},
        "unidades": [unidades[c] for c in sorted(unidades)],
        "potencial": "ESTIMADO: peso 0..1 de cada material en cada unidad geológica, de los pesos `unidades_dinamige` del catálogo (descripción litológica de la carta y la carta de materias primas de 1987)",
        "titulos": {"archivo": "titulos_mineros.json", "total": len(tits), "por_material": dict(sorted(conteo.items())),
                    "sustancias_sin_clasificar": sin_tipo, "estado": "MEDIDO (catastro minero, consulta del 17/11/2026)"},
        "regiones_mineras": [{"nombre": (r.get("properties") or {}).get("Nombre"), "materiales": (r.get("properties") or {}).get("Materiales"), "centro": _centro(r.get("geometry"))} for r in regiones],
        "fuentes": {"titulos": URL_CONSULTA_MINERA.format(capa="{2,3,4,5}"), "geologia": URL_GEOLOGIA, "regiones": URL_REGIONES, "catalogo": "assets/datos/recursos_uruguay.json"},
    }
    cm.guardar_json(os.path.join(salida, "pais.json"), pais, compacto=False)
    cm.guardar_json(os.path.join(salida, "titulos_mineros.json"), {
        "descripcion": "Títulos mineros del Uruguay (DINAMIGE, Consulta Minera 2026): centro del polígono, materiales del catálogo, sustancias tal como las escribe el catastro, tipo, estado, departamentos, año de otorgamiento y hectáreas. «roca» = piedra partida (granito o gneis según la geología del lugar).",
        "titulos": [{k: v for k, v in t.items() if k != "geometria"} for t in tits],
    })
    if previa:
        vista_previa(geo_bin, unidades, tits, cat)
    resumen = {"titulos": len(tits), "unidades": len(unidades), "grilla": [nx, ny], "sin_clasificar": sin_tipo}
    print(f"  país: {len(tits)} títulos, {len(unidades)} unidades geológicas, grilla {nx} × {ny}", flush=True)
    return resumen


COLOR_CATEGORIA = {
    "roca_construccion": (90, 90, 110), "carbonato": (245, 245, 235), "arido": (230, 200, 90), "arcilla": (200, 80, 40),
    "mineral_industrial": (60, 160, 160), "gema": (150, 60, 200), "metal": (230, 170, 0), "combustible": (40, 30, 20),
}


def vista_previa(geo_bin, unidades, tits, cat):
    """Imagen de control: la geología del país en grises por edad y los títulos por categoría."""
    base = os.path.join(RAIZ, "docs", "capturas")
    os.makedirs(base, exist_ok=True)
    cod = geo_bin.astype(np.int32) - 1
    rgb = np.full(geo_bin.shape + (3,), (160, 190, 220), dtype=np.float32)
    tierra = geo_bin > 0
    # Basamento (0–41) gris rosado; cuencas (42–64) pardo; Cenozoico (65–77) amarillo pálido.
    rgb[tierra & (cod <= 41)] = (175, 160, 165)
    rgb[tierra & (cod >= 42) & (cod <= 51)] = (185, 165, 130)
    rgb[tierra & (cod >= 52) & (cod <= 56)] = (120, 125, 120)
    rgb[tierra & (cod >= 57) & (cod <= 64)] = (205, 175, 140)
    rgb[tierra & (cod >= 65)] = (225, 215, 175)
    img = Image.fromarray(rgb.astype(np.uint8)).resize((geo_bin.shape[1] * 2, geo_bin.shape[0] * 2), Image.NEAREST)
    d = ImageDraw.Draw(img)
    categoria = {r["id"]: r["categoria"] for r in cat["recursos"]}
    categoria["roca"] = "roca_construccion"
    for t in tits:
        if not t["materiales"]:
            continue
        x, y = _px(t["lon"], t["lat"])
        col = COLOR_CATEGORIA.get(categoria.get(t["materiales"][0]), (255, 0, 0))
        d.ellipse([2 * x - 2.5, 2 * y - 2.5, 2 * x + 2.5, 2 * y + 2.5], fill=col, outline=(0, 0, 0))
    img.save(os.path.join(base, "recursos-uruguay.jpg"), quality=85)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cache", required=True)
    ap.add_argument("--salida", default=SALIDA)
    a = ap.parse_args()
    os.makedirs(a.cache, exist_ok=True)
    print(json.dumps(construir(a.cache, a.salida), ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
