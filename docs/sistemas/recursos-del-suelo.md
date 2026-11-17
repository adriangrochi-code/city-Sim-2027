# Recursos del suelo, aptitud de la tierra y externalidades de las zonas

> Documento de sistema. Estado: **en el motor** (`engine/src/city/recursos/`), con datos reales
> del mapa de Montevideo (`assets/mapas/montevideo/{suelos,productividad,geologia,materiales}*.bin`
> y `suelos.json`). Las épocas siguientes y las industrias de exportación lo usan por su API.

## Qué resuelve

No es lo mismo poner una estancia sobre las lomadas de limo del norte que sobre las colinas de
roca de la costa, ni un horno de ladrillos lejos del barro, ni una curtiembre aguas arriba de un
barrio. El sistema da:

1. **Mapas de calidad de la tierra** (cultivo y pasturas) a partir del índice de productividad
   CONEAT, la aptitud de cada grupo de suelos, la pendiente y los bañados.
2. **Mapas de materiales con tipo**: granito, gneis, balasto, canto rodado, arena, arcilla roja y
   gris, tosca, conchilla, caliza y turba, a partir de la carta geológica de DINAMIGE, los suelos
   CONEAT, la costa, los arroyos y la minería conocida con su sustancia. El catálogo de todos los
   recursos del país (y lo que se produce con ellos) está en
   [recursos-de-uruguay.md](recursos-de-uruguay.md).
3. **Rinde de cada sitio**: chacras, estancias, hornos y canteras producen según su lugar; una
   cantera sin piedra no se puede poner (y no rendiría nada), un horno sin arcilla tampoco.
4. **Molestias** (olor, ruido, humo y agua sucia) de cada industria y servicio, con alcance; el
   agua sucia de curtiembres y saladeros **baja por el arroyo**.
5. **Valor del suelo** de cada solar (acceso, servicios, costa, molestias, recursos) que decide qué
   solares crecen primero, cuánto se paga de alquiler y cómo viven los vecinos.

## Datos: qué es real y qué es estimación

Todo se consultó el **17/11/2026** desde el entorno de desarrollo; las respuestas quedan en la
caché del constructor (`--cache`), así el paso se puede repetir sin red.

### Verificado (servicios oficiales)

| Dato | Fuente (consulta que funcionó) | Archivo |
|---|---|---|
| Grupos CONEAT y aptitud general (`SC`, `APTITUD`) | SNIG `MapasBase/Renare_Coneat/MapServer/0` (MGAP-RENARE). Sus campos, según `…/MapServer/0?f=json`, son solo `OBJECTID`, `SC`, `APTITUD`: **no trae el índice de productividad** | `suelos.bin` |
| **Índice de productividad** por grupo (`PROD_SUE`, media nacional 100) | `https://dgrn.mgap.gub.uy/arcgis/rest/services/CONEAT/ConsultaConeat/MapServer/0/query` con `groupByFieldsForStatistics=SC,PROD_SUE` en la ventana del mapa (la capa también trae `IND_CONEAT` por padrón) | `productividad.bin`, `suelos.json` |
| **Aptitud para cultivos de verano de secano** y sus limitantes, por grupo | `https://dgrn.mgap.gub.uy/arcgis/rest/services/TEMATICOS/IntConeat/MapServer/11/query` (aptitud S1…N2, limitantes) unida por `OBJECTID` con la capa 12 del mismo servicio (grupo `SC`) | `suelos.json` |
| **Unidades geológicas** (código, unidad, período, descripción) | DINAMIGE (MIEM): `https://geoportal.miem.gub.uy/arcgis1091/rest/services/Dinamige_GeoS/Mapa_Base_Unidades_Geologicas_2017_GeoS/MapServer/2/query` (`f=geojson`, 334 polígonos, 17 unidades en la ventana). El servidor apareció en el código del visualizador geominero (`visualizadorgeominero.dinamige.gub.uy/Dinamige_MVC2/`) | `geologia.bin`, `suelos.json` |
| Minería conocida **con su sustancia** (tosca, balasto, arena, piedra partida, granito, arcilla…) | DINAMIGE `…/Dinamige_ConsMin/Consulta_Minera_2026/MapServer/{2,3,4,5}` (181 títulos en la ventana; antes `CatastroMineroGeoS/4` y `/5`, que no dicen qué mineral) | `materiales.bin` |
| Canteras y pozos mineros abandonados | IDE Uruguay, WFS `hidrografia:actividad_minera_abandonada_a` y `hidrografia_urbana:actividad_minera_abandonada_a` (34 + 21 polígonos) | `materiales.bin` |
| Playas, bancos de arena, bañados, arroyos, alturas, costa rocosa | los del mapa real (ver [mapa-real.md](mapa-real.md), [costa.md](costa.md), [suelos-y-roca.md](suelos-y-roca.md)) | — |

Productividad oficial de los grupos del mapa (CONEAT, media del país = 100):

| Grupo | IP | Aptitud | Cultivos de verano | Limitantes |
|---|---|---|---|---|
| 10.5 | 236 | Muy aptas | S1 | — |
| 10.11 | 210 | Muy aptas | S1 | — |
| 10.6a | 206 | Aptas | S2(e) | erosión |
| 11.9 | 201 | Aptas | S2(e) | erosión |
| 10.8b | 184 | Aptas | S2(e) | erosión |
| 03.51 | 175 | Medianamente aptas | S3(i)(d)(s) | inundable, drenaje, sodicidad |
| 03.41 | 158 | Marginales | N1(i)(d) | inundable, drenaje |
| 10.3 | 140 | Aptas | S2(e)(m) | erosión, poca agua |
| 10.6b, 10.7 | 131 | Med. aptas / aptas | S3(e)(m) / S2(e)(m) | erosión, poca agua |
| 03.2 | 131 | Marginales | N1(i)(d) | inundable, drenaje |
| 6.4 | 127 | Medianamente aptas | S3(e)(m) | erosión, poca agua |
| 11.10 | 114 | Medianamente aptas | S3(e)(m) | erosión, poca agua |
| 10.8a | 105 | Medianamente aptas | S3(e)(m) | erosión, poca agua |
| 03.3, 03.40 | 96 | No aptas / marginales | N2 / N1(i)(d) | — / inundable |
| 5.02b | 88 | No aptas | N2 | — (basamento) |
| 3.53 | 70 | Marginales | N1(d) | drenaje |
| 4.2 | 61 | Marginales | N1(e)(m) | erosión, poca agua (basamento) |
| 09.4 | 57 | Marginales | N1(e)(m) | erosión, poca agua |
| 03.52 | 53 | No aptas | N2 | — |
| 10.10 | 44 | Aptas | S2(e) | erosión |
| 9.42 | 35 | No aptas | N2 | — |
| 3.30 | 18 | No aptas | N2 | — |
| 03.10 | 9 | No aptas | N2 | — (bañados) |
| 07.1 | 4 | No aptas | N2 | — (arenas) |
| 07.2 | 0 | No aptas | N2 | — (arenas) |

El IP medio de la tierra del mapa es 146: las lomadas de limo (grupos 10.x, Formación Libertad)
de Montevideo y Canelones son de los mejores suelos del país, como sabían los chacareros del
ejido colonial. Las colinas de la costa (4.2, 5.02b) son basamento cristalino: poca tierra, mucha
piedra.

Unidades geológicas en la ventana y el material que se les asigna:

| Unidad (DINAMIGE) | Edad | Clase | Material |
|---|---|---|---|
| Formación Montevideo, Ortogneises, Migmatitas y Granitos de Suárez, Granitos Transamazónicos, Granitoides tardíos, Granito de La Paz, Zonas de Cizalla | Proterozoico | roca | **piedra** (gneises, granitos, anfibolitas) |
| Formación Libertad, Formación Dolores, Formación Fray Bentos | Pleistoceno / Oligoceno | loess | **arcilla** (lodolitas, loess, fangolitas) |
| Formación Villa Soriano, "Actual" (aluviones) | Holoceno | aluvión | arcilla, arena y algo de canto |
| Arenas litorales y fluviales, Formación Chuy | Holoceno / Pleistoceno | arena | **arena** |
| Formaciones Migues, Mercedes, Raigón | Cretácico / Plioceno | arenisca | arena, canto rodado, algo de piedra |

### Estimaciones (diseño, marcadas)

- **Zonas sin CONEAT** (1,2 % de la tierra del mapa: la punta de la Ciudad Vieja, Punta Carretas
  y pocos más): toman el grupo de la vecina más cercana **de la misma clase geológica**; quedan
  marcadas (bit 128 de `suelos.bin`) y la ficha dice «estimado».
- **Ley de cada material por unidad geológica** (`GEO_MAT` en
  `tools/mapa_real/recursos_suelo.py`): pesos de diseño (0 a 1) leídos de la descripción
  litológica de cada unidad y de la memoria de la carta geológica de Montevideo. La carta es
  1:500.000: no ve los afloramientos chicos. Detalle por material en
  [recursos-de-uruguay.md](recursos-de-uruguay.md#materiales-del-mapa-de-montevideo).
- **Piedra** (granito o gneis según la unidad; la costa rocosa, gneis): basamento + rocosidad
  CONEAT (afloramientos) + colinas de suelo superficial (zonas CONEAT 1, 2, 4, 5) + costa rocosa
  (`costa.json`) + títulos de piedra partida sobre roca.
- **Balasto**: la roca alterada bajo el suelo (menos donde aflora sana) y las areniscas
  conglomerádicas.
- **Arcilla roja** (limos y lodolitas) en las lomadas; **arcilla gris** en los bajos a menos de
  300 m de un arroyo, en los suelos de planicie (grupos 03) y en los bañados; poca en los arenales
  (07). **Tosca** en los limos de las lomadas.
- **Arena**: playas y bancos (IDE), 400 m de médanos detrás de la playa, grupos 07, la geología
  arenosa y la orilla de los arroyos anchos (más de 8 m).
- **Canto rodado**: el lecho de los arroyos a menos de 1,5 km del basamento y las unidades con
  cantos.
- **Conchilla**: la Formación Villa Soriano (estimado: la carta de materias primas de 1987 la
  explota en Villa Soriano y Piriápolis; en Montevideo la unidad aflora en el Santa Lucía y el
  Parque Lecocq). **Caliza: no hay** en el mapa ni en su entorno; la cal se importa (ver
  [recursos-de-uruguay.md](recursos-de-uruguay.md#en-el-juego)).
- **Minería conocida**: cada título marca su sustancia a 200 m; los que no la dicen y los pozos
  abandonados, lo que da la geología (piedra sobre roca; arcilla roja y tosca sobre limos; arena
  sobre arenas).
- **Aptitud** (motor, `Recursos::en_punto`): cultivo = ½ IP/210 + ½ clase de cultivos de verano
  (S1 1; S2 0,85; S3 0,62; N1 0,32; N2 0,1), por pendiente (cae entre 6 % y 20 %), bañado o vega
  de arroyo, roca a la vista y arena suelta. Pasturas = 0,15 + 0,45 · IP/170 + 0,4 · aptitud
  general, con castigos más suaves. En el mapa ficticio de las pruebas todo se estima del relieve.
- **Rinde**: chacra 0,35 + cultivo; estancia 0,4 + 0,9 · pasturas; horno, cantera y calera 0 por
  debajo de 0,15 de material y 0,3 + material (hasta 1,3) por encima (la cantera, con la ley de lo
  que saca; la calera, con la de su caliza o conchilla).
- **Molestias** y **valor del suelo**: pesos de diseño (abajo).

## Archivos

| Archivo | Grilla | Codificación |
|---|---|---|
| `suelos.bin`, `suelos_contexto.bin` | la de `altura.bin` (20 m) / `contexto.bin` (100 m) | uint8: índice del grupo en `suelos.json` (bits 0-6) + 128 si es estimación; 0 = agua |
| `productividad.bin`, `productividad_contexto.bin` | ídem | uint8: IP del grupo; 0 en el agua |
| `geologia.bin`, `geologia_contexto.bin` | ídem | uint8: índice de la unidad en `suelos.json` |
| `materiales.bin` | **40 m** (601 × 601) | 11 bytes por muestra: granito, gneis, balasto, canto rodado, arena, arcilla roja, arcilla gris, tosca, conchilla, caliza, turba (0..255 = ley 0..1) |
| `materiales_contexto.bin` | **200 m** desde −25 km (371 × 371) | ídem |
| `suelos.json` | — | grupos (IP, aptitud, cultivos de verano, limitantes, rocosidad), unidades geológicas, fuentes y resumen |

Se generan con el paso «Recursos del suelo» del constructor, que corre al final de la
construcción completa o solo:

```
python3 tools/mapa_real/construir_mapa.py --cache CARPETA --solo-recursos
```

(usa las alturas, biomas, arroyos, costa y rocosidad ya generados; los servidores cortan
conexiones a menudo: cada consulta se reintenta y, sin red ni caché, el IP y la aptitud usan una
tabla de respaldo con los valores de arriba). Deja dos imágenes de control en `docs/capturas/`:
`recursos-productividad.jpg` y `recursos-minerales.jpg`.

## Molestias

Cada fuente tiene un perfil (`recursos::molestias::PERFILES`): intensidad de olor, ruido y humo
(0 a 1) con su alcance, y lo que tira al arroyo. La molestia baja con la distancia como
(1 − d/alcance)^1,5 y las fuentes se suman.

| Fuente | Olor | Ruido | Humo | Alcance | Al arroyo |
|---|---|---|---|---|---|
| Panadería (taller nivel 1) | — | 0,05 | 0,18 | 50 m | — |
| Herrería y taller (nivel 1) | — | 0,45 | 0,12 | 80 m | — |
| Velería y jabonería (nivel 2) | 0,45 | 0,05 | 0,3 | 120 m | 0,25 |
| Curtiembre (nivel 2) | 0,9 | 0,1 | 0,05 | 180 m | 0,9 |
| Barraca y depósito (nivel 3) | 0,1 | 0,35 | — | 80 m | — |
| Horno de ladrillos | 0,05 | 0,1 | 0,85 | 260 m | — |
| Cantera | — | 0,85 | 0,2 | 320 m | 0,1 |
| Basurero | 0,9 | 0,05 | 0,15 | 300 m | 0,35 |
| Estancia (corrales) | 0,25 | 0,05 | — | 130 m | 0,15 |
| Muelle | 0,05 | 0,2 | — | 120 m | — |
| Lazareto | 0,2 | — | 0,05 | 150 m | 0,2 |
| Saladero, grasería, astillero (para las industrias de exportación) | 1 / 0,85 / 0,05 | 0,25 / 0,1 / 0,6 | 0,3 / 0,5 / 0,1 | 450 / 300 / 200 m | 1 / 0,6 / 0,1 |

El taller de cada solar industrial es el que se ve (las mismas reglas que el catálogo de
vóxeles). **Agua sucia**: una fuente a menos de 150 m de un cauce tira al punto más cercano; el
agua baja por el cauce (orientado de su punta alta a la baja, siguiendo en el arroyo donde
desemboca) hasta 5 km, diluyéndose como e^(−d/1,8 km); los solares a menos de 90 m del cauce sucio
la sufren. Aguas arriba no llega nada.

**Efectos**: en las casas, la vivienda vale menos para la felicidad (hasta −0,3) y, con más de
0,35, la salud baja y aparece el motivo «el olor, el humo y el ruido de los talleres del barrio»;
el agua sucia y el olor suman insalubridad (más enfermedad y mortalidad infantil). En el valor del
suelo (abajo) castigan a las casas el doble que a los comercios; a los talleres no.

## Valor del suelo

Por solar, de 0 a 1 (`recursos::valor`):

- **Casas**: 0,78 · e^(−d plaza/700 m) + 0,08 · e^(−d muelle/450 m) + calle (empedrada +0,06, sin
  frente −0,08) + 0,08 · agua + 0,03 · salud + 0,03 · escuela + hasta 0,05 por la costa a menos de
  250 m − 0,5 · molestias.
- **Comercios**: 0,68 · plaza + calle + 0,05 · agua + **paso de gente** (vecinos a 150 m, hasta
  +0,18) + 0,15 · muelle − 0,25 · molestias.
- **Talleres**: 0,4 · plaza + calle + **otros talleres** a 150 m (hasta +0,15) + 0,22 · muelle +
  0,18 · **materia prima** del suelo (piedra, arcilla o arena).

Usos: los umbrales de nivel de siempre (`LEVEL_VALUE`); la probabilidad de edificar o subir de
nivel se multiplica por 0,55 + 0,9 · valor (los mejores sitios crecen primero) y, en las casas,
por 1 − 0,8 · molestias; los alquileres valen 0,7 a 1,3 veces el de lista según el valor. El valor
es derivado (se recalcula cada mes, al construir o demoler y al cargar) pero **se guarda** en la
partida (`valor_suelo`, opcional): así el crecimiento entre dos meses es el mismo antes y después
de cargar.

**Campo del entorno**: la producción de las chacras y estancias privadas (la base rural de la
economía) se multiplica por 0,8 a 1,2 según la aptitud media para cultivo y pasturas a 6 km de la
plaza. Alrededor de la Ciudad Vieja da 0,57 y 0,68 (×1,03).

## En el juego

- **Capas de datos** (cajón Capas): Valor del suelo (también los solares vacíos), Molestias (en
  el terreno, en los solares y el arroyo sucio en pardo), Tierra para cultivo, Pasturas, Piedra
  (granito y gneis), Arcilla (roja y gris), Arena y grava (arena, canto rodado, balasto) y Cal,
  tosca y turba (una grilla de 6 × 6 km en celdas de 50 m alrededor del cursor; en las de
  materiales, cada celda con el color del material de más ley). Al elegir chacra, estancia, horno,
  cantera o calera se prende sola la capa que le corresponde y la ayuda dice la calidad del sitio,
  el rinde y qué saca.
- **Fichas**: del terreno (grupo CONEAT o «estimado», IP, aptitud y cultivos de verano con sus
  limitantes, geología, aptitud para chacra y estancia, cada material con su ley y para qué sirve,
  y molestias); de cada solar
  (molestias, tierra, materia prima de los talleres, componentes del valor); de chacras,
  estancias, hornos, canteras y caleras (rinde del sitio, qué material saca y qué vende) y de lo
  que molesta (qué y hasta dónde).

## API para otros sistemas

En `engine/src/city/recursos/`:

- `Recursos::de(&terreno)`: `aptitud(Uso, x, z)`, `en_punto(p)`, `en_area(c, frente, ancho,
  fondo)`, `en_lote(&solar)`, `suelo(p)` y `media_entorno(p, radio)`.
- `Uso::de_clave("arcilla" | "piedra" | "pastura" | "cultivo" | …)` y
  `rinde_por_clave(recurso, &RecursosLote)`: para `rinde_del_sitio(industria, lote)` del sistema
  de industrias (que hoy devuelve 1).
- `molestias::Perfil::desde_molestia(m)` (para los perfiles que solo traen `molestia: f32`) y
  `perfil_por_nombre("saladero" | "grasería" | "astillero" | …)`.
- Materiales con tipo y catálogo: `materiales::{Material, catalogo, de_construccion,
  se_saca_del_suelo, producto_cantera, producto_horno, fuente_de_cal}`,
  `Recursos::materiales_cerca(p, radio)` y `Recursos::construccion_cerca(clave, p, radio)` (ver
  [recursos-de-uruguay.md](recursos-de-uruguay.md#api-para-otros-sistemas)).
- Ya calculado por solar: `City::recursos_del_lote(id)`, `City::valor_del_suelo(id)`,
  `City::suelo.molestia_de(id)`.

## Pendiente

- Enchufar las industrias de exportación (saladeros, graserías): fuentes de molestias con sus
  perfiles y `rinde_del_sitio` con `rinde_por_clave`.
- Que el agua sucia del arroyo ensucie también a los aguadores que cargan ahí.
- Una carta geológica más detallada (1:100.000) cuando esté disponible por servicio; los
  afloramientos chicos del basamento bajo los limos no se ven en la de 1:500.000.
- El índice CONEAT por padrón (`IND_CONEAT`) para las estancias grandes de la escala país.
