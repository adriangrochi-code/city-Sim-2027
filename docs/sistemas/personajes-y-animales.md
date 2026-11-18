# Personajes y animales

> Código: `engine/src/detalle/ropa.rs` (figuras y vestuario), `detalle/anim.rs` (animación),
> `city/agentes/dibujo.rs` (vecinos y vehículos), `city/agentes/fauna.rs` (animales y detalles
> de la ciudad). Los modelos son de micro-vóxeles de 1/16 m: ver `modelos-de-detalle.md`.

## Qué se ve

- **Vecinos a pie**: hombres, mujeres, niños y niñas con la ropa de su oficio, su clase y su
  época, y su tono de piel.
- **Jinetes**: caballo criollo ensillado (recado, pellón, estribos) al paso, con el jinete que
  sube y baja con el paso y las manos adelante con las riendas.
- **Carretas de bueyes**: la carreta rioplatense de ruedas enormes y toldo de cuero, la yunta
  de bueyes con el yugo y el boyero caminando al costado.
- **Carros**: carro de dos ruedas con la carga, el caballo en las varas y el carrero en el
  pescante.
- **Animales**: el rodeo de cada estancia pastando en su potrero (vacas, overas, algún buey),
  la tropilla, ovejas desde 1840, la yunta de la chacra, caballos ensillados atados al palenque
  de las pulperías, perros en las calles y en las estancias, gallinas en el fondo de los
  ranchos y en las chacras.
- **Detalles de la ciudad**: palenque, letrero y tonel o barrica en cada pulpería, carreta
  parada frente a cada estancia y dos en la plaza (el mercado), botes amarrados al muelle
  (flotan con la marea; con la bajante quedan varados en el barro).

Sin caricaturas: todas las figuras tienen las mismas proporciones; lo que las distingue es la
ropa de su oficio y su época. No se representa la esclavitud (ver
`esclavitud-y-pueblos-originarios.md`): ni grilletes ni ropa o tareas asociadas a esclavizados;
la raíz familiar solo cambia el tono de piel, dentro de una gama natural.

## Atuendos

`ropa::Atuendo` (cada uno implica sexo y edad, que fijan las proporciones: hombre de 1,69 m,
mujer de 1,62 m, niño de 1,12 m; cabeza de 4 × 5 × 4 vóxeles):

| Época | Hombres | Mujeres |
|---|---|---|
| Colonia (hasta 1814) | **paisano** (poncho con guardas, chiripá, calzoncillo cribado, botas de potro, sombrero de copa baja), **peón** (camisa, chiripá, faja, vincha), **artesano** (chaleco, calzón corto, medias, gorro), **pulpero** (chaleco, delantal, arremangado), **vecino principal** (casaca con faldones abierta sobre la chupa, vueltas, medias, coleta y tricornio), **soldado** (casaca azul con cuello y vueltas encarnadas, correaje blanco cruzado, tricornio con escarapela), **fraile** (hábito con capucha caída, cordón, tonsura), **marinero** (camisa a rayas, calzón ancho, gorro de lana) | **señora** (vestido largo de una pieza y mantilla sobre la cabeza y los hombros), **mujer del pueblo** (falda, blusa, rebozo, trenza), **lavandera** (pañuelo, delantal, arremangada, el atado de ropa sobre la cabeza) |
| 1815–1879 | **levita** y galera (principales), paisano, peón, artesano, pulpero, soldado, fraile, marinero | **señora con peinetón** (el peinetón de carey de los años 1820–30 bajo la mantilla, vestido amplio), señora, mujer del pueblo, lavandera |
| 1880–1939 | **obrero** (saco corto, gorra con visera), **traje** (camisa, corbata, sombrero de fieltro), paisano en el campo | **señorita** (blusa, falda larga, rodete, sombrero de ala con cinta), lavandera |
| Desde 1940 | traje, obrero | **vestido** a la rodilla |

Niños: camisa y calzón; niñas: vestido y trenza (todas las épocas). Las reglas por época son
datos (`ropa::REGLAS`: años, sexo, grupos de oficio y atuendos con su peso): agregar una época o
un atuendo es agregar filas.

**Colores:** cada atuendo tiene sus opciones por prenda, con tintes de la época (ponchos de
vicuña, azul, pardo, colorado oscuro; chiripás azules, negros, grises; mantillas negras o
blancas; casacas azules, granates, verdes…). Cada persona elige con su semilla.

**De quién sale la ropa** (`dibujo::quien`):

1. Si el agente tiene persona en la demografía (`persona: Option<u32>`): sexo, edad (con el mes
   de la población), oficio, raíz y si es principal (riqueza, o vecino con bienes).
2. Si es un vecino sintético: sexo por el nombre, oficio (`OFICIOS` → `Oficio`), niño o anciano
   si no trabaja, raíz por el origen, principal si su casa es de nivel 3.
3. Carreteros y jinetes sin vecino: peón adulto.

Con eso, `ropa::vestir(quien, año)` da el atuendo y la paleta: **función pura**, la misma
persona en el mismo año viste siempre igual (prueba
`la_ropa_sale_de_la_persona_y_es_siempre_la_misma`). Los mayores de 55 tienen canas.

## Animación

Sin huesos ni cuadros: cada parte gira alrededor de su pivote con una fórmula (`detalle::anim`).

- **Caminar**: piernas ±0,45 rad y brazos ±0,35 en contrafase; la fase es la distancia recorrida
  / 1,5 m (los pies no patinan); la amplitud baja con la velocidad (parados, quietos). Los que
  están quietos miran de a ratos a un lado y al otro.
- **Cuadrúpedos al paso**: las cuatro patas en cuatro tiempos (trasera izquierda, delantera
  izquierda, trasera derecha, delantera derecha), cabeceo y cola; ciclo de 2,6 m el caballo,
  2,2 m los vacunos.
- **Pastar**: la cabeza abajo (el hocico llega al pasto), un cabeceo lento al arrancar el pasto,
  coletazos de a ratos. **Atados**: miran alrededor, descansan una pata trasera. **Gallinas**:
  picotazos rápidos con pausas.
- **Ruedas**: giran lo recorrido / radio (1,06 m la carreta, 0,75 m el carro).

## Los animales en el mundo

No se simulan ni se guardan: `fauna::armar(city, terreno)` deriva los grupos (`Rebano`: especie,
área circular, cantidad, semilla, modo) de los servicios y los solares, y se rehace solo cuando
cambia la firma (solares, servicios, rinde, año, pasturas). La posición de cada animal es una
fórmula del reloj (`fauna::estado`): en cada tramo de tiempo (45 s los que pastan, 14 s los
perros, 6 s las gallinas) camina de un punto de su área al siguiente —puntos al azar con su
semilla— y después pasta. Como el área es un disco y el camino es una recta entre dos puntos de
adentro, **nunca sale de su área** (prueba `los_animales_no_salen_de_su_area_y_son_deterministas`).
El reloj es el de los agentes dividido por la escala de tiempo: los animales se mueven a su
ritmo natural aunque el día pase rápido.

**Ligado a la economía:** el rodeo de cada estancia tiene 8 + 14 × rinde × pasturas cabezas
(entre 4 y 30), lo mismo que pesa en su producción de carne, cueros y sebo
(`services_input`): con sequía o mala tierra se ve menos ganado (prueba
`el_rodeo_crece_con_la_estancia_y_no_cambia_la_partida`).

## Nivel de detalle

- A menos de **110 m** del centro de la vista: los modelos de micro-vóxeles.
- De 110 a **600 m**: las cajas de siempre (5 a 12 por agente).
- Más lejos: nada (como antes). Animales y detalles fijos hasta 160 y 140 m.

## Medidas

| | |
|---|---|
| Biblioteca | 2217 cajas, 43 KB, armada una vez |
| Instancia | 48 bytes por caja (antes 32) |
| Persona | 13 a 66 cajas (≈ 25) + su sombra |
| Jinete / carreta con yunta y boyero / carro | ≈ 90 / ≈ 340 / ≈ 160 cajas |
| Ciudad del recorrido (1516 vecinos, 335 en la calle, vista a 110 m) | 4362 instancias (antes 1520): 205 KB por cuadro |
| Multitud de prueba (2500 agentes en grilla de 6 m, los ≈ 1000 de menos de 110 m con detalle) | 88 095 instancias, 4,1 MB, armadas en ~6 ms, subidas en 2,3 ms |

El banco de pruebas (`motor --bench carpeta`) agrega la tabla «Personajes y animales» con la
misma multitud en las vistas «Ciudad de cerca» y «Calle»: cuadro sin agentes, con solo cajas y
con micro-vóxeles (ver los números en `docs/sistemas/motor-voxel.md` cuando se corra con GPU).
En este entorno (render por CPU, lavapipe) los tiempos de cuadro son de 1 a 3 s y varían más que
la diferencia; la cuenta que importa en GPU es de vértices: 36 por caja, ≈ 3,2 M vértices para
la multitud de prueba completa (el terreno de la misma vista son 3,5 M triángulos).

## Capturas

`MOTOR_SOLO_PERSONAJES=1 motor --demo-ciudad carpeta`: recorrido corto que funda, traza, crece,
larga la mañana de un día hábil y una carreta, un carro y un jinete de paseo, pone la fila de
muestra de modelos de detalle y saca `personajes-calle`, `-gente-cerca`, `-carreta`, `-jinete`,
`-carro`, `-pulperia`, `-muestra`, `-muestra-cerca`, `-estancia`, `-rodeo`, `-muelle`; mide el
dibujo y guarda y carga la ciudad (tiene que dar IGUAL: nada de esto se guarda).

## Pendiente

- Sombras proyectadas en los mapas de cascadas (hoy, sombra de contacto).
- Que la demografía asigne personas a los vecinos (`asignar_trabajo` con `persona`): el
  vestuario ya las usa si están.
- Más poses: el boyero con la picana, la lavandera con el atado en las manos, gente sentada en
  los bancos, perros que siguen a los jinetes.
- Lanchas a vela en el puerto y carretas paradas en las barracas; pedidos de detalle de los
  edificios (rejas, postigos, macetas, faroles de fachada, cancel: ver `modelos-de-detalle.md`).
- Uniformes y ropa de trabajo de las épocas 3 a 6 más allá de traje y obrero.
