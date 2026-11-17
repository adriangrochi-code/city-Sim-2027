# Tipos de costa de Montevideo (de Santa Lucía a Arroyo Pando): arena, mixta, roca y barranca

Relevamiento hecho el 17/11/2026 para el mapa de 24 × 24 km. La esquina NO está en -34.778 / -56.325, así que el mapa abarca lon -56.325 … -56.0625 y lat -34.778 … -34.9943.

> **Ojo con los bordes del mapa.** Punta Espinillo, La Colorada, Pajas Blancas, Zabala y Los Cilindros quedan **al oeste** del borde del mapa (lon < -56.325), en el anillo de contexto. Punta Yeguas (-56.3126) queda apenas adentro. Hacia el este, el borde (-56.0625) cae justo en **Punta Mansa**: Carrasco, el Aº Carrasco y Ciudad de la Costa quedan en el anillo de contexto.

Los datos se marcan con estas etiquetas:
- **MEDIDO**: sale de una capa vectorial descargada y procesada por mí (geometría real).
- **IMAGEN**: lo estimé mirando la imagen satelital Esri World Imagery (fecha y marea desconocidas; el agua del Río de la Plata es turbia, así que lo sumergido casi no se ve).
- **DESCRITO**: lo dice textualmente una fuente técnica o científica.
- **ESTIMADO**: inferencia mía a partir de lo anterior. Es la mejor estimación, no un dato.
- **NO VERIFICADO**: no lo pude confirmar.

---

## 1. Qué fuentes de datos funcionaron y cuáles no

### 1.1 Funcionaron (con la URL exacta)

| Fuente | Qué contiene | Request que funcionó |
|---|---|---|
| **IDE Uruguay, WFS vectorial** (`mapas.ide.uy/geoserver-vectorial`) | Capas `hidrografia:` / `hidrografia_urbana:` → `linea_de_costa_l`, `linea_costera_l`, `playa_a` (polígonos de playa), `banco_de_arena_a`, `isla_a`. **No hay** capa de tipo de costa, puntas, acantilados ni rocas. El campo `slt` (*shoreline type*, codificación DGIWG/FACC) existe pero vale **0 (desconocido) en el 100 %** de los tramos. `playa_a` no trae nombre (`nam` = null). | `https://mapas.ide.uy/geoserver-vectorial/ows?service=WFS&version=2.0.0&request=GetFeature&typeNames=hidrografia_urbana:playa_a&outputFormat=application/json&srsName=EPSG:4326&bbox=-35.00,-56.75,-34.70,-55.75,urn:ogc:def:crs:EPSG::4326` (también sirve con `typeNames=hidrografia:playa_a`, `hidrografia_urbana:linea_costera_l`, `hidrografia:linea_de_costa_l`, `hidrografia:isla_a`, `hidrografia_urbana:banco_de_arena_a`). El servidor es intermitente (*connection reset*): a veces hay que reintentar 2 o 3 veces. GetCapabilities: `https://mapas.ide.uy/geoserver-vectorial/ows?service=WFS&version=2.0.0&request=GetCapabilities` |
| **Intendencia de Montevideo, descarga SHP "Espacios públicos"** | `v_sig_espacios_publicos` (EPSG:32721). Campo `DESC_TIPO_` ∈ {PLAYA (34), ISLA (7), PLAZA, …}, con `NOMBRE_ESP` = nombre de la playa. Son polígonos oficiales de las 34 playas con nombre. | La descarga va en **dos pasos**: (1) `https://intgis.montevideo.gub.uy/sit/php/common/datos/generar_zip2.php?nom_tab=v_sig_espacios_publicos&tipo=gis` devuelve un HTML que genera el zip; (2) `https://intgis.montevideo.gub.uy/sit/tmp/v_sig_espacios_publicos.zip` (10 MB). Probé nombres de tabla de playas sueltas (`v_sig_playas`, etc.) y dan error. |
| **OpenStreetMap vía Overpass (espejo mail.ru)** | `natural=bare_rock` (34 polígonos de roca costera en Montevideo), `natural=beach` (con `surface=sand/gravel`), `natural=cape` (nombres de puntas), `place=islet`, `natural=coastline`. **Es la única fuente con polígonos de roca costera.** | `https://maps.mail.ru/osm/tools/overpass/api/interpreter` con POST `data=[out:json][timeout:180];nwr(-35.00,-56.75,-34.70,-55.75)[natural~"^(bare_rock|rock|stone|reef|cliff|shoal|shingle|cape)$"];out geom;`. Las consultas grandes dan 504: conviene partir por tag o por bbox chico. `overpass-api.de`, `lz4`/`z.overpass-api.de`, `kumi.systems`, `private.coffee`, `openstreetmap.ru` y `osm.jp` **no responden** desde este entorno. |
| **API de OSM** (para rellenar la costa oeste) | Mapa completo de un bbox chico | `https://api.openstreetmap.org/api/0.6/map.json?bbox=-56.395,-34.885,-56.325,-34.84` |
| **Esri World Imagery** (inspección visual) | Ortoimagen | `https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/export?bbox=W,S,E,N&bboxSR=4326&imageSR=4326&size=1000,1000&format=png&f=image` |
| MTOP GeoServer | `geoservicios.mtop.gub.uy/geoserver/ows`: responde, pero no tiene nada útil para esta costa. `mb_sayago:*` son curvas de nivel **terrestres** del acceso al puerto de Puntas de Sayago (z = 29 m) y `rec_hidrograficos:v_granulometria` son 31 puntos de granulometría fluvial. `mb_piria:pir_rocas` es de Piriápolis (fuera del área). | `https://geoservicios.mtop.gub.uy/geoserver/ows?service=WFS&request=GetCapabilities` |

### 1.2 No funcionaron / no existen
- **IM GeoServer WFS/WMS** (`geoweb.montevideo.gub.uy/geoserver/...`): *empty reply*. La página oficial https://sig.montevideo.gub.uy/geoservicios-ogc.html avisa: *"GEOSERVICIOS EN INTERNET TEMPORALMENTE FUERA DE SERVICIO … disponibles solo para usuarios de Intranet"*.
- **Ministerio de Ambiente / DINAMA** (`geoservicios.ambiente.gub.uy`, `www.dinama.gub.uy`, `mapas.mvot.gub.uy`, `geo.ide.uy`): el proxy contesta 502. `visualizador.ide.uy` y `sit.mvotma.gub.uy`: *connection reset*. Por eso no pude acceder a capas de "ecosistemas costeros", SNAP ni "tipos de costa", si es que existen.
- **Catálogo de datos abiertos** (`catalogodatos.gub.uy`, API CKAN): no hay ningún dataset de tipo de costa, acantilados, puntas rocosas ni geomorfología costera. Las búsquedas por "costa", "playas", "acantilado", "geomorfologia" y "ecosistemas costeros" solo devuelven calidad de agua, residuos en playas, la boya, etc.
- **Cartas náuticas SOHMA**: no hay versión descargable libre. Ninguna fuente pública consultada da sondas ni distancias de restingas para Punta Brava, Punta Carretas o Punta Yeguas. Solo encontré un comentario náutico genérico que avisa de "piedras sumergidas" en Punta Carretas (sin distancia, NO VERIFICADO).

### 1.3 Archivo derivado listo para usar
`/tmp/claude-0/-home-user-city-Sim-2027/54af6cfe-b132-5c99-b638-f6520dd232c4/scratchpad/costa-osm-roca-playa.geojson` (EPSG:4326, 99 features) contiene:
- `kind=roca`: polígonos OSM `natural=bare_rock`;
- `kind=arena_sand` / `arena_gravel`: playas;
- `kind=punta`: puntos de cabos con nombre.

Hay que tener en cuenta que **OSM solo mapea la roca emergida** (franja intermareal o supramareal pegada a la costa) y que en la costa oeste rural (Espinillo–Cilindros) **no hay roca mapeada**. Además, en Montevideo la línea `natural=coastline` de OSM sigue el muro de la rambla actual.

Otros insumos descargados (también en el scratchpad):
- `dl/ide/*.json`: capas IDE;
- `dl/ep/v_sig_espacios_publicos.*`: SHP de la IM;
- `dl/osm/*.json`;
- `img/*.jpg`: recortes de imagen con escala de 100 m.

---

## 2. Marco geológico y geomorfológico (DESCRITO)

- **Las tres grandes divisiones de la costa** (López Laborde 2003, citado en IM, *Informe final "Ideas para estructurar un plan integral de gestión costera"*, https://montevideo.gub.uy/sites/default/files/biblioteca/informefinalgestioncosteraintegral.pdf, p. 10):
  - **(a) Oeste, de Punta Espinillo a Punta Lobos**: *"sucesión de pequeñas bahías o ensenadas entre puntas rocosas (puntas Espinillo, Monte, Castro, Canario, Pedregal, Tomador, Yeguas, Tigre (Montevideo), Sayago y Lobos)"*.
  - **(b) Punta Lobos – Punta Sarandí**: la bahía, *"en un área deprimida del basamento cristalino"*.
  - **(c) Este**: *"sucesión de arcos arenosos (playas Ramírez, de la Estacada, Pocitos, Buceo, Malvín, Honda, de los Ingleses, Verde, de la Mulata, Carrasco y Miramar) entre puntas rocosas (Pérez, Ramírez, Shannon, Brava, Canario, Trouville, Buceo, Gomensoro, Descanso, Gorda)"*.
  - **Desde la Barra de Carrasco hasta el Pando**: *"una estrecha faja arenosa"* con remanentes de dunas y bañados.
- **Carta Geológica de Montevideo 1:50.000** (Memoria, MIEM/CONICYT 6019, https://www.gub.uy/ministerio-industria-energia-mineria/sites/ministerio-industria-energia-mineria/files/documentos/publicaciones/Memoria%20de%20Carta%20Geologica%20de%20Montevideo.pdf, §7.1):
  - Divide la región costera en **"Faja Costera del Oeste y Centro-Oeste" (Punta Espinillo → Punta Mansa)**, con bahías en media luna y puntas rocosas, y **"Faja Costera del Este" (Punta Mansa → Aº Carrasco)**, que es *"un cordón rectilíneo de playa de dirección N30-45E"*.
  - Dentro de la primera: *"La zona Oeste entre Punta Espinillo y Punta Carretas se caracteriza por presentar una mayor ocurrencia de áreas rocosas que se alternan esporádicamente con microbahías de algunas decenas a pocos cientos de metros"*. *"La zona Este, entre Punta Carretas y Punta Mansa, se caracteriza por la frecuencia de ocurrencia y de tamaño, similares entre áreas de bahía y áreas de rocosidad."*
  - Arenas de playa: *"arenas finas hasta muy gruesas y gravillosas, blanco-amarillentas, cuarzo-feldespáticas"*.
  - Hay **dunas** conservadas entre Playa del Cerro y Punta Espinillo. En el este, las dunas originales fueron destruidas casi por completo, salvo restos en Playa Verde y Carrasco.
- **Litologías de las puntas** (misma Memoria, §5.1):
  - **Granitos Deformados de Punta Espinillo** (granito gris a rosado, foliado, con pegmatitas): afloran en Punta Yeguas, Los Cilindros, Playa La Colorada y Punta Espinillo.
  - **Formación Montevideo** (Paleoproterozoico): ortoanfibolitas verde oscuro, con *pillow lavas* (Cerro, costa de la bahía, Fortaleza), micaesquistos *"especialmente bien expuestos en la playa del Cerro"* y paraneises.
  - **Ortoneises de Punta Carretas** (gneis gris, tenaz): afloran en la *"porción centro oriental de la costa"*.
  - Goso & Muzio (2006), en *Bases para la conservación y el manejo de la costa uruguaya* (https://vidasilvestre.org.uy/wp-content/uploads/2012/09/02_Geologi%CC%81a-de-la-costa-uruguaya-y-sus-recursos-minerales-asociados-Goso.pdf): las mejores exposiciones de la Fm. Montevideo están en *"Playa del Cerro, Punta Espinillo y Pajas Blancas"*, y *"conforman los afloramientos rocosos de las diversas islas y tómbolos"* de la costa de Montevideo y Canelones.
- **Barrancas**:
  - La Carta Geológica las ubica en la **costa del río Santa Lucía (Melilla)**, sobre la Fm. Libertad y Fray Bentos, y **no** describe barrancas sobre el Río de la Plata dentro de Montevideo.
  - En **San José**, el Ministerio de Ambiente (https://www.gub.uy/ministerio-ambiente/politicas-y-gestion/san-jose) describe *"alternancias entre los campos de médanos y las barrancas"*: San Gregorio ~40 m, Arazatí ~20 m, Kiyú, *"pequeñas barrancas del orden de un metro"* cerca del Aº del Tigre y barranca *"de altura considerable sobre Playa Pascual"*.
  - **Canelones**: la Fm. Chuy forma barrancas en la Costa de Oro, pero lejos, al este del Pando (fuera del anillo).
- **Bahía**:
  - Gautreau (2006), *"La Bahía de Montevideo: 150 años de modificación de un paisaje costero y subacuático"*, en *Bases…* (libro completo: https://vidasilvestre.org.uy/wp-content/uploads/2022/06/basesparalaconservacinyelmanejodelacostauruguaya-140814122652-phpapp01-1.pdf, p. 401 y siguientes), describe: *"la costa S y E del Cerro hasta Capurro, con depósitos arenosos; la parte central de la bahía, [limos]"*, fondo de ≤ 2 m al N de la línea entrada del puerto – Punta del Rodeo y unos 5 m de profundidad media.
  - Rellenos: ~60 ha antes de 1900, de los cuales la **Playa de La Aguada se rellenó en 1866-1869**. Puerto entre 1901 y 1909 (los muelles avanzaron 500 m sobre el río). Rambla entre Playa Ramírez y Punta Carretas c. 1900-1915. En total, más de 150 ha ganadas al río hacia 1930.
- **Departamento** (Ministerio de Ambiente, https://www.gub.uy/ministerio-ambiente/politicas-y-gestion/montevideo): 71 km de costa y 19 playas. *"sucesión de arcos arenosos y puntas pedregosas"* tanto al oeste como al este. *"Las puntas rocosas conforman paisajes particulares, con pequeños charcos y pozos de agua entre las rocas"*.
- **Playa del Cerro** (MA, *Casos piloto. Playa del Cerro*, https://www.gub.uy/ministerio-ambiente/sites/ministerio-ambiente/files/2020-08/Casos%20piloto.%20Playa%20del%20Cerro%20%28Montevideo%29._0.pdf): *"playa pequeña, de aproximadamente 500 metros de longitud, encajada entre dos salientes rocosos"*, D50 = 0,25 mm, pendiente de rompientes 0,017, y a veces aparecen gravas en el sector este.
- **Playa Carrasco** (MA, *Casos piloto. Playa Carrasco*, https://www.gub.uy/ministerio-ambiente/sites/ministerio-ambiente/files/2020-08/Casos%20piloto.%20Playa%20Carrasco%20%28Montevideo%29._0.pdf): *"playa de arena fina blanca de aproximadamente 4.000 metros de longitud y 40 metros"* de ancho. Transporte longitudinal ≈ 19.000 m³/año hacia la desembocadura del Aº Carrasco.

---

## 3. Tabla de tramos (oeste → este)

Cómo leer las columnas:

- **Mapa**: si el tramo cae dentro del cuadrado de 24 km (MAPA), en el anillo (CTX) o en ambos.
- **f_roca**: fracción del frente costero que es roca, de 0 a 1, en estado **natural (pre-1726)**.
- **f_roca medido**: cuando aparece, sale de este procedimiento. Muestreé la línea de costa OSM cada 10 m y clasifiqué cada punto según caiga a menos de 30 m de un polígono de playa (IDE + IM + OSM) o de roca (OSM `bare_rock`). La fórmula es f = (roca + ½·ambos)/(arena + roca + ambos).
  - El resto ("otro") es muro de rambla, puerto, relleno o costa no mapeada, y por eso queda excluido.
  - **Cuidado:** en la costa urbana, una parte de "otro" era roca natural que hoy está bajo la rambla. Por eso el f_roca medido es un **mínimo**.
- **Roca mar adentro**: hasta dónde llega la roca hacia el agua, medida desde la línea de costa.
  - **e** = emerge siempre o en marea normal.
  - **b** = aflora con bajante o marea eólica negativa. El rango de marea astronómica es de apenas ~0,3–0,5 m, pero con viento N/NO el nivel baja bastante más.
  - **s** = sumergida y vista en la imagen como "costillas" oscuras. Es **mi interpretación**: no lo verifiqué con sondas.

| # | Tramo | Inicio (lat, lon) | Fin (lat, lon) | Mapa | Tipo natural | f_roca (natural) | Roca mar adentro | Roca típica | Fuente / calidad |
|---|---|---|---|---|---|---|---|---|---|
| 0 | Desembocadura del Santa Lucía y banco del Santa Lucía (San José / Montevideo) | -34.7594, -56.5568 (Pta. del Tigre, San José) | -34.8285, -56.4306 | CTX | **arena** (banco) + bañado/islas fluviales; barrancas en Santa Lucía/Melilla y San José | 0 | Banco **de arena** que *"llega a aflorar en las bajantes, ocupando hasta 3.5 km desde punta del Tigre"* (no es roca) | Arena / limo; barrancas de la Fm. Libertad y Fray Bentos | DESCRITO (IM Informe gestión costera, citando a López Laborde; Carta Geol. Mvd.; MA San José) |
| 1 | Punta Espinillo | -34.8285, -56.4306 | -34.8400, -56.4150 | CTX | **roca** (punta baja) con borde vegetado / bañado | 0.5–0.7 (ESTIMADO) | e: 10–40 m. En la imagen se ven pocas rocas, sobre todo en el lado S de la punta. s: no visible (agua muy turbia) | Granito deformado de grano medio con granate, pegmatitas, foliación protomilonítica ~E-O | DESCRITO (Carta Geol.); IMAGEN; OSM no tiene roca mapeada |
| 2 | Arcos Espinillo → La Colorada | -34.8394, -56.4150 | -34.8590, -56.3760 | CTX | **mixta**: microbahías de arena entre puntas | 0.4–0.6 (ESTIMADO). Medido: arena 38 % de la costa, roca no mapeada | e/b: 10–50 m frente a las puntas | Granito deformado (La Colorada), anfibolita | MEDIDO (playas IDE/IM: 5 polígonos de 90–760 m de largo); DESCRITO |
| 3 | La Colorada → Pta. del Canario (oeste) → Pajas Blancas | -34.8590, -56.3760 | -34.8700, -56.3410 | CTX | **mixta / roca**: bahías chicas entre puntas | 0.5–0.7 (ESTIMADO). Medido: arena 17 % | e/b: 10–30 m (imagen de Pajas Blancas: plataforma angosta entre la playa y el puertito) | Fm. Montevideo (anfibolitas, micaesquistos) | DESCRITO (Goso & Muzio: "mejores exposiciones… Pajas Blancas"); IMAGEN; MEDIDO (arena) |
| 4 | Playa Pajas Blancas → Playa Zabala | -34.8700, -56.3410 | -34.8750, -56.3350 | CTX | **arena** con puntas en los extremos | 0.3 (ESTIMADO). Medido: arena 55 % | e: 10–30 m en los extremos | Anfibolita / gneis | MEDIDO (IM: Pajas Blancas 407 × 236 m, Zabala 171 × 165 m); IMAGEN |
| 5 | Zabala → Los Cilindros | -34.8750, -56.3350 | -34.8876, -56.3312 | CTX | **roca** dominante, con playitas | 0.7–0.9 (ESTIMADO). Medido: arena 3 % | b/s: NO VERIFICADO | Granito deformado (Los Cilindros) | DESCRITO (Carta Geol.); MEDIDO (playa de Los Cilindros: 68 × 76 m) |
| 6 | Los Cilindros → Punta Yeguas | -34.8876, -56.3312 | -34.9042, -56.3126 | CTX→MAPA (cruza lon -56.325) | **mixta**: playitas en bolsillo entre plataformas | 0.6 (ESTIMADO). Medido: arena 22 % | e: plataformas de 20–80 m | Granito deformado | MEDIDO (playas IDE/IM: 3 bolsillos de 40–260 m); IMAGEN |
| 7 | **Punta Yeguas** → Playa Punta Yeguas → Santa Catalina | -34.9042, -56.3126 | -34.8930, -56.2960 | MAPA | **roca** en la punta y **arena** en la Playa Punta Yeguas (660 m) | Medido: **0.58** | e: plataformas de 30–80 m; **islotes emergidos ~350 m mar adentro** (ver §4); s: costillas oscuras rectilíneas hasta ~400–600 m (IMAGEN) | Granito deformado con xenolitos de anfibolita | MEDIDO (OSM bare_rock + playas); IMAGEN |
| 8 | Santa Catalina → **Pta. del Tigre** (Mvd.) → **Pta. de Sayago** | -34.8930, -56.2960 | -34.9016, -56.2758 | MAPA | **roca** dominante, con 2–3 playitas | Medido: **0.77** | e: 20–70 m (plataforma lobulada de ~500 × 460 m en Pta. del Tigre, OSM r15738251); roca suelta 60 m afuera (-34.8971, -56.2789); s: costillas hasta 300–600 m (IMAGEN) | Fm. Montevideo / granito deformado | MEDIDO (OSM); IMAGEN |
| 9 | Pta. de Sayago → Playa del Nacional → **Pta. de Lobos** | -34.9016, -56.2758 | -34.9059, -56.2588 | MAPA | **mixta**: playa de 480 m + costa rocosa larga (Fortaleza / Pta. Lobos, hoy con dique) | Medido: **0.60** | e: plataforma continua de 30–60 m de ancho (≈810 × 400 m en OSM r15738247); s: costillas hasta 300–700 m y mancha oscura ~500 m al S de Pta. Lobos (posible bajo rocoso, **NO VERIFICADO**) | Anfibolita, micaesquisto | MEDIDO; IMAGEN |
| 10 | **Playa del Cerro** | -34.8995, -56.2560 | -34.8930, -56.2460 | MAPA | **arena** de ~500 m encajada entre dos salientes rocosos | Medido: 0.56 (con las puntas incluidas); solo la playa: ~0.1–0.2 | e/b: plataforma oeste de 100–150 m; s: costillas a 300–600 m | Micaesquistos y anfibolitas (Fm. Montevideo); a veces gravas en el sector E | DESCRITO (MA Casos piloto; Carta Geol.); MEDIDO; IMAGEN |
| 11 | Bahía oeste: costa E del Cerro → Pantanoso → Capurro | -34.8930, -56.2460 | -34.8700, -56.2300 | MAPA | Natural: **mixta**, puntas de anfibolita del Cerro + arenas (*"costa S y E del Cerro hasta Capurro, con depósitos arenosos"*); hoy rellenos | Medido hoy: 0.23 de roca, sin arena mapeada (el resto es relleno). Natural: 0.3 (ESTIMADO) | e: islotes Humpreys / del Bizcochero (ver §4); bahía muy playa (≤ 2 m) y fondo limoso | Anfibolita (Fortaleza del Cerro, con *pillow lavas*) | DESCRITO (Gautreau; Carta Geol.); MEDIDO |
| 12 | Bahía interior: Pantanoso → Miguelete → La Aguada → puerto | -34.8700, -56.2300 | -34.9010, -56.2100 | MAPA | Natural: **arena / fango** (playa de La Aguada, rellenada en 1866-69; desembocaduras con bañados) | ~0 (ESTIMADO) | — (fondo limoso, ≤ 2 m) | — | DESCRITO (Gautreau). Hoy todo es puerto o relleno (100 % "otro") |
| 13 | Península (Ciudad Vieja): Punta San José / Punta Sarandí | -34.9010, -56.2100 | -34.9116, -56.1980 | MAPA | **roca** (pre-urbano) | Medido hoy: 0.28 de roca remanente. Natural: 0.7–0.9 (ESTIMADO) | e: banco rocoso remanente de ~590 × 130 m frente a la Rambla Sur (OSM r16117463, centro -34.9110, -56.2046) | Gneis / anfibolita | MEDIDO (OSM); ESTIMADO (Carta Geol.: el tramo Espinillo–Carretas es "mayor ocurrencia de áreas rocosas") |
| 14 | Rambla Sur: Pta. Sarandí → Pta. Pérez / Palermo / Parque Rodó | -34.9116, -56.1980 | -34.9147, -56.1722 | MAPA | Natural: **mixta**, puntas rocosas (Pta. Pérez) con playitas; hoy muro continuo | Medido hoy: 0 (100 % muro). Natural: 0.5 (ESTIMADO) | NO VERIFICADO | Ortoneis de Punta Carretas | DESCRITO (Pta. Pérez en López Laborde); el resto es ESTIMADO |
| 15 | **Playa Ramírez** | -34.9147, -56.1722 | -34.9187, -56.1693 | MAPA | **arena** con punta rocosa al S | Medido: **0.22** | e: 20–60 m en Pta. Ramírez | Ortoneis | MEDIDO (playa IM de 270 × 483 m); IMAGEN |
| 16 | Pta. Ramírez / Pta. Shannon → (Golf) → **Pta. Brava / Pta. Carretas** | -34.9200, -56.1714 | -34.9374, -56.1601 | MAPA | **mixta**: costa rocosa baja con playitas de arena y grava (OSM `surface=gravel` cerca de Pta. Brava) | Medido: **0.32** | e: 10–40 m (rocas emergidas al pie de la rambla; la punta S actual es relleno o emisario); b/s: el comentario náutico dice "piedras sumergidas", sin distancia (**NO VERIFICADO**) | Ortoneis gris de Punta Carretas | MEDIDO; IMAGEN; DESCRITO (Carta Geol.) |
| 17 | Pta. Brava → Pta. del Canario (este) → **Pta. Trouville** | -34.9374, -56.1601 | -34.9197, -56.1461 | MAPA | **roca** | Medido: **0.95** | e: 10–40 m, con una roca aislada ~40 m afuera en el lado E de Pta. Carretas; s: plataforma oscura de ~100–200 m frente a Pta. del Canario (IMAGEN, poco nítida) | Ortoneis | MEDIDO (OSM: Pta. Trouville ≈ 280 × 210 m de roca); IMAGEN |
| 18 | **Pocitos** | -34.9197, -56.1476 | -34.9096, -56.1400 | MAPA | **arena** | Medido: **0.11** (rocas solo en el extremo de Trouville) | — | — | MEDIDO (playa IM de 811 × 1082 m; la literatura le da ~1.700 m de largo) |
| 19 | Pocitos E → **Pta. del Buceo** (puerto) | -34.9096, -56.1380 | -34.9050, -56.1240 | MAPA | **roca** (hoy, puerto del Buceo) | Medido: **0.82** | e: plataforma de ~785 × 350 m en Pta. del Buceo (OSM r8976000); "Isla del Mono" a 59 m de la costa (§4) | Gneis | MEDIDO (OSM) |
| 20 | **Playa del Buceo** | -34.9028, -56.1231 | -34.8983, -56.1137 | MAPA | **arena** | Medido: **0.14** | e: rocas de Pta. Gomensoro, 20–40 m | Gneis | MEDIDO |
| 21 | **Malvín** (Pta. Gomensoro → Pta. del Descanso) | -34.8983, -56.1137 | -34.8995, -56.1007 | MAPA | **arena** ancha + **Isla de las Gaviotas** enfrente | Medido: **0.34** (incluye las puntas y la isla) | e: **Isla de las Gaviotas**, a 321 m de la costa firme más cercana (MEDIDO, OSM) o "a 450 m" de la playa (Sarroca et al. 2006); Pta. del Descanso: 70–100 m de rocas emergidas y rompientes (IMAGEN) | Gneis / roca cristalina (isla de roca desnuda con vegetación) | MEDIDO; DESCRITO; IMAGEN |
| 22 | **Playa Honda** | -34.8995, -56.1007 | -34.8954, -56.0880 | MAPA | **arena** (~600 m) con salientes rocosos en los dos extremos | Medido: **0.15** | e/b: 40–70 m en los extremos | Gneis | MEDIDO; IMAGEN |
| 23 | **Playa de los Ingleses** → **Punta Gorda** | -34.8954, -56.0857 | -34.9001, -56.0815 | MAPA | **mixta → roca** (bolsillo de arena de ~110 m; Pta. Gorda es rocosa) | Medido: arena 33 %; roca no mapeada en OSM (el resto es rambla). Natural: 0.6–0.8 (ESTIMADO) | e: 10–30 m al pie del muro (IMAGEN) | Gneis | MEDIDO (playa IM de 108 × 89 m); IMAGEN; la guía turística menciona "many stones" (NO VERIFICADO) |
| 24 | **Playa Verde**, **Playa de la Mulata** → **Punta Mansa** | -34.8990, -56.0790 | -34.8996, -56.0648 | MAPA (borde E) | **mixta**: dos arcos de arena (586 m y 231 m) con dunas en Playa Verde + puntas | Medido: **0.25** | e: 20–50 m (Pta. Mansa: *"área de rocosidad que conforma la saliente"*, Carta Geol. Fig. 3) | Gneis / anfibolita | MEDIDO; DESCRITO |
| 25 | **Carrasco**: Pta. Mansa → desembocadura del Aº Carrasco | -34.8996, -56.0648 | -34.8820, -56.0310 | CTX (borde) | **arena** fina blanca, rectilínea, con dunas | Medido: **0.00** | — | — | MEDIDO; DESCRITO (MA Casos piloto: ~4.000 m × 40 m) |
| 26 | Ciudad de la Costa: Aº Carrasco → Aº Pando | -34.8820, -56.0310 | ≈ -34.795, -55.86 (Pando, aprox.) | CTX | **arena**: faja estrecha, dunas y bañados remanentes | Medido: **0.00** (18 km) | — | — | MEDIDO (IDE `playa_a` continua); DESCRITO (López Laborde) |

Notas de la tabla:
- **Isla de Flores** cae fuera del mapa: se describe en §4.
- **Puntas citadas por López Laborde que no ubiqué** (Monte, Castro, Pedregal, Tomador): están entre Espinillo y Yeguas, pero no tengo sus coordenadas (**NO VERIFICADO**).
- **Nombres de puntas según OSM** (`natural=cape`), coherentes con la lista de López Laborde:

  | Punta | lat, lon |
  |---|---|
  | Espinillo | -34.8285, -56.4306 |
  | del Canario (oeste) | -34.8677, -56.3536 |
  | Yeguas | -34.9042, -56.3126 |
  | del Tigre (Mvd.) | -34.8996, -56.2873 |
  | de Sayago | -34.9016, -56.2758 |
  | de Lobos | -34.9059, -56.2588 |
  | Ramírez | -34.9200, -56.1714 |
  | Shannon | -34.9246, -56.1718 |
  | Brava | -34.9374, -56.1601 |
  | del Canario (este) | -34.9247, -56.1513 |
  | Trouville | -34.9197, -56.1461 |
  | del Buceo | -34.9100, -56.1302 |
  | Gomensoro | -34.8983, -56.1137 |
  | del Descanso | -34.8995, -56.1007 |
  | Gorda | -34.9001, -56.0815 |
  | Mansa | -34.8996, -56.0648 |

### Resumen de la costa natural de 1726 (ESTIMADO, combinando lo anterior)

- **Oeste (Espinillo → Pta. Lobos)**: es la costa más rocosa. Puntas bajas de granito o anfibolita con **plataformas emergidas de 20–80 m**, **costillas o restingas sumergidas que probablemente llegan a 300–700 m** (Yeguas, Tigre, Sayago, Lobos, Cerro) y **islotes aislados a ~350 m** (Yeguas). Entre las puntas hay playas en bolsillo de 50–700 m, con dunas detrás. Un valor razonable es **f_roca ≈ 0.55–0.75**.
- **Bahía**: arena al S y E del Cerro y fango con bañados al fondo (Pantanoso, Miguelete, La Aguada). Hay roca solo en el Cerro y en algunos islotes. El fondo es muy bajo (≤ 2 m).
- **Península (Ciudad Vieja) y Rambla Sur**: roca, sobre todo gneis, con playitas. Todo está hoy bajo muro o relleno; queda un banco rocoso remanente de ~590 m frente a la Rambla Sur.
- **Este (Ramírez → Pta. Mansa)**: arcos de arena de 0,5–1,7 km (Ramírez, Pocitos, Buceo, Malvín, Honda, Verde) separados por puntas de gneis. Las rocas emergidas llegan a **10–100 m**. **Isla de las Gaviotas** queda a 300–450 m. f_roca ≈ 0.25–0.35.
- **Carrasco y Ciudad de la Costa**: arena continua, con dunas y bañados detrás. **f_roca = 0**.
- **Barrancas**: no encontré ninguna fuente que documente barrancas sobre el Río de la Plata dentro de Montevideo, ni en Pajas Blancas ni en La Colorada (el nombre "La Colorada" no prueba que haya una barranca: **NO VERIFICADO**). Las barrancas verificadas están sobre el **río Santa Lucía (Melilla)** y en la **costa de San José (Playa Pascual y hacia el oeste)**.

---

## 4. Islotes y rocas aisladas cerca de la costa

| Nombre | lat, lon (centroide) | Tamaño | Distancia a la costa firme | Notas / fuente |
|---|---|---|---|---|
| **Isla de las Gaviotas** (frente a Malvín) | -34.9026, -56.1043 | ~170 × 230 m (2,3 ha) | **321 m** (MEDIDO a la línea OSM más cercana); *"a sólo 450 m de la costa"* de Playa Malvín (Sarroca et al. 2006, en *Bases…*); Wikipedia dice 300 m | Roca desnuda con vegetación. Reserva por Decreto 680/990 (https://www.impo.com.uy/bases/decretos/680-1990). VERIFICADO |
| **Isla de Flores** | -34.9411, -55.9251 | ~1,4 × 1,2 km (31 ha en OSM) | **~10,7 km** de la costa (MEDIDO); *"diez millas náuticas de la costa de Buceo"* (documento de áreas protegidas) | Afuera del mapa, al SE (lon > -56.0625). Tiene playas de grava y roca (OSM `beach surface=gravel` y `bare_rock`). VERIFICADO (nombre y posición) |
| Islotes sin nombre frente a **Punta Yeguas** | -34.9000, -56.3039 | 3 rocas de ~20–60 m (OSM r15738248) | **~357 m** (MEDIDO) | En la imagen se ven emergidas, blanqueadas por guano. A ~80 m al SO hay un **casco hundido** (naufragio, no es roca). El nombre no está verificado |
| "Isla del mono" (OSM) en Pta. del Buceo | -34.9071, -56.1251 | ~150 × 110 m | 59 m (hoy integrada al puerto o escollera del Buceo) | El nombre **NO VERIFICADO**: solo figura en OSM, y en la web aparece "Isla de Mono Pier" |
| **Isla de Ratas** / Isla Libertad (bahía) | -34.8887, -56.2342 | ~190 × 230 m | Dentro de la bahía, entre el Cerro y el puerto | Wikidata Q10301134. Es roca (OSM `bare_rock` r7914088). VERIFICADO |
| "Isla Humpreys" (bahía, costa E del Cerro) | -34.8942, -56.2425 | ~160 × 315 m | 119 m | Nombre solo en OSM (**NO VERIFICADO**) |
| "Isla del bizcochero" (bahía) | -34.8779, -56.2415 | ~160 × 140 m | 84 m | Nombre solo en OSM (**NO VERIFICADO**) |
| Roca suelta frente a Santa Catalina / Sayago | -34.8971, -56.2789 | ~80 × 85 m | ~60 m | OSM bare_rock r15738249, sin nombre |
| Roca aislada al E de Pta. Carretas | ≈ -34.9333, -56.1570 | < 10 m | ~40 m | Solo IMAGEN |
| Islote en la bahía (Miguelete / Capurro) | -34.8603, -56.2110 | ~66 × 90 m | 1,5 km de la línea OSM | Figura en OSM `place=islet` y en la IM como "ISLA". Tipo y nombre **NO VERIFICADOS** |
| *(descartados)* 4 islotes en -34.912…-34.914, -56.167 | — | — | — | Están en el **lago del Parque Rodó**, no en el río |
| *(no encontrado)* "Piedra de los Ingleses" | — | — | — | No encontré ninguna fuente que la mencione. Hay "Playa de los Ingleses" (Punta Gorda), pero no una piedra con ese nombre |
| Banco del Santa Lucía (contexto O) | frente a Punta del Tigre (San José, -34.7594, -56.5568) | hasta 3,5 km | — | **Arena**, emerge en bajantes (DESCRITO) |

---

## 5. Limitaciones y qué faltaría
1. **Extensión sumergida**: ninguna fuente pública tiene sondas cerca de la costa. Las "costillas" que aparecen en la imagen entre Yeguas y el Cerro son coherentes con roca foliada sumergida, porque en Punta Yeguas conectan con islotes emergidos. Aun así, **no lo verifiqué**: podrían ser algas o sombras. La forma de confirmarlo sería la carta SOHMA del acceso a Montevideo o la batimetría de la IM.
2. **Marea de la imagen**: es desconocida. Los rangos "e" pueden subestimar la roca que aflora con bajante fuerte (marea eólica).
3. **OSM**: la roca solo está mapeada de Punta Yeguas hacia el este. En el oeste rural (Espinillo–Cilindros), lo "no arena" no está clasificado, así que esos valores son estimaciones a partir de las descripciones geológicas.
4. **Capas del Ministerio de Ambiente / SNAP / EcoPlata**: no fueron accesibles desde este entorno (el proxy devuelve 502). Sería bueno reintentar desde otra red con `geoservicios.ambiente.gub.uy` y `visualizador.ide.uy`.

Los scripts de procesamiento están en `/tmp/claude-0/-home-user-city-Sim-2027/54af6cfe-b132-5c99-b638-f6520dd232c4/scratchpad/scripts/`: `classify.py` (fracciones medidas), `stretches.json` (cajas de cada tramo), `islets.py` (distancias de islotes) e `img.py` (recortes de imagen).
