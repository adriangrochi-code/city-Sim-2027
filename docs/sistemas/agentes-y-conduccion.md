# Agentes y conducción realista

> Documento de sistema. Complementa al [GDD](../GDD.md) (secciones 4.3 y 4.6).
> Estado: **primera versión implementada en el prototipo 1** (época colonial) y **portada al motor** (ver «En el motor»). Desde el 23 de noviembre de 2026, agenda encadenada, vecinos = personas de la población, perfil de conducta, tiempos con olvido, portones con aduana, arreos, adelantamiento y prioridad en cruces: ver [ia-y-transito.md](ia-y-transito.md), «Estado».

## Objetivo

Que la gente del juego **se comporte como personas reales**: que tenga rutinas, que elija
rutas con sentido, que conduzca con cuidado y que aprenda de su experiencia. Es la respuesta
directa a uno de los grandes fallos del referente del género: agentes que iban todos al
trabajo libre más cercano, por la misma calle, y que formaban atascos absurdos.

La regla de diseño: **si un jugador sigue a cualquier vecino durante un día, lo que hace debe
tener sentido.**

## 1. Personas con vida propia

Cada vecino es una persona concreta:

| Atributo | Ejemplo | Para qué sirve |
|---|---|---|
| Nombre y origen | *Josefa Machado, llegada de las Islas Atlánticas* | Identidad; se ve al inspeccionarla |
| Oficio y rol | Costurera; *trabaja en su casa-taller* | En la época colonial la mayoría trabaja en casa; solo una parte sale |
| Casa y trabajo **fijos** | Su casa y el taller donde trabaja | No cambia de trabajo cada día |
| Horario habitual | Sale a las 6:20, vuelve a las 17:10 | Cada día varía unos minutos, no al azar |
| Medio de transporte | A pie / a caballo | Los más acomodados van a caballo; para trayectos cortos todos caminan |
| Temperamento | Prudente / tranquilo / apurado | Cambia cómo conduce (ver 3) |
| Memoria de viajes | "Al trabajo: 18 min (esperaba 12)" | Base del aprendizaje (ver 2) |

**Rutina semanal (época colonial):** de lunes a sábado, trabajo y algún paso por el mercado de
la plaza al mediodía; los domingos, misa en la iglesia matriz. De noche casi nadie sale.
Las **carretas** traen cueros y trigo del campo durante el día y vuelven con mercancías.

## 2. Elegir la ruta con sentido

- **Peatones:** el camino más corto.
- **Vehículos:** el camino **más rápido según el tiempo medido** en cada carril. Cada vez que
  un vehículo recorre un tramo, su tiempo real actualiza el promedio de ese carril. Así, los
  atascos que ya existen pesan en las decisiones de los demás.
- **Percepción personal:** cada vecino "ve" los tramos un poco distinto (±15 %). Por eso no
  todos eligen la misma calle y el tráfico se reparte de forma natural.
- **Aprendizaje:** si un viaje le sale mucho peor de lo que esperaba (más de un 50 %), al día
  siguiente prueba otra ruta. Si le va bien, mantiene la suya (hábito). Esto evita que todos
  cambien a la vez y se forme el atasco en la calle nueva.

## 3. Conducir como una persona

Los vehículos usan un **modelo de conductor inteligente** (IDM, *Intelligent Driver Model*),
un modelo clásico de la ingeniería de tráfico:

- **Distancia de seguridad** según la velocidad: más rápido, más distancia.
- **Aceleración y frenada suaves**, con frenada fuerte solo en emergencias.
- **Velocidad según la vía:** más lento en camino de tierra que en empedrado.
- **Curvas:** aflojan según el radio (aceleración lateral cómoda).
- **Giros:** reducen antes de doblar en un cruce.
- **Ceder el paso:** en cada cruce de tres o más calles pasa **uno a la vez, por orden de
  llegada**. Solo pide paso el primero de cada fila.
- **No bloquear el cruce:** nadie entra si la calle de salida está llena.
- **Adelantamiento prudente:** un jinete detrás de una carreta lenta solo adelanta si el carril
  contrario está libre a lo largo de varias decenas de metros, y vuelve a su carril al pasarla.
  Los "apurados" adelantan más; los "prudentes", nunca.
- **Temperamento:**

| Temperamento | Velocidad deseada | Distancia de seguridad | Adelanta |
|---|---|---|---|
| Prudente | −10 % | +35 % | Nunca |
| Tranquilo | normal | normal | A veces |
| Apurado | +12 % | −25 % | Con frecuencia |

- **Salvaguarda:** si quien tiene el paso en un cruce no avanza en un minuto, lo cede, como
  harían dos personas que se hacen señas. En las pruebas, un día completo con más de 2.000
  viajes termina sin ningún agente atascado.

**Parámetros por tipo (época colonial)**

| Tipo | Velocidad | Aceleración | Distancia mínima | Tiempo de reacción | Largo |
|---|---|---|---|---|---|
| A pie | 4,9 km/h | — | — | — | 0,6 m |
| Jinete | 13,7 km/h | 1,3 m/s² | 2 m | 1,3 s | 2,6 m |
| Carreta de bueyes | 4,1 km/h | 0,25 m/s² | 3 m | 2,4 s | 7,5 m |

## 4. Legibilidad para el jugador

- **Inspeccionar a cualquier vecino o carreta:** nombre, oficio, a dónde va y por qué,
  estado ("en marcha", "cediendo el paso", "esperando en la fila", "adelantando"), velocidad,
  minutos de viaje frente a lo esperado, y cuántas veces cambió de ruta por demoras.
- **Seguirlo con la cámara** (tecla F).
- **Inspeccionar una calle:** tiempo de paso medido en cada sentido frente al tiempo libre.

## 5. Evolución en las épocas siguientes

| Época | Novedades de comportamiento |
|---|---|
| 2. Ciudad Nueva | Diligencias con horario; carruajes de alquiler que buscan clientes |
| 3. Inmigración | Tranvías con paradas y horario; los peatones esperan en la parada y eligen entre caminar o tomar el tranvía según tiempo y costo |
| 4. Art Déco | Automóviles y ómnibus: varios carriles, **cambio de carril** con espacio suficiente, **semáforos** y agentes de tránsito, **estacionamiento** (buscar lugar es parte del viaje) |
| 5. Modernismo | Avenidas y vías rápidas con incorporación (*merge*) cediendo el paso, horas pico marcadas |
| 6. Contemporánea | Ciclistas en bicisendas, peatonales, ómnibus eléctricos con prioridad |

Comportamientos transversales previstos:
- **Peatones que cruzan** por las esquinas y vehículos que los respetan.
- **Clima:** con lluvia y barro, todos van más lentos y con más distancia; con temporal, menos
  gente sale.
- **Elección del medio de transporte** por tiempo, costo y comodidad (no fija).
- **Imprevistos con sentido:** una calle cortada por obras o una inundación hace que los vecinos
  recalculen la ruta en el momento.
- **Simulación por niveles de detalle:** lejos de la cámara, los viajes se resuelven de forma
  agregada con los mismos tiempos medidos, para escalar a ciudades enormes.

## En el motor

Porte del prototipo a `engine/src/city/agentes/` (Rust), con la misma semántica:

| Archivo | Qué hace |
|---|---|
| `agentes/mod.rs` | Vecinos, agenda del día, viajes, pasos de la simulación, cruces, llegadas, estadísticas y consultas para la interfaz |
| `agentes/idm.rs` | Tipos de agente (`Tipo`: a pie, jinete, carreta, carro) con sus parámetros, y el IDM puro (`aceleracion`, `paso`) |
| `agentes/ruta.rs` | A* sobre la red de calles, tiempos medidos por carril (`Tiempos`) y caché de rutas |
| `agentes/dibujo.rs` | Alturas de cada tramo (calzada o tablero del puente), vestuario de cada vecino, modelos de micro-vóxeles de cerca y de cajas de lejos, y la lista de instancias |
| `agentes/fauna.rs` | Animales (rodeos, tropillas, caballos atados, perros, gallinas) y detalles fijos de la ciudad derivados de los servicios y los solares; `cajas_ciudad`, todo lo que se dibuja con los vecinos |
| `render/shaders/agentes.wgsl` | Una instancia por caja, con partes que se inclinan alrededor de su pivote; micro-vóxeles, luz, sombra recibida, niebla, curvatura y sombra de contacto |

**Vecinos.** Cada casa habitada tiene tantos vecinos como habitantes según su nivel y densidad
(`Lot::residents`). Sus atributos (nombre, origen, oficio, rol, a pie o a caballo, temperamento,
horario) salen de la semilla del solar en una sola función, `habitante_de_lote`: cuando llegue
el módulo de demografía, se reemplaza esa función por las personas de los hogares del solar (cada
vecino y cada agente ya tienen un campo `persona`), y `Agentes::asignar_trabajo` deja fijar el
trabajo desde afuera. Trabajo: un comercio o taller de la ciudad; el 15 % son peones del campo
(salen por el extremo de calle más alejado de la plaza) y el 10 % trabajan en la orilla del
puerto (la calle más cercana al agua, hasta que exista el muelle).

**Agenda.** La hora es la del reloj del juego (la sesión la pasa en cada cuadro; el día cambia a
medianoche, y el día 0 es lunes). Cada día se arma la agenda: trabajo de lunes a sábado con su
horario y una variación diaria, mercado de la plaza al mediodía para algunos, misa en la iglesia
matriz los domingos (60 %), carretas que traen cueros y trigo del campo (4 + 1 cada 200
habitantes, hasta 30; vuelven al campo más tarde) y carros de reparto a las tiendas. Los
vecinos que llegan a mitad del día se suman a lo que queda de la agenda.

**Rutas.** A* con heurística admisible; a pie, por largo; en vehículo, por el tiempo medido de
cada carril (media móvil, nunca menos que el tiempo libre) con la percepción personal de ±15 %
(8 variantes, según la semilla de ruta del vecino). Se busca desde los dos extremos del tramo
de salida hacia los dos del de llegada y se queda la mejor. Las rutas entre nodos se guardan en
caché (se vacía al cambiar la red y cada día). Los puentes son tramos como cualquier otro. Si un
viaje tarda más de 1,5 veces lo previsto, el vecino cambia su semilla de ruta (aprende).

**Movimiento.** Paso fijo de 0,25 s simulados, con los agentes 10 veces más rápidos que el
tiempo real (como en el prototipo; a velocidad 1 son 40 pasos por segundo, hasta 48 por
llamada). Los vehículos de un mismo carril se ordenan y cada uno sigue al de adelante (o al
último del carril siguiente) con el IDM; el avance nunca supera el hueco, así que no se
superponen. Ceden el paso en los cruces de tres o más calles por orden de llegada, solo si la
calle de salida tiene lugar, con la salvaguarda de un minuto; aflojan en curvas (aceleración
lateral 1,2 m/s²), antes de doblar y en caminos de tierra. La gente a pie va por la vereda y no
sigue a nadie. Tope: 3000 agentes en la calle.

**Nivel de detalle.** Los agentes en tramos a más de 900 m del centro de la vista avanzan un
paso de cada cuatro con el paso cuadruplicado. El costo está dominado por ordenar los carriles,
así que la ganancia es chica: con 3000 agentes, un paso cuesta 0,2 ms (≈ 1 % de un núcleo a
velocidad 1); con la vista encima, 0,4 ms (se guardan las posiciones para interpolar el dibujo).

**Determinismo y partidas.** Azar propio, agentes en orden de id y colas ordenadas: con la misma
partida y las mismas horas, la simulación da lo mismo (prueba
`un_dia_de_la_ciudad_sin_choques_y_determinista`, que además guarda y carga a mitad del día y
sigue igual). Se guarda todo el estado (`agentes::Estado`, en `CityData::vecinos`): vecinos,
agentes en camino, tiempos medidos, agenda y estadísticas. Las partidas viejas regeneran los
vecinos desde los solares. Lo único que no se guarda es la caché (rutas, alturas, lugares), que
se rehace sola.

**Dibujo.** A menos de 110 m del centro de la vista, modelos de **micro-vóxeles de 1/16 m**
(`detalle`, ver `personajes-y-animales.md` y `modelos-de-detalle.md`): cada vecino vestido según
su persona, su oficio y la época, caminando con piernas y brazos; el jinete sobre su caballo
ensillado al paso; la carreta con las ruedas que giran, la yunta y el boyero; el carro con su
caballo y el carrero. Más lejos, los modelos de cajas de antes (persona de 0,5 × 1,75 m; jinete;
carreta; carro). 48 bytes por caja, subidos cada cuadro (solo si cambiaron), con la posición
interpolada entre los dos últimos pasos. Solo los que están a menos de 600 m del centro de la
vista, hasta 2500 (los más cercanos). La altura sale del perfil del tramo, con la misma cuenta
que la voxelización de la calzada (y el tablero en los puentes), tomada en el medio del vehículo.

**Interfaz.** Clic en la población de la barra superior: panel «Vecinos y tránsito» (en la calle
por tipo, viajes de hoy y de ayer, viaje medio, demorados, los que no pudieron salir por calles
sin conexión, demora del mes y carretas llegadas al mercado). La ficha de una calle muestra
quién la está usando y el tiempo de paso medido en cada sentido frente al libre; inspeccionar
justo sobre un agente muestra su ficha (nombre, oficio, a dónde va, estado, velocidad, minutos
de viaje).

**Efecto en la ciudad.** Al cerrar cada mes, si los viajes en vehículo tardaron en promedio más
de 1,3 veces lo previsto, la opinión de los vecinos baja (hasta 6 puntos).

**Pendiente:** adelantamiento de jinetes, peatones que cruzan en las esquinas, clima (barro y
lluvia), seguir a un agente con la cámara, selección con el ratón de agentes en movimiento,
servicios del prototipo (aguateros, serenos, aduana del portón, arreos), y el nivel de detalle
agregado para ciudades muy grandes.
