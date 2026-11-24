# Servicios, incendios, puerto, muralla, campo e iglesia (motor)

> Documento de sistema. Complementa al [GDD](../GDD.md) (secciones 4.5 y 4.10), a la
> [época colonial](../epocas/01-colonial.md) y a
> [politicas-y-recursos.md](politicas-y-recursos.md).
> Estado: **portado al motor de vóxeles** (Fase 2, punto 5 de la hoja de ruta) desde
> `scripts/city/{services,fires,port,wall,church,epidemics}.gd` del prototipo.
> Código: `engine/src/city/services/` (lógica), `engine/src/structures/colonial.rs` (vóxeles),
> `engine/src/ui/services.rs` (tarjetas, capas y fichas).

## 1. Cómo se engancha a la ciudad

Todo el estado vive en `City::services` y se toca la ciudad en cuatro puntos, para que crezca
por épocas sin reescribir nada:

| Momento | Qué pasa | Función |
|---|---|---|
| Cada paso de crecimiento (0,5 s de juego) | Fuego (igniciones, baldes, propagación con viento) y contagios | `City::services_step` |
| Cada mes, antes de la economía | Reparto del agua, obras (iglesia y muralla), barcos y acontecimientos lejanos, epidemias, faroles | `City::services_month` |
| Cierre del mes de la economía | Producción del campo, hornos y canteras; barcos, contrabando, agua, faroles y mantenimiento | `City::services_input` |
| Al cargar la partida | Se regeneran los vóxeles de servicios, muelle, muralla, iglesia, faroles y ruinas | `City::services_rebuild` |

Cada servicio tiene su ficha en `services::CATALOG` (costo en pesos, ladrillos y piedra,
mantenimiento, capacidad y calidad del agua, alcance, alcance contra el fuego, tamaño real y
constructor de vóxeles). Las épocas siguientes agregan fichas (bomba de incendios, toma de la red
de agua corriente, comisaría…) sin tocar la cobertura.

## 2. Agua

| Fuente | Personas | Calidad | Alcance | Contra el fuego |
|---|---|---|---|---|
| Pozo de balde | 60 | 0,7 (0,35 si hay curtiembres o velerías a menos de 150 m) | 110 m | 140 m |
| Aljibe público | 160 | 1,0 | 170 m | 180 m |
| Puesto de aguadores | 360 | 0,9 | 1.100 m **por la red de calles** | 450 m |

- Se reparte de la mejor fuente a la peor, a los solares edificados más cercanos, hasta la
  capacidad. Los aguadores solo llegan a los solares que están en **la misma red de calles**
  que su puesto (GDD 4.5: «los servicios viajan por la red de calles»).
- **Sequía** (del clima, `climate.rs`): las fuentes dan un 30 % menos; con la ordenanza de turnos
  de agua, algo más. **Aljibe obligatorio**: las casas de altos tienen agua propia.
- **Efectos**: sin agua una casa o comercio no pasa del rancho; con agua mala no pasa de una
  planta (los talleres tienen su propio pozo). La fracción de vecinos con agua entra en la
  opinión de los vecinos. La ficha del solar dice «Sin agua» en «¿Por qué no crece?».
- Capa de datos **Agua**: azul la buena, celeste la regular, amarillo la mala, rojo sin agua, y
  el alcance de cada fuente.

Base histórica: la ciudad se abasteció durante unos 150 años de fuentes (manantiales),
cisternas y aljibes, según lo recordado por la Intendencia de Montevideo con motivo del hallazgo
de una posible fuente colonial en el Cabildo ([nota de prensa](https://economiasustentable.com/noticias/historico-hallazgo-en-montevideo-descubren-una-fuente-colonial-que-reescribe-la-historia/)).
Capacidades, calidades y alcances son **estimaciones de diseño**
heredadas del prototipo.

## 3. Serenos y faroles

Un puesto de serenos vigila 260 m: pone **faroles de sebo** cada 30 m a los lados de las
calles (vóxeles de poste y fanal que se ven de noche), cobra el sebo en el presupuesto y da la
voz de fuego (los incendios empiezan más chicos y se apagan antes). Capa **Serenos**.
Parámetros: estimación de diseño.

## 4. Incendios

No son al azar: dependen del **techo** y de las **paredes** de cada edificio, es decir, de los
materiales con que se levantó (ver [materiales de construcción](materiales-de-construccion.md);
las partidas viejas usan el techo de su tipo de casa):

| Techo | Igniciones por mes | Facilidad para que le salte el fuego |
|---|---|---|
| Paja (ranchos, galpones de nivel 1) | 0,15 % | 1,0 |
| Teja | 0,03 % | 0,25 |
| Azotea de ladrillo | 0,01 % | 0,08 |
| Tablas | 0,10 % | 0,6 |
| Cuero | 0,07 % | 0,4 |

Las paredes multiplican: tablas ×1,8 (y +0,35 de contagio), barro y paja ×1,1, piedra ×0,8.

- Los talleres arden el doble (fraguas y calderas); la sequía y el viento fuerte, más. La
  ordenanza «sin paja cerca de la plaza» baja un 70 % el riesgo de los ranchos a menos de 400 m.
- **Viento** (`water.rs`, dirección y velocidad de cada hora): la probabilidad de saltar a un
  vecino se multiplica por `1 + 0,8 · cos · v/4` (cos: alineación con el viento) y, a favor del
  viento, las chispas llegan `2 · v` metros más lejos que los 20 m de base.
- **Lucha**: con una fuente a su alcance, los vecinos forman la cadena de baldes y la intensidad
  baja; con serenos baja más. Sin agua sube hasta el máximo y, después de 6 s de juego al máximo,
  la casa se derrumba: queda en **ruinas** (vóxeles de muros quemados y ceniza) hasta que el solar
  se vuelve a edificar. Todo pasa por la gaceta.
- Capa **Fuego**: riesgo de cada casa, lo que arde y las ruinas.

Todos los números son **estimaciones de diseño** (no hay estadísticas coloniales de incendios
en este repositorio).

## 5. Puerto, aduana, epidemias

- **Muelle**: se coloca en una orilla baja (0,3 a 4 m) junto a un camino; entra al río hasta
  pasar el agua de 1,6 m de profundidad (40 a 160 m), sobre pilotes que bajan al lecho; con
  barraca de cueros si hay lugar en tierra. Sin muelle no hay comercio y los barcos desembarcan
  el 60 % de la gente en la playa.
- **Barcos**: uno cada 3 a 4,5 meses (más con libre comercio). Traen pobladores; con muelle, el
  mes del barco la economía comercia (importa lo que falta, exporta cueros).
- **Acontecimientos lejanos** (ficticios, GDD 4.11): guerra lejana, reglamento de comercio libre,
  fiebre del cuero, barco de pobladores, bloqueo del río, fiebre en los barcos.
- **Aduana** (junto al muelle): sin ella se pierde por contrabando el 35 % de los derechos; con
  ella, el 5 % (estimación).
- **Epidemias**: un barco trae fiebre con 6 % de probabilidad (50 % durante «fiebre en los
  barcos»). Con **lazareto** los pasajeros hacen dos meses de cuarentena y entran sanos; sin él,
  la fiebre empieza en las casas cercanas al desembarco y se contagia (más con agua mala, menos con
  la limpieza de calles), mata y espanta pobladores.

Base histórica: Montevideo tuvo su lazareto en la Isla de Flores **desde mediados del siglo XIX
hasta principios del XX**, con cuarentenas de hasta 40 días
([Ministerio de Ambiente](https://www.gub.uy/ministerio-ambiente/comunicacion/noticias/isla-flores-area-protegida-supo-epidemias)).
En la época colonial del juego el lazareto es, por lo tanto, una **licencia de diseño**
(anticipa lo que la ciudad tuvo después); la duración de la cuarentena (2 meses) es una
simplificación.

## 6. Muralla y portones

Se traza por tramos (hasta 800 m). Se pagan 18 pesos/m al trazarlo y se levanta mes a mes,
30 m por mes, con 3 de piedra por metro (si falta piedra, la obra se para). En vóxeles: muro de
6 m de alto y 4 m de espesor con talud, cordón, adarve con almenas hacia afuera y garitas en los
extremos; en obra, a media altura con andamios. Donde un tramo cruza una calle queda un
**portón** (vano libre de 5 m de alto apoyado a la altura de la calle) cuyos guardas cobran
derechos. No es defensiva (GDD 4.10): marca la ciudad y cobra. No se generan solares debajo; las
calles la cruzan.

Base histórica: la Ciudadela de Montevideo se empezó en 1741; los portones de San Juan y San
Pedro se demolieron en 1829 y la ciudadela en 1877; de ella queda solo la Puerta de la Ciudadela,
rearmada en 1959 junto a la Plaza Independencia
([Wikipedia: Gateway of the Citadel](https://en.wikipedia.org/wiki/Gateway_of_the_Citadel)).
Las medidas del muro, el ritmo de obra y la piedra por metro son **estimaciones**.

## 7. Iglesia matriz por fases

La ciudad se funda con una **capilla de adobe y paja** (fase 0). Con 1.000 habitantes se
financian las fases: cimientos de piedra (6 meses), muros (10), naves y techo de teja (10),
cúpula (8) y torres con campanario (8). Cada fase se paga al empezar y consume ladrillos y
piedra mes a mes; si faltan, la obra se para y lo dice la gaceta. Cada fase terminada atrae un
8 % más de pobladores. En vóxeles se ve la fase en obra (ladrillo a la vista y andamios) y, al
terminar, revocada a la cal.

Base histórica: en el solar de la actual Catedral hubo una iglesia de ladrillo desde 1740; la
piedra fundamental de la iglesia actual se puso en 1790 y se consagró en 1804
([Wikipedia: Catedral de Montevideo](https://es.wikipedia.org/wiki/Catedral_de_Montevideo)).
La capilla de adobe inicial, los costos y los meses de cada fase son **estimaciones de diseño**
del prototipo.

## 8. Campo

| Edificio | Produce | Clima |
|---|---|---|
| Chacra (60 × 80 m, a más de 300 m de la plaza) | 3.600 raciones de grano al año | Cosechas de `climate.rs` (trigo en diciembre y enero, maíz en otoño): la sequía y las heladas las achican |
| Estancia (casco de 50 × 45 m, a más de 600 m) | 500 de carne, 60 cueros y 20 de sebo por mes | Factor de pasturas |
| Horno de ladrillos (junto a barro: arroyo o costa) | 40 ladrillos por mes | El adobe no seca con lluvia ni frío |
| Cantera | 60 de piedra por mes | — |

Se suman a la producción de base del entorno rural. Cantidades: **estimaciones de diseño**.

## 8b. Servicios de las épocas siguientes

Cada servicio tiene su época (`services::epoca_de`); antes de ella la tarjeta dice "Época N" y
colocarlo responde "Llega con la época …".

| Época | Servicios | Dónde está |
|---|---|---|
| 2, Ciudad Nueva | Usina de gas, teatro, usina de agua corriente, colector y desagüe | [arquitectura-epoca2.md](arquitectura-epoca2.md) |
| 3, Inmigración | **Estación de ferrocarril** (una por ciudad; con la concesión la paga la compañía: `City::costo_servicio`), **usina eléctrica** (faroles eléctricos por su red, a más de 250 m de la plaza), **cocheras de tranvía** (líneas por la red de calles), **hotel de inmigrantes** (uno, cerca del muelle o del puerto moderno) y **puerto moderno** (uno, sobre la costa, con el muelle del lado del agua) | [arquitectura-epoca3.md](arquitectura-epoca3.md) |

La estación (×1,1), el hotel de inmigrantes (×1,2) y el puerto moderno (×1,25) multiplican las
llegadas (`Services::attraction`) [E].

## 9. Interfaz

- **Servicios**: pestañas de agua (pozo, aljibe, aguadores), educación, salud (con el lazareto),
  seguridad (serenos, alcalde de barrio, cárcel), saneamiento e iglesia (financiar la fase); ver
  [educacion-salud-seguridad.md](educacion-salud-seguridad.md).
- **Producción**: chacra, estancia, horno, cantera (atahona y saladero, próximamente).
- **Puerto y muralla**: muelle, aduana y muralla (dos clics por tramo; vista previa con los
  portones).
- **Capas de datos**: agua, serenos y fuego.
- Al colocar, el edificio se arrima a la calle más cercana con el frente hacia ella; la vista
  previa muestra la huella (verde o roja) y el alcance. La ficha de un servicio muestra a cuántos
  abastece y tiene **Demoler**; la de la iglesia, su fase y **Financiar la fase**.

## 10. Guardar y cargar

`CityData::servicios` (opcional: las partidas viejas cargan sin servicios) guarda edificios,
reparto del agua y topes de nivel, iglesia, tramos de muralla, muelle, acontecimiento y barcos,
fuegos, ruinas, epidemia, cuarentenas y el estado del azar. Los vóxeles se regeneran.

## Pendiente

- Ampliaciones y modernización (segunda cisterna, más serenos, alargar el muelle…, ver
  [edificios-ampliables.md](edificios-ampliables.md)) y agentes visibles (aguadores, serenos,
  barcos y lanchas) — los barcos esperan al sistema de vehículos.
- Atahona, saladero y ciudadela; la decisión final de la muralla.
- ~~Contaminación de pozos por pozos negros~~: hecho, junto con escuelas, salud, delito y
  basurales (ver [educacion-salud-seguridad.md](educacion-salud-seguridad.md)).
