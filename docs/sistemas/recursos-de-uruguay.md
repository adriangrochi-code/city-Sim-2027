# Recursos naturales del Uruguay: catálogo, materiales y producción

> Documento de sistema. Estado: **datos y motor**. Catálogo en
> `assets/datos/recursos_uruguay.json` (lo lee `engine/src/city/recursos/materiales.rs`),
> materiales con tipo del mapa de Montevideo en `assets/mapas/montevideo/materiales*.bin`, y el
> dataset del país en `assets/datos/uruguay/` para el mapa futuro del Uruguay. Lo complementa
> [recursos-del-suelo.md](recursos-del-suelo.md) (aptitud de la tierra, molestias, valor del
> suelo).

## Qué resuelve

No es lo mismo sacar amatista que caliza: la amatista sale de geodas en el basalto de Artigas, a
pico y en galerías, se clasifica a mano en una docena de calidades y vale mucho por kilo; la caliza
sale de lentes del basamento de Lavalleja y Treinta y Tres, se quema con leña para hacer cal o se
muele con arcilla, yeso y hierro para hacer cemento, y vale poco por tonelada. El catálogo dice,
para cada recurso del país:

- **qué es** y en qué **categoría** va (roca de construcción u ornamental, carbonato, árido,
  arcilla, mineral industrial, gema, metal, combustible, sal, agricultura, ganadería, monte);
- **dónde** aparece: las unidades de la carta geológica de DINAMIGE (con un peso por unidad, para
  inferir dónde puede haber aunque no haya títulos) y, para el agro, los suelos CONEAT;
- **quién lo produce**: departamentos y localidades (los títulos mineros del catastro, contados);
- **para qué** sirve y **qué productos** salen (cal viva, cemento, adoquines, ladrillos, tejas,
  vidrio, joyería…), con el **material de construcción** que da (`construccion`: "piedra",
  "ladrillo", "teja", "cal", "arena", "madera", "adobe", "paja", "cuero");
- **cuánto** se produce (la estadística oficial 2010–2015, o la de 1977–1981 de la carta de
  materias primas);
- **desde cuándo** (año en que el juego lo habilita y los hitos con su fuente);
- **precio, trabajo y capital**;
- si **hay en Montevideo** (y en qué canal del mapa);
- el **estado** del dato: **MEDIDO** (servicio o estadística oficial), **DOCUMENTADO** (lo dice
  una fuente citada) o **ESTIMADO** (diseño del juego, con la base dicha), y sus **fuentes**.

Todo se consultó el **17/11/2026**. Las cifras de los títulos son de la Consulta Minera 2026 de
DINAMIGE; las de producción, de las estadísticas del MIEM.

## Fuentes: qué funcionó y qué no

| Consulta | Resultado |
|---|---|
| `https://geoportal.miem.gub.uy/arcgis1091/rest/services/Dinamige_ConsMin/Consulta_Minera_2026/MapServer/{2,3,4,5}/query` (`where=1=1`, `outFields=…MINERALES,PADRONES,…`, `f=geojson`, `outSR=4326`, de a 100) | **Funcionó**: 32 + 602 + 920 + 421 = 1.975 títulos de todo el país con la **sustancia** (`MINERALES`, p. ej. «TOSCA, BALASTO», «AMATISTA LAJA DE 1º»), el tipo de título, el estado, los padrones (con el departamento), la fecha de otorgamiento y el área. Se guardan sin empresas ni contactos. El servidor corta conexiones seguido: se reintenta. Las consultas con `groupByFieldsForStatistics` sobre estas capas dan error 400 (son vistas): se bajan los objetos y se cuentan acá. |
| `…/Dinamige_GeoS/CatastroMineroGeoS/MapServer/4` y `/5` (lo que usaba el mapa antes) | Funciona, pero **sin sustancia**: solo geometría. Reemplazado por la consulta de arriba. |
| `…/arcgis1091/rest/services/Consulta_Minera?f=json` | Pide token (`499 Token Required`): no se usa. |
| `…/Dinamige_GeoS/GeologiaGeoS/MapServer/0` (Regiones mineras) | **Funcionó**: una sola región, el «Distrito Gemológico Los Catalanes» (ágatas y amatistas). |
| `…/Dinamige_GeoS/Mapa_Base_Unidades_Geologicas_2017_GeoS/MapServer/2/query` en todo el país (`maxAllowableOffset=0.003`) | **Funcionó**: 4.081 polígonos, 78 unidades con su descripción litológica. |
| `…/Canon/Pedimentos_Historicos/MapServer/0` | Existe (pedimentos 2016 con `DescTemaPrimario`), sin sustancia útil: no se usa. |
| `…/Hosted/analisis_geoquimicos`, `muestras_total` | Existen; quedan para más adelante (geoquímica de muestras). |
| MIEM: *Producción nacional de minerales 2010–2015* (PDF) | **Funcionó**: tabla por mineral, en miles de toneladas. |
| MIEM: *Estadística de producción minera 2002* (PDF) | Funcionó (por departamento y mineral); se citan cifras sueltas. |
| DINAMIGE (1987): *Memoria de la Carta de Materias Primas Minerales No Metálicas* (PDF, 121 p.) | **Funcionó**: usos, yacimientos, producción 1977–1981, hornos de cal, cemento, vidrio, cerámica, arenas de Montevideo. Es la fuente principal de los usos y de las localidades. |
| *Memoria de la carta geológica de Montevideo* (PDF, en el sitio del MGAP) | **Funcionó**: gneises de la Formación Montevideo (Canteras Montevideo), Granito de La Paz alterado a balasto, Villa Soriano en el Santa Lucía y el Parque Lecocq, dunas y arenas costeras. |
| Calera de las Huérfanas (FING), cal jesuita (FHCE), cemento «Artigas» 1919 (FING), Minas de Corrales 1878, Valentines/Aratirí, Conchillas 1887, adoquines labrados por presos (Jardín Botánico), turberas de Laguna Negra (Ministerio de Ambiente), Tannat 1870 (Uruguay XXI), Ley Forestal 1987 | Búsquedas en la web; se citan como DOCUMENTADO con la fuente. **No encontré** la fecha de inicio de la explotación de amatista (queda ESTIMADA), de dónde venía la cal de la Ciudadela, ni la apertura de la planta de ANCAP en Minas. |
| DIEA (anuario, lechería, arroz) | La página del anuario existe; **no bajé** las tablas regionales: las zonas agropecuarias del catálogo son DOCUMENTADAS en forma general (CONEAT + DIEA) sin cifras. |

## Materiales del mapa de Montevideo

Once canales de `materiales.bin` (ley 0..1 por punto), uno por material con su entrada en el
catálogo. Los pesos por unidad geológica (`GEO_MAT` en `tools/mapa_real/recursos_suelo.py`) son
**ESTIMADOS** de la descripción litológica de la carta; los títulos del catastro y las playas,
médanos y bañados de la IDE son **MEDIDOS**.

| Canal | Material | De dónde sale en el mapa | Construcción | Lo saca |
|---|---|---|---|---|
| 0 | granito | Granito de La Paz (0,85), granitos y granitoides transamazónicos (0,75–0,8), migmatitas de Suárez (0,5); más donde aflora (rocosidad CONEAT) y en las colinas | piedra | cantera: piedra, adoquines y sillería |
| 1 | gneis | Formación Montevideo (0,8), ortogneises (0,75), zonas de cizalla; la costa rocosa | piedra | cantera: piedra de mampostería (y algún sillar) |
| 2 | balasto | la roca alterada bajo el suelo (Granito de La Paz 0,8: el pedregullo de Montevideo), areniscas conglomerádicas (Mercedes, Migues, Raigón) | — | cantera: algo de piedra y arena |
| 3 | canto rodado | lecho de los arroyos que bajan del basamento; Villa Soriano, Raigón, Mercedes | piedra | cantera: piedra y arena |
| 4 | arena | playas y bancos (IDE), 400 m de médanos, grupos CONEAT 07, arenas litorales, Chuy, Villa Soriano, arroyos anchos | arena | cantera (arenera): arena para el comercio |
| 5 | arcilla roja | lodolitas de Libertad (0,75), Dolores (0,7), Fray Bentos (0,55), menos en los bajos | ladrillo (y adobe) | horno: ladrillos |
| 6 | arcilla gris | aluviones («Actual», 0,55), Villa Soriano (0,45), bajos junto a los arroyos (+0,35), suelos hidromórficos (grupos 03) y bañados | teja | horno: menos ladrillos y tejas |
| 7 | tosca | limos con carbonato de Libertad (0,5) y Fray Bentos (0,6), no en los bajos | — | cantera: poca piedra (caminos) |
| 8 | conchilla | Formación Villa Soriano (0,35, **estimado**: la carta de 1987 la explota en Villa Soriano y Piriápolis) | cal | calera (rinde 70 %) |
| 9 | caliza | **0 en todo el mapa y su entorno**: ninguna unidad calcárea en la carta 1:500.000, ningún título de caliza en Montevideo, Canelones o San José dentro de la ventana | cal | calera |
| 10 | turba | bañados (0,55) y aluviones bajos (la unidad «Actual» nombra depósitos de turba) | — | — |

Se generan con `python3 tools/mapa_real/construir_mapa.py --cache CARPETA --solo-recursos` (que
usa la caché sin red si está completa). Grillas: el mapa a 40 m (601 × 601 × 11 bytes), el
entorno a 200 m desde −25 km (371 × 371 × 11). Reemplazan a `minerales*.bin` (cuatro canales
genéricos). En la ventana hay **181 títulos mineros**: tosca, balasto, arena, piedra partida,
granito de color, arcilla para cerámica roja; ninguno de caliza.

Media de la ley en la tierra del mapa (y fracción con ley ≥ 0,15): granito 0,03 (4 %), gneis 0,14
(15 %), balasto 0,06 (20 %), canto rodado 0,06 (18 %), arena 0,05 (11 %), arcilla roja 0,52
(74 %), arcilla gris 0,10 (24 %), tosca 0,32 (67 %), conchilla 0,006 (2 %), caliza 0, turba 0,013
(2 %).

Imagen de control: `docs/capturas/recursos-minerales.jpg` (el material de más ley en cada celda,
con los colores de la capa del juego).

## En el juego

- **Cantera** (`Kind::Cantera`): saca el material que manda en su sitio, entre granito, gneis,
  balasto, canto rodado, arena y tosca (`materiales::producto_cantera`). Granito → piedra a pleno
  más 40 adoquines y 10 sillares por mes; gneis → piedra a pleno (mampostería) y 4 sillares;
  balasto → 55 % de piedra (pedregullo) y algo de arena; canto rodado → 60 % y arena; arena → nada
  de piedra y 30 carretadas de arena; tosca → 30 % (relleno). Sin nada que sacar no se puede poner.
  En el Cerro saca gneis; en La Paz, granito o, sobre todo, el balasto del granito alterado.
- **Horno de ladrillos** (`Kind::Horno`): con arcilla roja, ladrillos a pleno; con arcilla gris,
  60 % de ladrillos y 12 cientos de tejas por mes (`producto_horno`).
- **Horno de cal** (`Kind::Calera`, nuevo): necesita caliza o conchilla en el sitio (la conchilla
  rinde 70 %); da 30 fanegas de cal por mes a rinde 1. **Alrededor de Montevideo no hay caliza**:
  solo se puede en la conchilla estimada de Villa Soriano (desembocadura del Santa Lucía).
- **Cal importada**: las obras piden cal para los morteros (0,35 fanegas por unidad de ladrillos y
  0,2 por unidad de piedra que se usaron el mes anterior: estimación). Lo que no dan las caleras se
  trae por barco al precio del mundo más un 25 % de flete y suma a las importaciones del mes
  (`Resultado::cal_importada`, panel de Desarrollo).
- **Productos nuevos del comercio** (`Prod`): adoquines (0,3 pesos), sillería (3), arena (0,5 la
  carretada), tejas (1,2 el ciento) y cal (0,8 la fanega): precios **ESTIMADOS**. La ciudad usa lo
  suyo y embarca el resto (arena 80 %, adoquines 40 %, sillería 30 %, tejas 20 %; la cal, lo que
  sobra de las obras); las ventas van al cabildo («Canteras, hornos y caleras: ventas»). Las
  partidas viejas, con 8 productos, se completan al cargar.
- **Capas de datos** (cajón Capas, y solas al elegir la herramienta): *Piedra* (granito rosado,
  gneis gris; al elegir la cantera), *Arcilla* (roja, gris; al elegir el horno), *Arena y grava*
  (arena amarilla, canto azul, balasto pardo) y *Cal, tosca y turba* (caliza, conchilla blanca,
  tosca ocre, turba oscura; al elegir la calera). Cada celda toma el color del material de más ley
  del grupo, más intenso cuanto más ley.
- **Ficha del terreno**: los materiales con ley ≥ 0,15, de mayor a menor, con su calidad y su
  porcentaje, para qué sirven los tres primeros y, si no hay caliza, de dónde viene la cal.
- **Fichas de cantera, horno y calera**: qué saca (material, ley), cuánto da a la ciudad y qué
  vende.

## API para otros sistemas

En `engine/src/city/recursos/`:

- `materiales::catalogo()`, `entrada(id)`, `de_construccion(clave)`,
  `materiales_de_construccion(clave)` y `se_saca_del_suelo(clave)`: qué recursos dan cada material
  de construcción ("piedra", "ladrillo", "teja", "cal", "arena", "madera", "adobe", "paja",
  "cuero") y si sale del suelo del mapa o se importa. `Entrada::construccion` (principal) y
  `construccion_tambien` (secundarios: la arcilla roja da también adobe).
- `Material` (los once canales): `id`, `nombre`, `construccion`, `grupo`, `color`, `entrada`.
- `RecursosLote::mat` (ley de cada material), `principal()`, `derivar()` (piedra, arcilla, arena y
  grava de siempre); `Uso::Mat(m)` y `Uso::Cal`; `Uso::de_clave` acepta los ids del catálogo.
- `Recursos::materiales_cerca(p, radio)` (la mejor ley de cada material a menos de `radio`) y
  `Recursos::construccion_cerca(clave, p, radio)` (cuánto hay de un material de construcción
  cerca; 0 si se importa).
- `rinde(Kind, &lote)`, `produccion_sitio(Kind, &lote)`; ya calculado por edificio:
  `City::suelo.sitios`.

## Dataset del país (`assets/datos/uruguay/`)

Para el mapa del Uruguay entero (todavía no hay mapa; lo leen las pruebas):

- `geologia_pais.bin`: grilla de 510 × 557 celdas de ~1 km (0,0107° × 0,009°, desde 58,50° O y
  30,05° S), `uint8` = código de la unidad de DINAMIGE + 1 (0 = agua o fuera).
- `pais.json`: la grilla, las 78 unidades con su descripción, área y **potencial de cada material**
  (los pesos `unidades_dinamige` del catálogo: ESTIMADO), los **títulos por material y
  departamento** (MEDIDO) y la región minera de Los Catalanes.
- `titulos_mineros.json`: 1.975 puntos (centro del título) con sus materiales del catálogo, las
  sustancias como las escribe el catastro, el tipo (concesión, exploración, prospección, solicitud,
  cantera de obra pública, reserva), el estado, el departamento, el año y las hectáreas.
- Imagen de control: `docs/capturas/recursos-uruguay.jpg` (basamento gris rosado, cuencas
  pardas, basalto gris, sedimentos cenozoicos amarillos; los títulos por categoría: violeta gemas,
  naranja metales, amarillo áridos, blanco carbonatos, rojo arcillas).

Se genera con `python3 tools/mapa_real/recursos_uruguay.py --cache CARPETA` (también al final de
`--solo-recursos`). Títulos por material (los que dicen la sustancia; 473 no la dicen):

| Material | Títulos | Departamentos con más títulos |
|---|---|---|
| tosca | 761 | Durazno 102, Río Negro 92, Paysandú 90, Soriano 69, Tacuarembó 65 |
| amatista / ágata | 204 / 204 | Artigas 196 / 194, Salto 5 / 7 |
| oro (y plata) | 165 | Rivera 80, Lavalleja 27, Durazno 26, Treinta y Tres 12, San José 10 |
| balasto | 163 | Canelones 53, Colonia 28, Rocha 21, Maldonado 12, Montevideo 10 |
| hierro | 146 | Rivera 84, Durazno 25, Lavalleja 18, Treinta y Tres 11, Florida 8 |
| piedra partida o bruta («roca») | 128 | Maldonado 29, Canelones 14, Rocha 12, Colonia 11, Paysandú 8 |
| cobre, plomo, zinc | 111 | Rivera 35, Durazno 24, Lavalleja 24 |
| níquel, cromo, platinoides, tierras raras, litio, diamantes | 95 | Rivera 32, Durazno 23, Lavalleja 18 (prospección) |
| caliza | 82 | Lavalleja 23, Treinta y Tres 22, Maldonado 13, Paysandú 13, Rivera 5 |
| arena | 74 | San José 24, Maldonado 14, Canelones 10, Colonia 9, Salto 7 |
| manganeso | 44 | Rivera 23, Lavalleja 11 |
| fosfatos (apatita) | 25 | Lavalleja 10, Rivera 7 |
| dolomita | 23 | Maldonado 11, Lavalleja 10, Treinta y Tres 3 |
| granito | 20 | Canelones, Flores, Rivera, Rocha (3 cada uno), Colonia 2 |
| canto rodado | 20 | Maldonado 7, Salto 6, Paysandú 4 |
| piedra laja | 13 | Lavalleja 7, Treinta y Tres 5 |
| arcilla roja | 10 | Cerro Largo 5, Canelones 3 |
| bentonita | 9 | Cerro Largo 9 |
| feldespato y cuarzo | 8 | Rocha 2, Florida, Lavalleja, Maldonado, Rivera |
| mármol | 5 | Maldonado 4, Lavalleja 1 |
| caolín | 4 | Durazno 3 (Blanquillo), Cerro Largo 1 |
| basalto | 4 | Artigas 2, Rivera, Salto |
| talco, granate, corindón, grafito, fluorita, yeso, arenas negras | 1–2 cada uno | Lavalleja, Rivera, San José, Durazno |

## Producción agropecuaria por zona de suelos

Van en el catálogo como recursos de las categorías agricultura, ganadería y monte, con las
unidades geológicas donde dominan (para el mapa del país) y su material de construcción cuando lo
dan (madera, paja, cuero). Son **DOCUMENTADAS en forma general** (CONEAT, DIEA, Ley Forestal); las
cifras por región quedan pendientes.

| Zona (suelos / geología) | Producción | Departamentos |
|---|---|---|
| Basalto superficial (Arapey) y sierras del cristalino: IP CONEAT bajo, no aptos para cultivos | ganadería extensiva de vacunos y ovinos (carne, cueros, lana) | Artigas, Salto, Paysandú, Tacuarembó, Rivera, Durazno, Lavalleja |
| Limos profundos del litoral oeste (Fray Bentos, Libertad; grupos CONEAT 10 y 11) | agricultura de secano: trigo, maíz, cebada, soja | Soriano, Río Negro, Paysandú, Colonia |
| Limos del sur cerca de Montevideo | lechería (cuenca lechera), horticultura, fruticultura y vid (Tannat desde 1870) | San José, Florida, Colonia, Canelones, Montevideo |
| Planicies inundables de la Laguna Merín (Chuy, aluviones) | arroz con riego | Treinta y Tres, Rocha, Cerro Largo (y Artigas) |
| Suelos arenosos sobre areniscas (Tacuarembó, cretácicas) y arenas costeras | forestación (eucalipto, pino): madera, celulosa, leña | Rivera, Tacuarembó, Río Negro, Paysandú |
| Riberas y quebradas | monte nativo: leña y maderas duras | todo el país |
| Bañados | paja brava, totora, junco (techos); turba | Rocha, los bañados de todo el sur |

## Catálogo

Generado del JSON (los conteos de títulos son los de `pais.json`).

### Rocas de construcción y ornamentales

| Recurso | Estado | Dónde (geología) | Departamentos / localidades | Usos y productos | Producción | Títulos (catastro 2026) | Desde | Precio · trabajo · capital | En Montevideo | Fuentes |
|---|---|---|---|---|---|---|---|---|---|---|
| **Granito (y granodiorita, sienita: «granitos» comerciales)** (`granito`) | MEDIDO | Granitos del basamento cristalino: Granito de La Paz, granitos y granitoides transamazónicos (Terreno Piedra Alta), granitoides brasilianos del este, sienitas de Sierra de Ánimas. En Montevideo: el Granito de La Paz (canteras de La Paz y Las Piedras). | La Paz y Las Piedras (Canelones): pedregullo del granito alterado; Maldonado (granito gris); Flores, San José y Colonia (granito negro); Rivera (diorita y granodiorita) | sillería y piedra labrada, adoquines, revestimientos y monumentos (granitos de color), piedra partida y pedregullo para hormigón | 2.63 a 8.18 kt/año (2010–2015) | 20 (Canelones 3, Flores 3, Rivera 3, Rocha 3) | 1726. 1726: Piedra de las canteras del basamento para cimientos y mampostería desde la fundación. [ESTIMADO]; 1870: Adoquines de granito labrados a mano (en Montevideo, por presos) para pavimentar las calles. [DOCUMENTADO]; 1979: Los áridos gruesos («pedregullo») de Montevideo salen casi todos de las canteras al oeste de La Paz, en el granito alterado. [DOCUMENTADO] | medio · mucho (labra a mano) · bajo a medio | sí: Granito de La Paz y granitos transamazónicos al norte del mapa; en la carta 1:500.000 los granitoides de Suárez y transamazónicos. | catastro_2026, miem_2010_2015, dinamige_1987, geologia_montevideo, adoquines_presos, geologia_2017 |
| **Gneis y anfibolita («piedra de Montevideo»)** (`gneis`) | DOCUMENTADO | Gneises, anfibolitas y esquistos de la Formación Montevideo y ortogneises: afloran en la costa (Punta Carretas, Punta Gorda, el Cerro, Punta Yeguas, Pajas Blancas), en la Ciudad Vieja y en la cañada de las Canteras, donde Canteras Montevideo saca la mayor parte de la piedra partida del área metropolitana. | Montevideo: cañada de las Canteras, Cerro, Punta Yeguas, Pajas Blancas, Parque Rodó | mampostería y cimientos, piedra partida para hormigón y bases, escollera | — | — | 1726. 1726: La roca de la costa y del Cerro, la más cercana a la ciudad, para cimientos, muros y fortificaciones. [ESTIMADO]; 2000: Canteras Montevideo, en la cañada de las Canteras, suministra el mayor volumen de piedra partida del área metropolitana. [DOCUMENTADO] | bajo · mucho · bajo | sí: Es la roca de la Ciudad Vieja, de la costa y del Cerro. | geologia_montevideo, geologia_2017, catastro_2026 |
| **Basalto** (`basalto`) | MEDIDO | Coladas de basalto toleítico de la Formación Arapey (Jurásico–Cretácico): el norte y el noroeste del país. | Artigas; Salto; Rivera | piedra partida y balasto en el norte, piedra bruta | 0.1 a 0.15 kt/año (2010–2012) | 4 (Artigas 2, Rivera 1, Salto 1) | 1830 | bajo · mucho · medio | no: No hay basalto cerca de Montevideo. | catastro_2026, miem_2010_2015, geologia_2017 |
| **Mármol** (`marmol`) | MEDIDO | Mármoles calcáreos y dolomíticos asociados al Grupo Lavalleja y al Grupo Barriga Negra (Precámbrico superior). | Lavalleja (Minas); Maldonado (Pan de Azúcar); Treinta y Tres | revestimientos, monolitos, artesanía | 0.03 a 1.83 kt/año (2010–2015) | 5 (Maldonado 4, Lavalleja 1) | 1870 | alto · mucho (oficio de marmolero) · medio | no: Viene de Lavalleja y Maldonado. | dinamige_1987, catastro_2026, miem_2010_2015, geologia_2017 |
| **Piedra laja (filitas y esquistos)** (`piedra_laja`) | MEDIDO | Filitas esquistosas del Grupo Lavalleja: entre Minas y Pan de Azúcar, verdes, rojizas y negras. | Lavalleja (Minas); Maldonado (Pan de Azúcar); Treinta y Tres | revestimientos de paredes y pisos | 2.39 a 6.64 kt/año (2010–2015) | 13 (Lavalleja 7, Treinta y Tres 5, Maldonado 1) | 1900 | medio · mucho · bajo | no: Viene de Lavalleja. | dinamige_1987, catastro_2026, miem_2010_2015 |
| **Arenisca de revestimiento** (`arenisca`) | MEDIDO | Areniscas finas y medias, amarillas a rojizas, de la Formación Tacuarembó. | Tacuarembó; Rivera | revestimiento | 0.67 a 2.7 kt/año (2010–2013) | — | 1900 | medio · mucho · bajo | no: Las areniscas del entorno (Migues, Mercedes, Raigón) son blandas y dan arena y canto, no piedra de revestimiento. | dinamige_1987, miem_2010_2015 |

### Calizas, dolomitas, conchilla y yeso (cal y cemento)

| Recurso | Estado | Dónde (geología) | Departamentos / localidades | Usos y productos | Producción | Títulos (catastro 2026) | Desde | Precio · trabajo · capital | En Montevideo | Fuentes |
|---|---|---|---|---|---|---|---|---|---|---|
| **Caliza (metamórfica y sedimentaria)** (`caliza`) | MEDIDO | Calizas metamórficas lenticulares del Precámbrico (Grupo Lavalleja, Barriga Negra: Minas, Polanco, Treinta y Tres, Cerro Largo) y calizas sedimentarias del tipo Queguay (Paysandú), con lentes en Mercedes y Asencio. | Lavalleja: Minas, Polanco (cal y cemento); Treinta y Tres; Maldonado: Laguna del Sauce; Paysandú: Queguay (cemento ANCAP); Cerro Largo; Rivera | cal viva y apagada (1 t de cal viva pide 2 t de caliza), cemento portland (con arcilla, filita, yeso y hierro), vidrio, corrector de suelos | 1432.38 a 1988.73 kt/año (2010–2015) | 82 (Lavalleja 23, Treinta y Tres 22, Maldonado 13, Paysandú 13) | 1745. 1745: Caleras jesuitas en Colonia: la estancia de la Calera de las Huérfanas (obras de 1745–1750) y los hornos de cal de Belén. [DOCUMENTADO]; 1750: La producción de cal se intensifica desde la segunda mitad del siglo XVIII y tiene su apogeo en el XIX, en muchos pequeños hornos. [DOCUMENTADO]; 1919: Cemento portland nacional con la marca «Artigas» (Compañía Uruguaya de Cemento Portland), después de la fábrica de Metzen-Vincenti. [DOCUMENTADO]; 1980: 48 hornos de cal (33 en marcha) producen unas 8.800 t por mes; cerca del 90 % en Minas y alrededores. El cemento se hace junto a Minas, Paysandú y Montevideo. [DOCUMENTADO] | bajo (la piedra); medio (la cal) · medio; los hornos piden leña · medio (calera) a muy alto (cementera) | no: No hay caliza en la ventana del mapa ni en su entorno (ninguna unidad calcárea en la carta geológica; ningún título de caliza en Montevideo). La cal se trae por agua de Colonia o por tierra de Minas. | catastro_2026, miem_2010_2015, dinamige_1987, fing_cemento, fhce_cal, fing_calera, geologia_2017 |
| **Dolomita** (`dolomita`) | MEDIDO | Dolomías del Grupo Lavalleja. | Maldonado; Lavalleja | vidrio, cal dolomítica, corrector de suelos | 17.18 a 23.01 kt/año (2010–2015) | 23 (Maldonado 11, Lavalleja 10, Treinta y Tres 3) | 1900 | bajo · poco · medio | no: No hay. | catastro_2026, miem_2010_2015, dinamige_1987 |
| **Conchilla (bancos de conchas)** (`conchilla`) | DOCUMENTADO | Bancos arenosos con conchas de la Formación Villa Soriano (Holoceno, transgresión marina), de unos 2 m de potencia; lumaquelas en la Formación Camacho. | Villa Soriano (Soriano); Piriápolis (Maldonado) | fuente de calcio para raciones balanceadas (se tamiza), cal de conchas (diseño: cal más pobre que la de piedra) | 2.27 a 19.38 kt/año (1977–1981) | — | 1726. 1726: Cal de conchilla en hornos chicos junto a la costa: posible, pero no la verifiqué para Montevideo. [ESTIMADO] | bajo · poco · bajo | sí: La Formación Villa Soriano aflora en la desembocadura del Santa Lucía y cerca del Parque Lecocq (memoria de la carta de Montevideo); que tenga conchilla explotable ahí es estimación. | dinamige_1987, geologia_montevideo, geologia_2017 |
| **Yeso** (`yeso`) | DOCUMENTADO | Rosetas y cristales sueltos (18–20 %) en sedimentos cuaternarios, mantos de 2–2,5 m; ocurrencias en el Devónico al norte de Blanquillo. | Bellaco (Río Negro): la única explotación | retardador del cemento portland, construcción, cerámica | — | 1 (Durazno 1) | 1900. 1980: La mayor parte del yeso para cemento se importa. [DOCUMENTADO] | medio · poco · bajo | no: No hay: se importa. | dinamige_1987, catastro_2026 |

### Áridos: arena, balasto, canto rodado, tosca

| Recurso | Estado | Dónde (geología) | Departamentos / localidades | Usos y productos | Producción | Títulos (catastro 2026) | Desde | Precio · trabajo · capital | En Montevideo | Fuentes |
|---|---|---|---|---|---|---|---|---|---|---|
| **Tosca (limo calcáreo)** (`tosca`) | MEDIDO | Limos y lodolitas con carbonato de calcio de las formaciones Libertad (con «presencia constante de carbonato de calcio») y Fray Bentos (la «tosca parda» de Frenguelli). | Durazno; Río Negro; Paysandú; Soriano; Tacuarembó; Florida; Canelones; Montevideo (oeste) | base y sub-base de caminos, rellenos, terraplenes | 1074.71 a 2470.22 kt/año (2010–2015) | 761 (Durazno 102, Río Negro 92, Paysandú 90, Soriano 69) | 1726 | muy bajo · poco · bajo | sí: En los limos de la Formación Libertad, al norte y al oeste: hay títulos de tosca y balasto en Montevideo y Canelones. | catastro_2026, miem_2010_2015, geologia_montevideo, geologia_2017 |
| **Arena (de río, de playa y de médano)** (`arena`) | MEDIDO | Arenas litorales, fluviales y eólicas del Holoceno (playas y médanos de la costa sur y este), Formación Chuy, arenas del río Santa Lucía y del Río Uruguay. | San José (río Santa Lucía, Kiyú); Canelones (El Pinar: arena para vidrio); Montevideo (Carrasco: 80–90 % de la arena de la ciudad en 1979); Maldonado; Colonia (Conchillas); Salto | morteros y hormigón, vidrio (arenas cuarzosas de médano), moldes de fundición, exportación a Buenos Aires | 1719.05 a 2426.88 kt/año (2010–2015) | 74 (San José 24, Maldonado 14, Canelones 10, Colonia 9) | 1726. 1726: Arena de las playas y médanos para los morteros de cal. [ESTIMADO]; 1887: En Conchillas (Colonia) la empresa de C. H. Walker saca arena y piedra para el puerto de Buenos Aires (obras de 1887 a 1897). [DOCUMENTADO]; 1977: Se exportan a la Argentina entre 0,6 y 1 millón de toneladas de arena por año (1977–1981). [DOCUMENTADO] | muy bajo · poco · bajo | sí: Playas, médanos de Carrasco y del oeste de Pajas Blancas, bancos del Santa Lucía. | catastro_2026, miem_2010_2015, dinamige_1987, geologia_montevideo, conchillas |
| **Balasto (granito alterado y gravas arcillosas)** (`balasto`) | MEDIDO | La zona alterada de los granitos (el Granito de La Paz, fracturado por la tectónica cretácica, se vuelve un material «tipo balasto») y las areniscas conglomerádicas. | Canelones (La Paz, Las Piedras); Colonia; Rocha; Montevideo; Maldonado; Florida | caminos, base de pavimentos, hormigón pobre | 3038.25 a 5919.59 kt/año (2010–2015) | 163 (Canelones 53, Colonia 28, Rocha 21, Maldonado 12) | 1726 | muy bajo · poco · bajo | sí: Sobre el basamento con suelo encima y en las areniscas del norte; hay títulos de balasto en Montevideo y Canelones. | catastro_2026, miem_2010_2015, dinamige_1987, geologia_montevideo |
| **Canto rodado y grava** (`canto_rodado`) | MEDIDO | Cantos de los cauces (el Río Uruguay, dragado) y de las unidades con conglomerados. | Salto y Paysandú (Río Uruguay); Maldonado; Rocha | hormigón, empedrado de canto, exportación a la Argentina | 43.97 a 110.4 kt/año (2010–2015) | 20 (Maldonado 7, Salto 6, Paysandú 4, Rocha 2) | 1726. 1977: Producto casi exclusivamente de exportación a la Argentina. [DOCUMENTADO] | bajo · poco · bajo | sí: En el lecho de los arroyos que bajan del basamento (estimado). | catastro_2026, miem_2010_2015, dinamige_1987 |

### Arcillas: cerámica roja, caolín, bentonita

| Recurso | Estado | Dónde (geología) | Departamentos / localidades | Usos y productos | Producción | Títulos (catastro 2026) | Desde | Precio · trabajo · capital | En Montevideo | Fuentes |
|---|---|---|---|---|---|---|---|---|---|---|
| **Arcilla roja y limo para ladrillos (lodolitas)** (`arcilla_roja`) | DOCUMENTADO | Lodolitas débilmente consolidadas de las formaciones Libertad y Dolores (Cuaternario): limo con arcilla illítica (San José: 26 % arena, 39 % limo, 35 % arcilla). Se explotan a cielo abierto, 2–3 m de manto. | Montevideo (60–75 % de la producción, 1977–1981); Maldonado (~17 %); San José (3–7 %); todo el país: hornos de campo | ladrillos, ticholos, baldosas, adobe (el barro crudo) | 285 a 350 kt/año (1977–1981) | 10 (Cerro Largo 5, Canelones 3, Durazno 1, San José 1) | 1726. 1726: Hornos de ladrillo en el ejido con el barro de los limos y los bajos. [ESTIMADO]; 1980: Se censaron cerca de 400 hornos de ladrillos de campo, que usan suelo como materia prima; las ladrilleras mecanizadas usan lodolitas de Libertad y Dolores. [DOCUMENTADO] | bajo · mucho · bajo | sí: Es el material más común del mapa: las lomadas de la Formación Libertad. | dinamige_1987, catastro_2026, miem_2010_2015, geologia_2017 |
| **Arcilla gris de bañado y aluvión (barro de tejas y loza basta)** (`arcilla_gris`) | ESTIMADO | Sedimentos limo-arcillosos grises de los aluviones («Actual») y las arcillas grises de Vizcaíno (Formación Villa Soriano), con materia orgánica, en los bajos de los arroyos por debajo de la cota 10. | bajos de todos los arroyos; bañados del Santa Lucía y de Carrasco | tejas, alfarería, ladrillo de segunda | — | — | 1726. 1726: Barro fino de los bajos para tejas y loza de uso diario: estimación de diseño (las tejas de calidad las hacía la cerámica con arcillas caoliníticas). [ESTIMADO] | bajo · mucho · bajo | sí: En los bajos del Miguelete, el Pantanoso, el Carrasco y el Santa Lucía. | geologia_2017, geologia_montevideo, diseno |
| **Arcilla caolinítica (caolín)** (`caolin`) | DOCUMENTADO | Lentes y niveles de arcillas caoliníticas de las formaciones devónicas Cordobés y Cerrezuelo (Durazno). | Blanquillo (Durazno) | loza de mesa y sanitaria, azulejos, porcelana, refractarios, tejas y baldosas de cerámica roja | — | 4 (Durazno 3, Cerro Largo 1) | 1900. 1977: Se importa caolín de primera calidad por unos 0,4 a 0,8 millones de dólares por año (1977–1981). [DOCUMENTADO] | medio · medio · medio | no: Viene de Durazno; las fábricas de loza estaban en Montevideo. | dinamige_1987, catastro_2026, geologia_2017 |
| **Bentonita** (`bentonita`) | MEDIDO | Lentes de bentonita rosada y negra (hasta 5,5 m) en areniscas de la Formación Yaguarí (Pérmico). | Bañado de Medina, al oeste de Melo (Cerro Largo) | tierra decolorante (aceites y grasas), dispersante, pinturas | 0.43 a 8.99 kt/año (2010–2015) | 9 (Cerro Largo 9) | 1930 | medio · poco · medio | no: No hay. | dinamige_1987, catastro_2026, miem_2010_2015 |

### Minerales industriales

| Recurso | Estado | Dónde (geología) | Departamentos / localidades | Usos y productos | Producción | Títulos (catastro 2026) | Desde | Precio · trabajo · capital | En Montevideo | Fuentes |
|---|---|---|---|---|---|---|---|---|---|---|
| **Feldespato y cuarzo (pegmatitas y filones)** (`feldespato_cuarzo`) | MEDIDO | Pegmatitas y filones de cuarzo blanco (99 % de sílice) del basamento cristalino. | sur de Florida (Cerro Pelado); el este del país, a 100–150 km de Montevideo | vidrio, cerámica, detergentes | 0.004 a 0.3 kt/año (2013–2015) | 8 (Rocha 2, Florida 1, Lavalleja 1, Maldonado 1) | 1900 | medio · poco · bajo | no: Pegmatitas chicas en el basamento, sin explotación conocida en el mapa. | dinamige_1987, catastro_2026, miem_2010_2015 |
| **Talco** (`talco`) | MEDIDO | Talco de metamorfismo de dolomitas (Arroyo Miguelete, Colonia: mina en galerías) y de talquificación de ultrabásicas (Arroyo Tapes, Lavalleja); indicio en Mal Abrigo (San José). | Arroyo Miguelete (Colonia); Arroyo Tapes (Lavalleja); Mal Abrigo (San José) | farmacia y cosmética, papel, cerámica, pinturas | 0.36 a 0.83 kt/año (2010–2015) | 2 (Lavalleja 1, Rivera 1) | 1930 | medio · medio (galerías) · medio | no: No hay. | dinamige_1987, catastro_2026, miem_2010_2015 |
| **Corindón** (`corindon`) | DOCUMENTADO | Corneanas con corindón (68,5 % de alúmina) entre cuarcitas, esquistos y dolomitas del Grupo Lavalleja: el único yacimiento del país. | 6 km al sur de Minas por la ruta 12 (Lavalleja) | abrasivos, refractarios | 0.139 a 0.421 kt/año (1977–1981) | 1 (Lavalleja 1) | 1918 | alto · poco · medio | no: No hay. | dinamige_1987, catastro_2026 |
| **Arenas negras (ilmenita, magnetita, rutilo, zircón, monacita)** (`arenas_negras`) | DOCUMENTADO | Niveles de minerales densos en arenas cuarzosas litorales de la costa este. | costas de Canelones, Maldonado y Rocha | blanco de titanio (pigmento), zircón | — | 1 (Lavalleja 1) | 1955 | alto · poco · alto | no: En las playas de Montevideo hay solo estratos milimétricos oscuros de minerales densos. | dinamige_1987 |
| **Grafito** (`grafito`) | DOCUMENTADO | Hojas, copos y venillas en micaesquistos y gneises precámbricos (hasta 16 % de carbono). | alrededores de Soca (Canelones) | lubricantes, crisoles, lápices | — | 1 (San José 1) | 1900 | medio · poco · bajo | no: Los gneises de Montevideo tienen grafito disperso (para-gneises grafitosos), sin valor de mina. | dinamige_1987, catastro_2026 |
| **Fluorita y baritina** (`fluorita_baritina`) | DOCUMENTADO | Baritina en filones de cuarzo del Precámbrico superior (Minas); fluorita: un solo yacimiento conocido. | alrededores de Minas (Lavalleja) | vidrio, cerámica, pinturas, metalurgia | — | 1 (Lavalleja 1) | 1930. 1980: Las industrias del vidrio, la cerámica y las pinturas importan 20–50 t de fluorita por año. [DOCUMENTADO] | medio · poco · bajo | no: No hay. | dinamige_1987, catastro_2026 |
| **Fosfatos (apatita)** (`fosfatos`) | DOCUMENTADO | Rocas a cuarzo-apatito en metamorfitos precámbricos de La Calera (norte de Minas); indicios en pórfidos. | La Calera, al norte de Minas (Lavalleja); Rivera, Treinta y Tres, Colonia (prospección) | fertilizantes | — | 25 (Lavalleja 10, Rivera 7, Colonia 3, Treinta y Tres 3) | 1990 | medio · poco · alto | no: No hay. | dinamige_1987, catastro_2026 |

### Piedras semipreciosas

| Recurso | Estado | Dónde (geología) | Departamentos / localidades | Usos y productos | Producción | Títulos (catastro 2026) | Desde | Precio · trabajo · capital | En Montevideo | Fuentes |
|---|---|---|---|---|---|---|---|---|---|---|
| **Amatista (y geodas de cuarzo)** (`amatista`) | MEDIDO | Geodas en las coladas de basalto de la Formación Arapey: el Distrito Gemológico Los Catalanes (Artigas). | Los Catalanes (Artigas); Salto (pocas) | geodas y drusas de colección y decoración, piedras talladas para joyería, artesanías | 14.69 a 60.46 kt/año (2010–2015) | 204 (Artigas 196, Salto 5) | 1870. 1870: Explotación en pequeñas empresas familiares en el norte: no encontré la fecha de inicio verificable. [ESTIMADO] | muy alto · mucho (galerías a pico y clasificación a mano) · bajo a medio | no: Solo en el basalto del norte. | catastro_2026, regiones_mineras, miem_2010_2015, catalanes, amatista_nacional, dinamige_1987, geologia_2017 |
| **Ágata (y cornalina, calcedonia, jaspe, ópalo)** (`agata`) | MEDIDO | Ágatas y calcedonias rellenando vesículas y geodas del basalto Arapey y de las areniscas intertrápicas. | Artigas; Salto | piedras talladas y pulidas, artesanías, objetos decorativos | 0.02 a 0.02 kt/año (2011–2014) | 204 (Artigas 194, Salto 7) | 1870 | alto · mucho · bajo | no: Solo en el norte. | catastro_2026, regiones_mineras, miem_2010_2015, dinamige_1987 |
| **Granate** (`granate`) | DOCUMENTADO | En gneises y esquistos granatíferos del basamento. | Rivera (títulos); Montevideo: gneises granatíferos de la costa (sin valor de mina) | abrasivo, gema menor | — | 2 (Rivera 2) | 1990 | medio · poco · bajo | no: Hay granate en los gneises de la Formación Montevideo, en cristales chicos. | catastro_2026, geologia_montevideo |

### Metales

| Recurso | Estado | Dónde (geología) | Departamentos / localidades | Usos y productos | Producción | Títulos (catastro 2026) | Desde | Precio · trabajo · capital | En Montevideo | Fuentes |
|---|---|---|---|---|---|---|---|---|---|---|
| **Oro (y plata asociada)** (`oro`) | MEDIDO | Oro en filones y zonas de cizalla del basamento de Rivera (Isla Cristalina de Rivera) y del Grupo Lavalleja. | Minas de Corrales (Rivera); Durazno, Lavalleja, Treinta y Tres, San José y Florida (exploración) | doré (oro y plata en barras) para exportar | 0.001 a 0.004 kt/año (2011–2015) | 165 (Rivera 80, Lavalleja 27, Durazno 26, Treinta y Tres 12) | 1878. 1878: La Compañía Francesa de Minas de Oro del Uruguay prospecta en Cuñapirú y nace Minas de Corrales; levanta la represa de Cuñapirú (1879–1882). [DOCUMENTADO]; 1997: Se retoma la minería de oro a gran escala en Minas de Corrales. [DOCUMENTADO] | muy alto · medio · muy alto | no: No hay. | catastro_2026, miem_2010_2015, corrales |
| **Hierro (magnetita y hematita)** (`hierro`) | MEDIDO | Cuarcitas magnetíticas (formaciones de hierro bandeado) del Complejo Granulítico Valentines-Rivera; cuarcitas ferríferas del Precámbrico superior en Lavalleja (hematita para el cemento). | Valentines y Cerro Chato (Florida, Durazno y Treinta y Tres); Rivera; Lavalleja (hematita para portland) | aditivo del cemento portland, mineral de hierro para exportar (proyecto Aratirí, no concretado) | 9.5 a 16.8 kt/año (2010–2015) | 146 (Rivera 84, Durazno 25, Lavalleja 18, Treinta y Tres 11) | 1930. 2010: El proyecto Aratirí (Zamin Ferrous) planea sacar 18 millones de toneladas por año en Valentines y un mineroducto a Rocha; no se concreta. [DOCUMENTADO] | bajo · poco · muy alto | no: No hay. | catastro_2026, miem_2010_2015, valentines, dinamige_1987, geologia_2017 |
| **Manganeso** (`manganeso`) | MEDIDO | Títulos de prospección y exploración en el basamento. | Rivera; Lavalleja; Treinta y Tres | siderurgia | — | 44 (Rivera 23, Lavalleja 11, Treinta y Tres 4, Colonia 3) | 1990 | medio · poco · alto | no: No hay. | catastro_2026 |
| **Cobre, plomo y zinc** (`metales_base`) | MEDIDO | Títulos de prospección y exploración (sin producción en la estadística 2010–2015). | Rivera; Lavalleja; Durazno; Treinta y Tres; Florida; San José | metalurgia | — | 111 (Rivera 35, Durazno 24, Lavalleja 24, Treinta y Tres 11) | 1990 | alto · poco · muy alto | no: No hay. | catastro_2026 |
| **Níquel, cromo, cobalto, platinoides, tierras raras, litio, diamantes (prospección)** (`metales_estrategicos`) | MEDIDO | Solo prospección y exploración: ultrabásicas y metagabros del basamento (níquel, cromo, platinoides), pegmatitas (litio, tantalio), kimberlitas buscadas (diamantes). | Durazno; Lavalleja; Treinta y Tres; Rivera; Florida; Colonia | ninguno todavía: es búsqueda | — | 95 (Rivera 32, Durazno 23, Lavalleja 18, Treinta y Tres 11) | 2000 | muy alto · poco · muy alto | no: No hay. | catastro_2026 |

### Combustibles fósiles y turba

| Recurso | Estado | Dónde (geología) | Departamentos / localidades | Usos y productos | Producción | Títulos (catastro 2026) | Desde | Precio · trabajo · capital | En Montevideo | Fuentes |
|---|---|---|---|---|---|---|---|---|---|---|
| **Turba** (`turba`) | DOCUMENTADO | Depósitos de turba en los sedimentos actuales de los bañados (la unidad «Actual» de la carta los nombra); turberas activas de tiririca en los bañados de Santa Teresa y Laguna Negra. | Laguna Negra y bañados de Santa Teresa (Rocha); Litoral costero | horticultura (sustrato), combustible pobre (diseño) | — | — | 1726 | bajo · mucho · bajo | sí: Estimada en los bañados del mapa (Carrasco, Pantanoso, Santa Lucía): la carta dice turba en los sedimentos actuales, sin ubicarla. | turberas, dinamige_1987, geologia_2017 |
| **Lutitas pirobituminosas** (`lutitas_bituminosas`) | DOCUMENTADO | Lutitas pirobituminosas de la Formación Mangrullo (Pérmico, cuenca norte). | Cerro Largo; Tacuarembó | aceite de esquisto (solo estudios) | — | — | 1955 | medio · poco · muy alto | no: No hay. | geologia_2017, dinamige_1987 |
| **Carbón (lechos carbonosos)** (`carbon`) | DOCUMENTADO | Lechos carbonosos intercalados en las areniscas de la Formación Tres Islas (Pérmico): sin yacimiento explotado. | Cerro Largo | ninguno conocido; el carbón se importa | — | — | 1870 | medio · mucho · alto | no: No hay: el carbón de piedra se importa. | geologia_2017 |

### Sal

| Recurso | Estado | Dónde (geología) | Departamentos / localidades | Usos y productos | Producción | Títulos (catastro 2026) | Desde | Precio · trabajo · capital | En Montevideo | Fuentes |
|---|---|---|---|---|---|---|---|---|---|---|
| **Sal** (`sal`) | ESTIMADO | No hay evaporitas ni salinas explotables (la carta de materias primas no registra sal). |  | saladeros, consumo | — | — | 1726. 1726: La sal se trae por barco (el juego ya la cobra como gasto de los saladeros). [ESTIMADO] | medio (importada) · — · — | no: Se importa. | diseno, dinamige_1987 |

### Agricultura

| Recurso | Estado | Dónde (geología) | Departamentos / localidades | Usos y productos | Producción | Títulos (catastro 2026) | Desde | Precio · trabajo · capital | En Montevideo | Fuentes |
|---|---|---|---|---|---|---|---|---|---|---|
| **Cultivos de secano (trigo, maíz, cebada, soja)** (`cultivos_secano`) | DOCUMENTADO | Suelos profundos y fértiles (grupos CONEAT 10 y 11, índice de productividad alto) sobre los limos de Fray Bentos y Libertad: el litoral oeste y el sur. | Soriano; Río Negro; Paysandú; Colonia; San José; Canelones | harina y pan, forraje, exportación de granos | — | — | 1726. 1726: Las chacras de trigo y maíz del ejido de Montevideo (capa Tierra para cultivo). [ESTIMADO] | bajo · mucho (hasta la mecanización) · bajo | sí: Las lomadas de la Formación Libertad (grupos 10.x, IP medio 146) son de los mejores suelos del país. | coneat, diea |
| **Arroz** (`arroz`) | DOCUMENTADO | Planicies bajas e inundables de la cuenca de la Laguna Merín (sedimentos de la Formación Chuy y aluviones) y del norte, con riego. | Treinta y Tres; Rocha; Cerro Largo; Artigas | arroz para exportar | — | — | 1930 | medio · medio · alto (riego) | no: No en el mapa. | diea, coneat |
| **Horticultura y fruticultura** (`horticultura_fruticultura`) | DOCUMENTADO | Suelos profundos cerca de los mercados: el cinturón hortícola de Montevideo y Canelones; citrus y hortalizas de primor en Salto. | Canelones; Montevideo rural; San José; Salto (citrus) | verduras, frutas, citrus | — | — | 1726 | medio · mucho · bajo | sí: Las quintas del ejido (capa Tierra para cultivo). | diea, coneat |
| **Vid y vino** (`vid`) | DOCUMENTADO | Suelos de los limos del sur (Canelones y Montevideo) y del litoral. | Canelones; Montevideo; Salto (el primer Tannat); Colonia | vino (Tannat) | — | — | 1870. 1870: Pascual Harriague planta Tannat en Salto (hacia 1870–1874). [DOCUMENTADO] | medio · mucho · medio | sí: Viñedos en las lomadas del norte desde la inmigración. | tannat, diea |

### Ganadería

| Recurso | Estado | Dónde (geología) | Departamentos / localidades | Usos y productos | Producción | Títulos (catastro 2026) | Desde | Precio · trabajo · capital | En Montevideo | Fuentes |
|---|---|---|---|---|---|---|---|---|---|---|
| **Ganado vacuno sobre campo natural** (`ganado_vacuno`) | DOCUMENTADO | Campo natural en todo el país; los suelos superficiales del basalto y de las sierras del cristalino (IP CONEAT bajo, no aptos para cultivos) son de ganadería extensiva. | todo el país; extensiva en el basalto (norte) y las sierras del este | carne, cueros, sebo, tasajo | — | — | 1726. 1726: Las vaquerías y las estancias del ejido: la base de la economía colonial (ver docs/sistemas/servicios.md). [ESTIMADO] | medio · poco por animal · medio (tierra) | sí: Capa Pasturas: aptitud CONEAT del mapa. | coneat, diea |
| **Ovinos (lana)** (`ovinos`) | DOCUMENTADO | Campo natural del basalto y de las areniscas del norte. | Artigas; Salto; Paysandú; Tacuarembó; Durazno; Flores | lana, carne de cordero | — | — | 1830 | medio · medio (esquila) · medio | no: Fuera del mapa. | diea, coneat |
| **Lechería** (`leche`) | DOCUMENTADO | Suelos profundos de los limos (Libertad, Fray Bentos) cerca de las plantas y de Montevideo: la cuenca lechera del sur. | San José; Florida; Colonia; Canelones | leche, manteca, queso | — | — | 1870 | medio · mucho (dos ordeñes por día) · medio | sí: Tambos en el ejido y en las lomadas del norte (capa Pasturas). | diea, coneat |

### Monte y forestación

| Recurso | Estado | Dónde (geología) | Departamentos / localidades | Usos y productos | Producción | Títulos (catastro 2026) | Desde | Precio · trabajo · capital | En Montevideo | Fuentes |
|---|---|---|---|---|---|---|---|---|---|---|
| **Forestación (eucalipto y pino)** (`forestacion`) | DOCUMENTADO | Suelos de prioridad forestal (Ley 15.939 de 1987): arenosos y de baja productividad ganadera, sobre areniscas cretácicas y de Tacuarembó y las arenas costeras. | Rivera; Tacuarembó; Río Negro; Paysandú; costas arenosas del litoral sur | madera aserrada, celulosa, leña | — | — | 1870. 1987: La Ley Forestal define los suelos de prioridad forestal (hoy unos 3,5 a 4,3 millones de hectáreas). [DOCUMENTADO] | bajo · poco · alto | sí: Pinares y eucaliptos en los médanos de Carrasco y del oeste desde fines del siglo XIX (no en 1726). | forestal |
| **Monte nativo (leña y maderas duras)** (`monte_nativo`) | DOCUMENTADO | Monte ribereño en las orillas de ríos y arroyos y monte de quebrada en las sierras. | orillas del Santa Lucía, el Miguelete, el Pantanoso y el Carrasco | leña para hornos de ladrillo y de cal, madera dura para postes y vigas, carbón vegetal | — | — | 1726. 1726: La leña del monte de los arroyos alimenta los hornos y las cocinas; se agota cerca de la ciudad. [DOCUMENTADO] | bajo · mucho · bajo | sí: Capa de bosque del mapa (bosque.bin). | juego_vegetacion |
| **Paja brava, totora y junco de los bañados** (`paja_junco`) | ESTIMADO | Vegetación de los bañados y bajos inundables. | bañados de Carrasco, del Pantanoso y del Santa Lucía | techos de paja (quincha), esteras | — | — | 1726. 1726: Los primeros ranchos se techan con paja de los bañados. [ESTIMADO] | muy bajo · medio · nulo | sí: En los bañados del mapa (biomas oficiales de la IDE). | diseno, geologia_montevideo |

### Productos

| Producto | Sale de | Desde | Material de construcción | Estado | Fuente |
|---|---|---|---|---|---|
| Piedra bruta (mampostería, cimientos) (`piedra_bruta`) | gneis, granito, basalto | 1726 | piedra | DOCUMENTADO | dinamige_1987 |
| Sillería (granito labrado) (`silleria`) | granito | 1726 | piedra | ESTIMADO | diseno |
| Adoquines de granito (`adoquines`) | granito | 1870 | piedra | DOCUMENTADO | adoquines_presos |
| Placas de revestimiento (granito, mármol, laja, arenisca) (`revestimiento`) | granito, marmol, piedra_laja, arenisca | 1870 | piedra | DOCUMENTADO | dinamige_1987 |
| Piedra partida (triturada) (`piedra_partida`) | gneis, granito, basalto | 1900 | piedra | MEDIDO | miem_2010_2015 |
| Pedregullo y balasto para hormigón y caminos (`pedregullo`) | balasto, granito, canto_rodado | 1726 | — | DOCUMENTADO | dinamige_1987 |
| Tosca y balasto para caminos y rellenos (`relleno`) | tosca, balasto | 1726 | — | MEDIDO | miem_2010_2015 |
| Arena para morteros y hormigón (`arena_construccion`) | arena | 1726 | arena | MEDIDO | miem_2010_2015 |
| Ladrillos (`ladrillos`) | arcilla_roja, monte_nativo | 1726 | ladrillo | DOCUMENTADO | dinamige_1987 |
| Adobe (barro crudo con paja) (`adobe`) | arcilla_roja, paja_junco | 1726 | adobe | ESTIMADO | diseno |
| Ticholos (ladrillo hueco) (`ticholos`) | arcilla_roja | 1930 | ladrillo | DOCUMENTADO | dinamige_1987 |
| Baldosas de cerámica roja (`baldosas`) | arcilla_roja, caolin | 1870 | ladrillo | DOCUMENTADO | dinamige_1987 |
| Tejas (`tejas`) | arcilla_gris, caolin | 1726 | teja | DOCUMENTADO | dinamige_1987 |
| Loza basta y alfarería (`loza_basta`) | arcilla_gris | 1726 | — | ESTIMADO | diseno |
| Loza, azulejos y porcelana (`loza_porcelana`) | caolin, feldespato_cuarzo | 1900 | — | DOCUMENTADO | dinamige_1987 |
| Ladrillos refractarios (`refractarios`) | caolin | 1900 | — | DOCUMENTADO | dinamige_1987 |
| Cal viva (horno de cal) (`cal_viva`) | caliza, conchilla, monte_nativo | 1745 | cal | DOCUMENTADO | dinamige_1987 |
| Cal apagada (morteros y revoques) (`cal_apagada`) | cal_viva | 1745 | cal | DOCUMENTADO | fhce_cal |
| Cemento portland (`cemento_portland`) | caliza, arcilla_roja, piedra_laja, yeso, hierro | 1919 | cal | DOCUMENTADO | fing_cemento |
| Vidrio (envases, plano, ornamental) (`vidrio`) | arena, feldespato_cuarzo, dolomita, caliza, fluorita_baritina | 1900 | — | DOCUMENTADO | dinamige_1987 |
| Piedras talladas, geodas y artesanías (`joyeria_artesania`) | amatista, agata | 1870 | — | MEDIDO | miem_2010_2015 |
| Tierra decolorante activada (`tierra_decolorante`) | bentonita | 1930 | — | DOCUMENTADO | dinamige_1987 |
| Calcio para raciones balanceadas (`alimento_balanceado`) | conchilla | 1950 | — | DOCUMENTADO | dinamige_1987 |
| Doré (oro y plata) (`oro_dore`) | oro | 1878 | — | MEDIDO | miem_2010_2015 |
| Sustrato de turba para huertas (`sustrato`) | turba | 1950 | — | DOCUMENTADO | dinamige_1987 |
| Carne (`carne`) | ganado_vacuno | 1726 | — | DOCUMENTADO | diea |
| Cueros (`cueros`) | ganado_vacuno | 1726 | cuero | DOCUMENTADO | diea |
| Sebo (`sebo`) | ganado_vacuno | 1726 | — | DOCUMENTADO | diea |
| Tasajo (`tasajo`) | ganado_vacuno, sal | 1780 | — | ESTIMADO | diseno |
| Lana (`lana`) | ovinos | 1830 | — | DOCUMENTADO | diea |
| Leche y lácteos (`leche`) | leche | 1870 | — | DOCUMENTADO | diea |
| Trigo y harina (`trigo`) | cultivos_secano | 1726 | — | DOCUMENTADO | diea |
| Arroz (`arroz`) | arroz | 1930 | — | DOCUMENTADO | diea |
| Frutas y verduras (`frutas_verduras`) | horticultura_fruticultura | 1726 | — | DOCUMENTADO | diea |
| Vino (`vino`) | vid | 1870 | — | DOCUMENTADO | tannat |
| Madera (`madera`) | forestacion, monte_nativo | 1726 | madera | DOCUMENTADO | forestal |
| Celulosa (`celulosa`) | forestacion | 2007 | — | ESTIMADO | diseno |
| Leña y carbón vegetal (`lena`) | monte_nativo, forestacion, turba | 1726 | — | DOCUMENTADO | juego_vegetacion |
| Paja para techos (`paja_techo`) | paja_junco, cultivos_secano | 1726 | paja | ESTIMADO | diseno |

### Fuentes del catálogo

| Clave | Fuente | Tipo |
|---|---|---|
| `catastro_2026` | DINAMIGE (MIEM): Consulta Minera 2026, títulos mineros con sus sustancias (campo MINERALES), capas 2 a 5 — https://geoportal.miem.gub.uy/arcgis1091/rest/services/Dinamige_ConsMin/Consulta_Minera_2026/MapServer | servicio |
| `geologia_2017` | DINAMIGE (MIEM): Mapa base de unidades geológicas 2017 (escala 1:500.000), descripción litológica de cada unidad — https://geoportal.miem.gub.uy/arcgis1091/rest/services/Dinamige_GeoS/Mapa_Base_Unidades_Geologicas_2017_GeoS/MapServer/2 | servicio |
| `regiones_mineras` | DINAMIGE (MIEM): Regiones mineras (GeologiaGeoS, capa 0): Distrito Gemológico Los Catalanes, «ágatas y amatistas» — https://geoportal.miem.gub.uy/arcgis1091/rest/services/Dinamige_GeoS/GeologiaGeoS/MapServer/0 | servicio |
| `dinamige_1987` | Coronel, N. et al. (1987): Memoria explicativa de la Carta de Materias Primas Minerales No Metálicas del Uruguay, 1:1.000.000. DINAMIGE–BGR — https://www.gub.uy/ministerio-industria-energia-mineria/sites/ministerio-industria-energia-mineria/files/documentos/publicaciones/Memoria%20de%20Carta%20Materias%20Primas%20No%20Met%C3%A1licas.pdf | publicacion |
| `miem_2010_2015` | MIEM–DINAMIGE: Producción nacional de minerales por año según mineral, 2010–2015 (miles de toneladas; 2015 preliminar) — https://www.gub.uy/ministerio-industria-energia-mineria/sites/ministerio-industria-energia-mineria/files/documentos/publicaciones/estadistica_produccion_mineral_anos_2010_al_2015.pdf | estadistica |
| `miem_2002` | MIEM–DINAMIGE: Estadística de producción minera 2002 (por departamento y mineral) — https://www.gub.uy/ministerio-industria-energia-mineria/sites/ministerio-industria-energia-mineria/files/documentos/publicaciones/estadistica_produccion_minera_2002.pdf | estadistica |
| `geologia_montevideo` | Memoria de la carta geológica de Montevideo (publicada por el MGAP): gneises de la Formación Montevideo, Granito de La Paz alterado a balasto, Villa Soriano, dunas y arenas costeras — https://www.gub.uy/ministerio-ganaderia-agricultura-pesca/sites/ministerio-ganaderia-agricultura-pesca/files/2020-02/Memoria%20carta%20Geologica%20Montevideo_0.pdf | publicacion |
| `fing_calera` | Instituto de Ensayo de Materiales (FING-UdelaR): ficha de la Calera de las Huérfanas (construcción 1745–1750, Monumento Histórico Nacional desde 1976) — https://www.fing.edu.uy/sites/default/files/2024-11/Calera%20de%20las%20huerfanas.docx%20%281%29_0.pdf | publicacion |
| `fhce_cal` | Revista de Arqueología (FHCE-UdelaR): la producción de cal en el territorio uruguayo se intensifica desde la segunda mitad del siglo XVIII, con apogeo en el XIX; caleras jesuitas en Colonia — https://ojs.fhce.edu.uy/index.php/aarq/article/download/2077/2689/7869 | publicacion |
| `fing_cemento` | FING-UdelaR (biblioteca): en 1919 Metzen-Vincenti y Cía. venden la fábrica de portland a la International Portland Cement Corporation; desde el 1/7/1919 se vende con la marca «Artigas» (Compañía Uruguaya de Cemento Portland) — https://www.fing.edu.uy/biblioteca/bibliografias/431514.pdf | publicacion |
| `corrales` | Minas de Corrales: la Compañía Francesa de Minas de Oro del Uruguay prospecta desde 1878; represa de Cuñapirú (1879–1882); retoma en 1996–1997 (fuentes secundarias) — https://en.wikipedia.org/wiki/Minas_de_Corrales | secundaria |
| `valentines` | MIEM (2016), monitoreo zona Valentines: Formación Valentines (Bossi 1966) con cuarcitas a magnetita; El Observador: repaso del proyecto Aratirí (18 Mt/año previstos, no concretado) — https://www.gub.uy/ministerio-industria-energia-mineria/sites/ministerio-industria-energia-mineria/files/documentos/publicaciones/monitoreo_2016_zfv_-_miem_web_0.pdf | publicacion |
| `conchillas` | Revista de Antropología (FHCE-UdelaR): Conchillas (Colonia) nace de la extracción de áridos, arena y piedra triturada para el puerto de Buenos Aires (obras 1887–1897, C. H. Walker & Co.) — https://ojs.fhce.edu.uy/index.php/revantroetno/article/view/31/35 | publicacion |
| `adoquines_presos` | Jardín Botánico de Montevideo (IM): los adoquines de granito antiguos los labraban los presos y con ellos se pavimentaron muchas calles — https://jardinbotanico.montevideo.gub.uy/sites/jardinbotanico.montevideo.gub.uy/files/articulos/descargas/adoquines.pdf | publicacion |
| `turberas` | Ministerio de Ambiente: turberas de Laguna Negra y bañados de Santa Teresa (Rocha), turbera activa — https://www.gub.uy/ministerio-ambiente/sites/ministerio-ambiente/files/documentos/noticias/Producto%202%20-%20Turberas%20Laguna%20Negra%20-%20FINAL.pdf | publicacion |
| `catalanes` | IUGS Geoheritage: yacimientos de amatista del Distrito Gemológico Los Catalanes (Artigas) — https://iugs-geoheritage.org/geoheritage_sites/deposits-of-amethysts-of-los-catalanes-gemological-district/ | publicacion |
| `amatista_nacional` | MIEM: la amatista es la piedra nacional — https://www.gub.uy/ministerio-industria-energia-mineria/comunicacion/noticias/miem-celebro-amatista-es-piedra-nacional | publicacion |
| `forestal` | Ley 15.939 (1987) y Decreto 452/988: suelos de prioridad forestal; MGAP: Los suelos de prioridad forestal y su productividad — https://www.gub.uy/ministerio-ganaderia-agricultura-pesca/sites/ministerio-ganaderia-agricultura-pesca/files/documentos/publicaciones/los_suelos_de_prioridad_forestal_y_su_productividad.pdf | publicacion |
| `diea` | MGAP–DIEA: Anuario Estadístico Agropecuario (regiones agropecuarias, lechería, arroz) — https://www.gub.uy/ministerio-ganaderia-agricultura-pesca/tematica/diea | estadistica |
| `tannat` | Uruguay XXI: el Tannat, introducido hacia 1870 por Pascual Harriague en Salto — https://www.uruguayxxi.gub.uy/en/news/article/tannat-the-grape-variety-that-has-found-its-finest-expression-in-uruguay/ | publicacion |
| `coneat` | MGAP–RENARE: grupos CONEAT, índice de productividad y aptitud (servicios de dgrn.mgap.gub.uy) — https://dgrn.mgap.gub.uy/arcgis/rest/services/CONEAT/ConsultaConeat/MapServer/0 | servicio |
| `juego_vegetacion` | City Sim 2027: reconstrucción del monte nativo de 1726 (docs/sistemas/vegetacion-historica.md) — [docs/sistemas/vegetacion-historica.md](../../docs/sistemas/vegetacion-historica.md) | interna |
| `diseno` | Estimación de diseño del juego (la base se dice en cada entrada) — docs/sistemas/recursos-de-uruguay.md | estimacion |

## Pendiente

- Enchufar el sistema de materiales de construcción (otro agente): consumir piedra, ladrillo, teja,
  cal, arena, madera, adobe y paja por edificio con `construccion_cerca` y `se_saca_del_suelo`.
- Las cifras de la DIEA por región agropecuaria (lechería, arroz, cultivos de secano) y la serie de
  producción minera posterior a 2015.
- La fecha de inicio de la explotación de amatista y ágata en Artigas; de dónde venía la cal de la
  Montevideo colonial (Colonia, Minas o conchilla); las primeras caleras de Minas.
- Una carta geológica más detallada (1:100.000) y la geoquímica de las muestras de DINAMIGE
  (`Hosted/analisis_geoquimicos`) para afinar las leyes.
- El mapa del país: leer `assets/datos/uruguay/` en el motor (hoy solo lo leen las pruebas).
