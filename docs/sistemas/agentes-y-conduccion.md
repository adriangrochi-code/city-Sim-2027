# Agentes y conducción realista

> Documento de sistema. Complementa al [GDD](../GDD.md) (secciones 4.3 y 4.6).
> Estado: **primera versión implementada en el prototipo 1** (época colonial).

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
