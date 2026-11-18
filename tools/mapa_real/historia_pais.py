"""Historia del país para el mapa a escala país: pueblos con su año de fundación, caminos reales,
puertos, estancias y saladeros de muestra, años de las vías férreas y de los departamentos.

Son tablas a mano. Cada dato lleva su estado, como el catálogo de recursos:
  DOCUMENTADO  lo dice una fuente citada (casi siempre el artículo de Wikipedia en español de la
               localidad, consultado el 18/11/2026, que a su vez cita a los historiadores locales);
  ESTIMADO     diseño del juego con la base dicha (posiciones de caminos, poblaciones iniciales,
               años de vías que no se encontraron).
Las posiciones de los pueblos salen de la capa de localidades del MTOP (centro del polígono de
la localidad de hoy, que es el casco histórico o lo contiene); si la localidad no está, se usa
la coordenada de la tabla (aproximada a 0,01°).

Lo usa construir_pais.py. Ver docs/sistemas/mapa-pais.md.
"""
import heapq
import math

import numpy as np

WIKI = "https://es.wikipedia.org/wiki/"

# id, nombre, año, tipo, localidad (MTOP), lon, lat, población al fundarse (ESTIMADO), estado,
# fuente (página), nota.
PUEBLOS = [
    ("montevideo", "Montevideo", 1726, "capital", "MONTEVIDEO", -56.205, -34.906, 0, "DOCUMENTADO", "Montevideo",
     "La ciudad del jugador (escala ciudad): fortificación en 1724, fundación con los primeros pobladores en 1726."),
    ("colonia", "Colonia del Sacramento", 1680, "ciudad", "COLONIA DEL SACRAMENTO", -57.84, -34.47, 1500, "DOCUMENTADO", "Colonia_del_Sacramento",
     "Fundada por Manuel Lobo (portugueses) en 1680; española desde el Tratado de San Ildefonso (1777)."),
    ("soriano", "Santo Domingo Soriano", 1718, "reduccion", "VILLA SORIANO", -58.32, -33.39, 400, "DOCUMENTADO", "Villa_Soriano",
     "Reducción de los dominicos: en 1718 pasa al sur del Río Negro y en 1726 a su sitio actual (antes, desde 1624, en otros lugares)."),
    ("las_piedras", "Las Piedras", 1744, "pueblo", "LAS PIEDRAS", -56.22, -34.73, 120, "DOCUMENTADO", "Las_Piedras_(Canelones)",
     "El proceso fundacional empieza el 8 de marzo de 1744 (Luis de Sosa Mascareñas); capilla de San Isidro en 1765."),
    ("maldonado", "San Fernando de Maldonado", 1755, "villa", "MALDONADO", -54.95, -34.90, 200, "DOCUMENTADO", "Maldonado_(Uruguay)",
     "José Joaquín de Viana sale con unas familias en agosto de 1755; Pérez Montero fecha la fundación a fines de 1757."),
    ("santa_teresa", "Fortaleza de Santa Teresa", 1762, "fuerte", None, -53.55, -33.97, 100, "DOCUMENTADO", "Fortaleza_de_Santa_Teresa",
     "Fortaleza empezada por los portugueses en 1762 y tomada por Cevallos en 1763."),
    ("san_carlos", "San Carlos", 1763, "villa", "SAN CARLOS", -54.92, -34.79, 250, "DOCUMENTADO", "San_Carlos_(Uruguay)",
     "Fundada en 1763 por el gobernador Pedro de Cevallos, con familias azorianas."),
    ("paysandu", "Paysandú", 1772, "pueblo", "PAYSANDU", -58.08, -32.32, 100, "DOCUMENTADO", "Paysandú",
     "La tradición fecha la reducción en 1772 (1 de enero); el «paso de Paisandú» ya figura en 1754. Parroquia en 1805."),
    ("rosario", "Rosario del Colla", 1775, "villa", "ROSARIO", -57.35, -34.32, 150, "DOCUMENTADO", "Rosario_(Uruguay)",
     "Villa de Nuestra Señora del Rosario del Colla, fundada por Benito Herosa el 24 de enero de 1775."),
    ("santa_lucia", "San Juan Bautista (Santa Lucía)", 1781, "villa", "SANTA LUCIA", -56.39, -34.45, 150, "DOCUMENTADO", "Santa_Lucía_(Uruguay)",
     "Pobladores desde el 26 de octubre de 1781 (Francisco Zurdo); cabildo en 1783."),
    ("canelones", "Guadalupe (Canelones)", 1783, "villa", "CANELONES", -56.28, -34.52, 300, "DOCUMENTADO", "Canelones",
     "Villa de Nuestra Señora de Guadalupe, fundada el 24 de abril de 1783 (ya tenía 70 casas y capilla)."),
    ("minas", "Concepción de las Minas", 1783, "villa", "MINAS", -55.24, -34.37, 150, "DOCUMENTADO", "Minas_(Uruguay)",
     "Villa de la Concepción de las Minas, trazada por Rafael Pérez del Puerto en marzo de 1783."),
    ("san_jose", "San José", 1783, "villa", "SAN JOSE DE MAYO", -56.71, -34.34, 150, "DOCUMENTADO", "San_José_de_Mayo",
     "Fundada el 1 de junio de 1783 por el teniente de dragones Eusebio Vidal."),
    ("mercedes", "Capilla Nueva de las Mercedes", 1788, "villa", "MERCEDES", -58.03, -33.25, 150, "DOCUMENTADO", "Mercedes_(Uruguay)",
     "Fundada en 1788 por el presbítero Manuel Antonio de Castro y Careaga."),
    ("pando", "Pando", 1788, "pueblo", "PANDO", -55.96, -34.72, 100, "DOCUMENTADO", "Pando_(Uruguay)", "Año de la fundación según el artículo; sin fundador."),
    ("rocha", "Rocha", 1793, "villa", "ROCHA", -54.33, -34.48, 150, "DOCUMENTADO", "Rocha_(Uruguay)",
     "Rafael Pérez del Puerto elige el sitio el 22 de noviembre de 1793; capilla de los Remedios en 1794."),
    ("melo", "Villa de Melo", 1795, "villa", "MELO", -54.18, -32.37, 150, "DOCUMENTADO", "Melo",
     "Fundada el 27 de junio de 1795 por Agustín de la Rosa a orillas del arroyo Conventos, a pedido del virrey Melo."),
    ("dolores", "Dolores", 1801, "pueblo", "DOLORES", -58.22, -33.53, 100, "ESTIMADO", None, "Año de fundación sin verificar en esta corrida."),
    ("belen", "Belén", 1801, "pueblo", "BELEN", -57.78, -30.79, 80, "ESTIMADO", None, "Año sin verificar (pueblo de la frontera del norte)."),
    ("trinidad", "Santísima Trinidad de los Porongos", 1804, "pueblo", "TRINIDAD", -56.90, -33.52, 100, "DOCUMENTADO", "Trinidad_(Uruguay)",
     "Tierras concedidas a fray Manuel de Úbeda el 14 de abril de 1804 (hay controversia sobre los fundadores)."),
    ("florida", "San Fernando de la Florida Blanca", 1809, "villa", "FLORIDA", -56.21, -34.10, 150, "DOCUMENTADO", "Florida_(Uruguay)",
     "Fundada el 24 de abril de 1809 por orden del Cabildo de Montevideo."),
    ("carmelo", "Carmelo", 1816, "pueblo", "CARMELO", -58.28, -34.00, 150, "DOCUMENTADO", "Carmelo_(Uruguay)",
     "Fundada por Artigas el 12 de febrero de 1816 (traslado del pueblo de Las Víboras a la boca del arroyo de las Vacas)."),
    ("salto", "Salto", 1817, "pueblo", "SALTO", -57.96, -31.39, 150, "DOCUMENTADO", "Salto_(Uruguay)",
     "Fuerte de San Antonio en 1757 (Viana y Cevallos), abandonado en 1763; población permanente desde 1817."),
    ("durazno", "San Pedro del Durazno", 1821, "villa", "DURAZNO", -56.52, -33.38, 150, "DOCUMENTADO", "Durazno_(Uruguay)",
     "Fundada por Fructuoso Rivera el 12 de octubre de 1821."),
    ("bella_union", "Santa Rosa del Cuareim", 1829, "pueblo", "BELLA UNION", -57.59, -30.26, 300, "DOCUMENTADO", "Bella_Unión",
     "Fundada en 1829 por Fructuoso Rivera con los guaraníes de las Misiones Orientales; Bella Unión desde 1853."),
    ("nueva_palmira", "Nueva Palmira", 1831, "pueblo", "NUEVA PALMIRA", -58.42, -33.88, 100, "ESTIMADO", None, "Año sin verificar (puerto de Higueritas)."),
    ("tacuarembo", "San Fructuoso (Tacuarembó)", 1832, "villa", "TACUAREMBO", -55.98, -31.71, 150, "DOCUMENTADO", "Tacuarembó",
     "Fundada por el coronel Bernabé Rivera el 27 de enero de 1832; se llama Tacuarembó desde 1912."),
    ("artigas", "San Eugenio del Cuareim", 1852, "pueblo", "ARTIGAS", -56.47, -30.40, 150, "DOCUMENTADO", "Artigas_(Uruguay)",
     "Fundada por Carlos Catalá el 12 de septiembre de 1852."),
    ("treinta_y_tres", "Treinta y Tres", 1853, "pueblo", "TREINTA Y TRES", -54.38, -33.23, 150, "DOCUMENTADO", "Treinta_y_Tres_(Uruguay)",
     "Fundada el 10 de marzo de 1853 (el artículo nombra a Dionisio Coronel y a Carlos Graña)."),
    ("fray_bentos", "Villa Independencia (Fray Bentos)", 1859, "pueblo", "FRAY BENTOS", -58.31, -33.12, 150, "DOCUMENTADO", "Fray_Bentos",
     "Fundada oficialmente el 16 de abril de 1859 como Villa Independencia; Fray Bentos desde 1900."),
    ("nueva_helvecia", "Colonia Suiza (Nueva Helvecia)", 1862, "colonia", "NUEVA HELVECIA", -57.23, -34.30, 200, "ESTIMADO", None, "Colonia agrícola de inmigrantes suizos; año sin verificar en esta corrida."),
    ("castillos", "Castillos", 1866, "pueblo", "CASTILLOS", -53.83, -34.20, 100, "ESTIMADO", None, "Año sin verificar."),
    ("rivera", "Pueblo Rivera", 1867, "pueblo", "RIVERA", -55.55, -30.90, 200, "DOCUMENTADO", "Rivera_(Uruguay)",
     "Acta de fundación en Cuñapirú el 20 de julio de 1867 (decreto de junio); ley de 1862 de la Villa de Ceballos."),
]

# Caminos: (desde, hasta, nombre, año desde el que existe). El año es el del pueblo más nuevo
# del tramo salvo que se diga otro. El TRAZADO es ESTIMADO: se busca el camino más fácil sobre el
# relieve real (por las cuchillas, cruzando los ríos por donde son más angostos, lejos de los
# bañados), como iban las carretas; la existencia de cada ruta entre esos pueblos es la de la
# red de caminos de la época (villas unidas con la capital y la costa del Uruguay).
CAMINOS = [
    ("montevideo", "rosario", "Camino Real a la Colonia", 1730),
    ("rosario", "colonia", "Camino Real a la Colonia", 1730),
    ("colonia", "soriano", "Camino de la costa del Uruguay", 1730),
    ("montevideo", "las_piedras", "Camino de las Piedras", 1744),
    ("montevideo", "pando", "Camino Real a Maldonado", 1757),
    ("pando", "san_carlos", "Camino Real a Maldonado", 1763),
    ("san_carlos", "maldonado", "Camino Real a Maldonado", 1763),
    ("san_carlos", "santa_teresa", "Camino a la Angostura", 1763),
    ("soriano", "mercedes", "Camino de la costa del Uruguay", 1788),
    ("mercedes", "paysandu", "Camino de la costa del Uruguay", 1788),
    ("las_piedras", "canelones", "Camino Real del Norte", 1783),
    ("canelones", "santa_lucia", "Camino a Santa Lucía", 1783),
    ("montevideo", "san_jose", "Camino a San José", 1783),
    ("san_jose", "mercedes", "Camino a Mercedes", 1788),
    ("pando", "minas", "Camino a Minas", 1783),
    ("san_carlos", "rocha", "Camino a Rocha", 1793),
    ("minas", "melo", "Camino a Cerro Largo", 1795),
    ("san_jose", "trinidad", "Camino a Porongos", 1804),
    ("canelones", "florida", "Camino Real del Norte", 1809),
    ("paysandu", "salto", "Camino de la costa del Uruguay", 1817),
    ("florida", "durazno", "Camino Real del Norte", 1821),
    ("trinidad", "durazno", "Camino de Porongos al Durazno", 1821),
    ("salto", "bella_union", "Camino del Cuareim", 1829),
    ("durazno", "tacuarembo", "Camino Real del Norte", 1832),
    ("salto", "belen", "Camino de la costa del Uruguay", 1801),
    ("salto", "tacuarembo", "Camino de Salto a Tacuarembó", 1832),
    ("melo", "treinta_y_tres", "Camino a Treinta y Tres", 1853),
    ("minas", "treinta_y_tres", "Camino de Minas a Treinta y Tres", 1853),
    ("bella_union", "artigas", "Camino del Cuareim", 1852),
    ("mercedes", "fray_bentos", "Camino a Fray Bentos", 1859),
    ("tacuarembo", "rivera", "Camino a la frontera de Rivera", 1867),
    ("rocha", "castillos", "Camino a Castillos", 1866),
    ("colonia", "carmelo", "Camino de la costa del Uruguay", 1816),
]

# Puertos con el año desde el que embarcan en el juego (ESTIMADO salvo nota): en la época
# colonial el comercio legal salía por Montevideo (y Colonia era portuguesa); los puertos del
# litoral se abren con el comercio de vapores (época 2).
PUERTOS = [
    ("montevideo", 1726, "mar", "DOCUMENTADO", "El puerto de la bahía (escala ciudad)."),
    ("colonia", 1680, "rio", "DOCUMENTADO", "Puerto portugués frente a Buenos Aires hasta 1777 (contrabando)."),
    ("maldonado", 1757, "mar", "ESTIMADO", "Bahía de Maldonado: puerto de la comandancia."),
    ("soriano", 1830, "rio", "ESTIMADO", "Puerto del Río Negro."),
    ("mercedes", 1830, "rio", "ESTIMADO", "Puerto del Río Negro."),
    ("carmelo", 1830, "rio", "ESTIMADO", ""),
    ("nueva_palmira", 1831, "rio", "ESTIMADO", ""),
    ("paysandu", 1830, "rio", "ESTIMADO", ""),
    ("salto", 1830, "rio", "ESTIMADO", "Cabecera de la navegación del Río Uruguay (más arriba están los saltos)."),
    ("fray_bentos", 1859, "rio", "ESTIMADO", ""),
]

# Estancias y saladeros de muestra (las demás aparecen solas en la simulación).
ESTANCIAS = [
    ("huerfanas", "Estancia de las Huérfanas (jesuitas)", 1741, -57.67, -34.24, "ESTIMADO",
     "Estancia de los jesuitas con su calera (Calera de las Huérfanas, Colonia). Año aproximado de la estancia."),
    ("del_rey", "Real Estancia del Rey", 1730, -56.10, -34.55, "ESTIMADO", "Estancia del Rey de los primeros años de Montevideo (ubicación aproximada)."),
    ("medina", "Estancia de Francisco de Medina", 1781, -57.45, -34.27, "DOCUMENTADO",
     "Abastece al primer saladero de la Banda Oriental (Colla, 1781; INAC, ver desarrollo-y-tecnologia.md)."),
]
SALADEROS = [
    ("colla", "Saladero de Medina (arroyo Colla)", 1781, -57.40, -34.30, "DOCUMENTADO", "Primer saladero de la Banda Oriental, Francisco de Medina, 1781 (INAC)."),
    ("fray_bentos", "Fábrica de extracto de carne (Fray Bentos)", 1863, -58.30, -33.13, "DOCUMENTADO", "Giebert et Compagnie, 1863 (después Liebig)."),
    ("paysandu", "Saladeros de Paysandú", 1840, -58.07, -32.33, "ESTIMADO", "Saladeros del litoral (año aproximado)."),
    ("salto", "Saladeros de Salto", 1850, -57.95, -31.40, "ESTIMADO", "Saladeros del litoral (año aproximado)."),
]

# Departamentos (nombre de la capa → nombre, año de creación). Los seis de 1816 (Artigas) y los
# siguientes por ley. Años de la historia administrativa conocida; ESTIMADO: no se verificaron
# en esta corrida.
DEPARTAMENTOS = {
    "ARTIGAS": ("Artigas", 1884), "CANELONES": ("Canelones", 1816), "CERRO LARGO": ("Cerro Largo", 1828),
    "COLONIA": ("Colonia", 1816), "DURAZNO": ("Durazno", 1828), "FLORES": ("Flores", 1885), "FLORIDA": ("Florida", 1856),
    "LAVALLEJA": ("Lavalleja", 1837), "MALDONADO": ("Maldonado", 1816), "MONTEVIDEO": ("Montevideo", 1816),
    "PAYSANDU": ("Paysandú", 1828), "PAYSANDÚ": ("Paysandú", 1828), "RIO NEGRO": ("Río Negro", 1868), "RÍO NEGRO": ("Río Negro", 1868),
    "RIVERA": ("Rivera", 1884), "ROCHA": ("Rocha", 1880), "SALTO": ("Salto", 1837), "SAN JOSE": ("San José", 1816),
    "SAN JOSÉ": ("San José", 1816), "SORIANO": ("Soriano", 1816), "TACUAREMBO": ("Tacuarembó", 1837),
    "TACUAREMBÓ": ("Tacuarembó", 1837), "TREINTA Y TRES": ("Treinta y Tres", 1884),
}


def nombre_depto(n):
    return DEPARTAMENTOS.get(n.strip().upper(), (n.strip().title(), None))[0]


def anio_depto(n):
    return DEPARTAMENTOS.get(n.strip().upper(), (None, None))[1]


# Vías férreas: año de habilitación de cada tramo según su línea y su latitud. DOCUMENTADO:
# Wikipedia, «Ferrocarril en Uruguay» (Bella Vista–Las Piedras 1/1/1869, Durazno 1874, Paso de
# los Toros 1887, Tacuarembó 1891, Rivera 1892, Pando–Minas 1888, Trinidad 1888); Santa Lucía
# 1872 de la historia de Canelones («el tramo que la une con Montevideo llegó en 1872»). Lo demás
# ESTIMADO (orden de magnitud: las líneas del litoral y del este se terminan entre 1887 y 1930).
def anio_via(linea, pts):
    nombre = (linea or "").upper()
    lat = sum(p[1] for p in pts) / len(pts)
    lon = sum(p[0] for p in pts) / len(pts)
    if "RIVERA" in nombre:
        # La línea sube de Montevideo (34,9° S) hacia el norte.
        if lat < -34.70:
            return 1869, "DOCUMENTADO: Bella Vista–Las Piedras, 1 de enero de 1869"
        if lat < -34.44:
            return 1872, "DOCUMENTADO: llega a Santa Lucía y Canelones en 1872"
        if lat < -33.37:
            return 1874, "DOCUMENTADO: llega al Durazno en 1874"
        if lat < -32.80:
            return 1887, "DOCUMENTADO: Paso de los Toros en 1887"
        if lat < -31.70:
            return 1891, "DOCUMENTADO: Tacuarembó en 1891"
        return 1892, "DOCUMENTADO: Rivera en 1892"
    if "MINAS" in nombre and "ANCAP" not in nombre:
        return 1888, "DOCUMENTADO: de Pando a Minas en 1888"
    if "TRINIDAD" in nombre:
        return 1888, "DOCUMENTADO: hacia Trinidad en 1888"
    if "CUAREIM" in nombre or ("LITORAL" in nombre and lat > -31.4) or ("ARTIGAS" in nombre and lat > -31.0):
        return 1887, "ESTIMADO: North Western, de Salto al Cuareim"
    if "LITORAL" in nombre or "ARTIGAS" in nombre:
        return 1890, "ESTIMADO: Midland, Paysandú y Salto"
    if "MELO" in nombre or "RIO BRANCO" in nombre or "KM" in nombre:
        return 1909, "ESTIMADO: la línea del este llega a Melo a principios del siglo XX"
    if "MERCEDES" in nombre or "COLONIA" in nombre:
        return 1901, "ESTIMADO"
    if "FRAY BENTOS" in nombre:
        return 1911, "ESTIMADO"
    if "PALOMA" in nombre:
        return 1940, "ESTIMADO"
    if "ROCHA" in nombre:
        return 1928, "ESTIMADO"
    if "ARROZAL" in nombre:
        return 1935, "ESTIMADO"
    if "ANCAP" in nombre or "PIEDRA SOLA" in nombre or "PUMA" in nombre:
        return 1955, "ESTIMADO: ramales industriales del siglo XX"
    return 1910, "ESTIMADO: tramo sin línea conocida"


# --- Armado ---------------------------------------------------------------------------------------
def _sin_tildes(s):
    import unicodedata
    return "".join(c for c in unicodedata.normalize("NFD", s) if unicodedata.category(c) != "Mn").upper()


def armar(localidades, celda, alt, dep, deptos, agua, rio=None):
    """Resuelve posiciones y rutas. `localidades`: {NOMBRE: (lon, lat, área, depto)}."""
    loc = {_sin_tildes(k): v for k, v in localidades.items()}
    ny, nx = alt.shape
    pueblos = []
    pos = {}
    for (pid, nombre, anio, tipo, clave, lon, lat, pob, estado, pagina, nota) in PUEBLOS:
        fuente_pos = "tabla"
        if clave and _sin_tildes(clave) in loc:
            l = loc[_sin_tildes(clave)]
            # La localidad de hoy tiene que estar cerca de la coordenada aproximada (homónimos).
            if abs(l[0] - lon) < 0.15 and abs(l[1] - lat) < 0.15:
                lon, lat = l[0], l[1]
                fuente_pos = "localidades MTOP"
        c, f = celda(lon, lat)
        # Si cae en el agua (puertos), se corre a la tierra más cercana.
        c, f = _a_tierra(c, f, agua)
        pos[pid] = (c, f)
        d = int(dep[int(round(f)), int(round(c))])
        pueblos.append({
            "id": pid, "nombre": nombre, "anio": anio, "tipo": tipo, "pos": [round(c, 2), round(f, 2)],
            "lon": round(lon, 4), "lat": round(lat, 4), "posicion": fuente_pos,
            "departamento": nombre_depto(deptos[d - 1]) if d > 0 else None,
            "poblacion_inicial": pob, "estado": estado, "fuente": (WIKI + pagina) if pagina else None, "nota": nota,
        })
    costo = _costo(alt, agua, dep, rio)
    caminos = []
    for a, b, nombre, anio in CAMINOS:
        ruta = _ruta(costo, pos[a], pos[b])
        caminos.append({"desde": a, "hasta": b, "nombre": nombre, "anio": anio, "estado": "ESTIMADO",
                        "nota": "trazado buscado sobre el relieve real (cuchillas, pasos angostos, sin bañados)",
                        "puntos": ruta})
    puertos = [{"pueblo": p, "desde": y, "tipo": t, "estado": e, "nota": n} for p, y, t, e, n in PUERTOS]

    def punto(lon, lat):
        c, f = celda(lon, lat)
        c, f = _a_tierra(c, f, agua)
        return [round(c, 2), round(f, 2)]
    estancias = [{"id": i, "nombre": n, "anio": y, "pos": punto(lo, la), "estado": e, "nota": t} for i, n, y, lo, la, e, t in ESTANCIAS]
    saladeros = [{"id": i, "nombre": n, "anio": y, "pos": punto(lo, la), "estado": e, "nota": t} for i, n, y, lo, la, e, t in SALADEROS]
    deps = [{"nombre": nombre_depto(n), "creado": anio_depto(n)} for n in deptos]
    return {
        "descripcion": "Historia del país (escala país). DOCUMENTADO / ESTIMADO en cada dato. Ver docs/sistemas/mapa-pais.md.",
        "consultado": "2026-11-18",
        "pueblos": pueblos, "caminos": caminos, "puertos": puertos, "estancias": estancias, "saladeros": saladeros,
        "departamentos": deps,
    }


def _a_tierra(c, f, agua):
    ny, nx = agua.shape
    i, j = int(round(f)), int(round(c))
    if 0 <= i < ny and 0 <= j < nx and not agua[i, j]:
        return c, f
    for r in range(1, 6):
        mejor = None
        for di in range(-r, r + 1):
            for dj in range(-r, r + 1):
                a, b = i + di, j + dj
                if 0 <= a < ny and 0 <= b < nx and not agua[a, b]:
                    d = di * di + dj * dj
                    if mejor is None or d < mejor[0]:
                        mejor = (d, b, a)
        if mejor:
            return float(mejor[1]), float(mejor[2])
    return c, f


def _costo(alt, agua, dep, rio=None):
    """Costo de andar por cada celda (carretas): llano 1, pendiente, ríos y fuera del país.
    Cruzar un río cuesta (así el camino lo cruza de frente y no corre por el cauce)."""
    gy, gx = np.gradient(alt)
    pend = np.hypot(gx, gy) / 1000.0
    c = 1.0 + pend * 400.0
    if rio is not None:
        c = c + np.where(rio > 40, 25.0, 0.0)
    c = np.where(agua, 60.0, c)          # cruzar agua grande: muy caro (pasos y balsas)
    c = np.where(dep == 0, c + 30.0, c)  # fuera del país: se evita
    return c.astype(np.float32)


def _ruta(costo, a, b):
    """A* en la grilla (8 vecinos) entre dos puntos; devuelve la polilínea simplificada."""
    ny, nx = costo.shape
    s = (int(round(a[1])), int(round(a[0])))
    t = (int(round(b[1])), int(round(b[0])))
    dist = {s: 0.0}
    prev = {}
    h = lambda p: math.hypot(p[0] - t[0], p[1] - t[1])  # noqa: E731
    pq = [(h(s), 0.0, s)]
    vec = [(1, 0, 1.0), (-1, 0, 1.0), (0, 1, 1.0), (0, -1, 1.0), (1, 1, 1.414), (1, -1, 1.414), (-1, 1, 1.414), (-1, -1, 1.414)]
    while pq:
        _, g, p = heapq.heappop(pq)
        if p == t:
            break
        if g > dist.get(p, 1e18):
            continue
        for di, dj, w in vec:
            q = (p[0] + di, p[1] + dj)
            if not (0 <= q[0] < ny and 0 <= q[1] < nx):
                continue
            ng = g + w * 0.5 * (costo[p] + costo[q])
            if ng < dist.get(q, 1e18):
                dist[q] = ng
                prev[q] = p
                heapq.heappush(pq, (ng + h(q), ng, q))
    camino = [t]
    while camino[-1] != s and camino[-1] in prev:
        camino.append(prev[camino[-1]])
    camino.reverse()
    pts = [(float(a[0]), float(a[1]))] + [(float(j), float(i)) for i, j in camino[1:-1]] + [(float(b[0]), float(b[1]))]
    pts = _rdp(pts, 0.7)
    return [[round(x, 2), round(y, 2)] for x, y in pts]


def _rdp(pts, tol):
    if len(pts) < 3:
        return pts
    (x0, y0), (x1, y1) = pts[0], pts[-1]
    dx, dy = x1 - x0, y1 - y0
    n = math.hypot(dx, dy) or 1e-9
    k, dmax = 0, -1.0
    for i in range(1, len(pts) - 1):
        d = abs(dx * (pts[i][1] - y0) - dy * (pts[i][0] - x0)) / n
        if d > dmax:
            k, dmax = i, d
    if dmax > tol:
        return _rdp(pts[:k + 1], tol)[:-1] + _rdp(pts[k:], tol)
    return [pts[0], pts[-1]]
