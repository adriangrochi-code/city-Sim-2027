# Educación, salud, saneamiento y seguridad (motor)

> Documento de sistema. Complementa al [GDD](../GDD.md) (secciones 4.5, 4.6 y 4.10), a
> [servicios](servicios.md), a [población](poblacion.md) y a
> [políticas y recursos](politicas-y-recursos.md).
> Estado: **implementado en el motor para la época colonial**; las épocas siguientes quedan
> como datos (tarjetas desactivadas «Época N»).
> Código: `engine/src/city/services/barrio.rs` (modelo), `engine/src/city/services/mod.rs`
> (catálogo), `engine/src/city/poblacion/{demografia,felicidad}.rs` (efectos en las personas),
> `engine/src/structures/colonial.rs` (vóxeles), `engine/src/ui/services.rs` (tarjetas y
> pestañas), `engine/src/city/game.rs` (capas y fichas).

## 1. Cómo funciona

Los edificios de servicio dan una **cobertura por solar**; la población la usa persona por
persona. Cada mes (`City::services_month`):

1. **Saneamiento** (con azar): los pozos y aljibes se ensucian de a poco con los pozos negros, la
   basura y el basurero de alrededor; aparecen basurales en los baldíos y las carretillas los
   levantan.
2. **Agua** (como antes, pero con la contaminación de cada pozo).
3. **Barrio** (sin azar, `City::refresh_barrio`): escuela, salud, insalubridad, vigilancia y
   delito de cada solar. Se recalcula también al construir o demoler y al cargar la partida.
4. **Brotes de disentería** y **delitos del mes** (con azar).

La población recibe la cobertura por `Entorno::servicios` (un `ServiciosLote` por solar). Con la
ciudad del motor `Entorno::con_servicios` es verdadero; la ciudad sintética de las pruebas de
población funciona como antes (las ordenanzas de escuelas y hospital actúan en toda la ciudad).

Todo es determinista (el azar es el de los servicios, `Rng::from_state`) y se guarda; lo
derivado se recalcula al cargar.

## 2. Educación

| Edificio | Alumnos | Rinde | Alcance | Costo | Por mes |
|---|---|---|---|---|---|
| Escuela de primeras letras (12 × 16 m) | 100 | 1,0 | 400 m | 1.500 pesos, 50 ladrillos | 35 |
| Convento con aulas (30 × 36 m, uno por ciudad) | 60 | 0,8 | 650 m | 5.000 pesos, 250 ladrillos, 80 piedra | 15 |

- Los lugares se reparten, como el agua, a las **casas más cercanas** dentro del alcance hasta
  llenar el cupo. Cuentan **todos los niños de 6 a 12 años** de cada casa (el juego no
  representa la esclavitud: no hay niños excluidos de la escuela, ver
  [poblacion.md](poblacion.md), sección 5).
- **Quién va**: a la probabilidad de base por clase (la familia o un preceptor) se suma
  `0,4 × acceso × paga`. Si el cabildo **paga al maestro** (ordenanza «Escuelas de primeras
  letras»), los pobres y modestos van como los demás; si no, solo el 25 % (pobres) o el 60 %
  (modestos) de esa probabilidad.
- **Efectos**: más lectura → oficios letrados y movilidad (ya modelado en la población:
  comerciantes, pulperos, maestros) → ingresos. Felicidad **«Educación»** (nuevo factor 12) para
  los padres de chicos de 5 a 12 años y para los propios chicos: sin escuela cerca, «no hay
  escuela para sus hijos»; sin poder pagarla, «no puede pagar la escuela de sus hijos».
- Prueba: `con_escuelas_aprenden_a_leer_mas_ninos` (ocho años después, más jóvenes leen) y
  `la_escuela_tiene_cupo_y_alcance`.

## 3. Salud

| Edificio | Atiende | Rinde | Alcance | Costo | Por mes |
|---|---|---|---|---|---|
| Botica (8 × 12 m) | 400 personas | 0,35 | 300 m | 900 pesos | 8 |
| Médico y cirujano (12 × 18 m) | 700 | 0,5 | 900 m **por la calle** | 1.400 pesos | 30 |
| Hospital de caridad (36 × 30 m) | 1.500 | 0,85 | 1.200 m **por la calle** | 6.000 pesos, 300 ladrillos, 100 piedra | 60 |
| Lazareto (ya existía) | cuarentena de los barcos | | | | |

- Cada uno atiende a las personas de los solares más cercanos hasta su capacidad; el médico y el
  hospital solo llegan a su red de calles (GDD 4.5). Varias coberturas se combinan:
  `1 − (1 − a)(1 − b)`.
- **Mortalidad**: × `(1 − 0,3 · salud · acceso)`; el acceso de pobres y modestos es la mitad si
  el cabildo no paga la **asistencia de caridad** (la ordenanza «Hospital de caridad y asistencia a
  pobres»). **Salud** de las personas: `+0,08 · salud · acceso`. Felicidad «Salud»: un cuarto
  depende de tener quién atienda («no hay médico ni botica cerca»).

## 4. Saneamiento colonial

- **Pozos negros y letrinas por tipo de casa** (`barrio::foco`): rancho 1,0 (hoyo poco hondo),
  casa de azotea 0,75, casa de altos 0,6 (pozo negro de ladrillo), ×1,3 en densidad alta; comercio
  0,5; taller 0,8; curtiembre o velería 2,0. En vóxeles, los ranchos y las casas de azotea tienen
  su **letrina** (casilla de tablas con techo de paja) al fondo.
- **Pozos de agua que se contaminan**: cada mes la contaminación de un pozo va un 12 % hacia
  `0,1 · Σ focos a menos de 40 m + 0,3 por basural a menos de 60 m + 0,5 si hay un basurero a menos
  de 250 m` (el aljibe, la mitad). El agua del pozo pierde hasta un 65 % de calidad (el aljibe, un
  30 %); con calidad < 0,6 la casa no pasa de una planta. Se suma a la regla vieja de los talleres
  a menos de 150 m. Ficha del pozo: «agua N % sucia (huele mal)»; gaceta cuando un pozo pasa del
  50 %.
- **Basurales**: en un baldío con 5 o más casas a menos de 60 m aparece un basural con
  probabilidad mensual de 2,5 % (1,2 % si hay un basurero a menos de 700 m al que los vecinos
  lleven la basura; 0,8 % con recolección pero sin dónde descargar), ×0,5 a ×1,5 según las casas.
  Se ven en vóxeles (montones de barro, huesos, cueros, trapos y loza rota, y una rueda de carreta)
  hasta que se edifica el solar o las carretillas lo levantan.
- **Insalubridad** de cada solar (0 a 1): agua (sin agua 0,3; con agua `(1 − calidad) · 0,6`) +
  pozos negros a menos de 30 m (hasta 0,35) + basural a menos de 60 m (0,35) o de 120 m (0,15) +
  basurero a menos de 250 m (0,2); ×0,85 con la limpieza de calles.
- **Efectos**: mortalidad × `(1 + insalubridad · 1,3)` en los menores de 5 años (diarreas) y
  × `(1 + 0,35 · insalubridad)` en los demás; salud `−0,15 · insalubridad`. **Brotes de disentería**
  cuando la insalubridad media de las casas pasa de 0,3 (probabilidad `(media − 0,3) · 0,25` por mes):
  golpean las 8 casas peores por `Poblacion::choque_epidemia`. El agua mala, además, ya hacía que las
  epidemias de los barcos se contagiaran más.
- **Herramientas del jugador**:
  - ordenanza **«Alejar los pozos negros de los pozos de agua»** (10 pesos de inspección, molesta a
    los vecinos): los pozos negros ensucian el agua un 65 % menos y la insalubridad local a la
    mitad;
  - ordenanza **«Recolección de basura con carretilla»** (15 pesos más 15 por cada 1.000
    habitantes): con un basurero en la misma red de calles y a menos de 1.600 m, las carretillas
    levantan los basurales (la mitad por mes) y no se forman nuevos;
  - edificio **«Basurero fuera de la muralla»** (a más de 450 m de la plaza): donde descargan las
    carretillas; ensucia los pozos y las casas cercanas;
  - la **limpieza de calles** existente (epidemias −40 %, insalubridad −15 %).
- Pruebas: `los_pozos_negros_ensucian_los_pozos_y_enferman`, `basurales_y_recoleccion`,
  `la_insalubridad_y_la_salud_mueven_la_mortalidad`.

## 5. Seguridad y delito

- **Riesgo de delito** de cada solar edificado (antes de la vigilancia):
  `0,06 + 0,30 · pobreza + 0,25 · desigualdad + 0,20 · desocupación + 0,15 · oscuridad +
  0,08 · densidad + 0,12 · baldíos y ruinas + 0,15 · puerto (+0,05 en comercios)`, donde, en un
  vecindario de 120 m: pobreza = hogares pobres / hogares; desigualdad = `min(1, 4 · pobreza ·
  ricos)` (hogares pobres y acomodados juntos); desocupación = hombres libres de 16 a 60 sin oficio
  o sin trabajo en un comercio o taller; oscuridad = ningún farol a menos de 35 m; densidad = casas a
  menos de 60 m / 14; baldíos = ruinas y basurales a menos de 60 m / 3; puerto = cercanía al muelle
  (300 m: contrabando y marineros).
- **Vigilancia**: `1 − Π(1 − q)` de los que cubren el solar — **serenos** (los mismos de antes:
  ronda de 260 m, q 0,3, y sus faroles bajan la oscuridad), **garita del alcalde de barrio** (220 m,
  q 0,35, 300 pesos), **cárcel del Cabildo con cuerpo de guardia** (600 m, q 0,3, una por ciudad, a
  menos de 400 m de la plaza; disuade un 10 % en toda la ciudad mientras haya lugar para sus 30
  presos, 4 % si está llena). Delito = riesgo × (1 − vigilancia).
- **Delitos**: cada solar tiene `delito · 2 %` por mes de un delito (los comercios, el doble):
  **robo** en una casa (el hogar pierde un 20 % de sus ahorros, hasta 40 pesos), **asalto** a una
  pulpería o tienda (el dueño pierde hasta 80 pesos y el cabildo pierde alcabala) o **contrabando**
  junto al muelle. La vigilancia prende al ladrón con probabilidad `0,8 · vigilancia`; si hay cárcel
  con lugar, queda preso (salen de a un sexto por mes). Gaceta: un caso por mes cuando hay tres o
  más delitos (o a veces uno), por ejemplo «Asaltan una pulpería cerca de la plaza; la ronda prende
  al ladrón y va a la cárcel del Cabildo (4 delitos este mes)».
- **Efectos**: felicidad «Seguridad» = `0,85 − 0,6 · delito + 0,1 · vigilancia` («teme a los
  ladrones del barrio»); alcabala × `(1 − 0,3 · delito de los comercios)`; opinión de los
  comerciantes `−30 · delito` y de los vecinos `−8 · delito`; contrabando de la aduana
  `+0,25 · delito medio junto al muelle`.
- Pruebas: `el_delito_sube_con_la_pobreza_y_la_oscuridad_y_baja_con_la_vigilancia`,
  `la_oscuridad_sube_el_delito`.

## 6. Interfaz

- **Cajón de Servicios con pestañas**: Agua · Educación · Salud · Seguridad · Saneamiento ·
  Iglesia. Cada pestaña tiene sus edificios, las ordenanzas que le tocan (se aprueban o derogan
  desde la tarjeta) y, desactivados con «Época N», los de las épocas siguientes
  (`barrio::FUTUROS`).
- **Capas de datos**: Educación (casas con o sin escuela y el alcance), Salud, Saneamiento
  (insalubridad, basurales y un anillo por pozo que va de azul a rojo según lo sucio) y Delito (mapa
  de calor y la ronda de serenos, alcaldes y guardia).
- **Fichas**: de cada edificio (alumnos, atendidos, presos, basurales) y de cada solar (escuela,
  salud, insalubridad, delito y vigilancia; los baldíos con basural).
- **Población → Resumen**: niños con escuela, atendidos por la salud, insalubridad (basurales,
  pozos sucios, brotes) y delitos del año.

## 7. Épocas siguientes (datos)

| Servicio | Época | Fecha en Montevideo |
|---|---|---|
| Universidad | 2 (Ciudad Nueva) | fundada el 18 de julio de 1849 [4] |
| Colectores (saneamiento) | 2 | mediados del siglo XIX, la primera red de colectores de América Latina [5] |
| Comisaría | 2 | — |
| Agua corriente | 3 | planta de Aguas Corrientes sobre el Santa Lucía, inaugurada el 18 de julio de 1871 [6] |
| Escuela pública | 3 | Decreto Ley de Educación Común, 24 de agosto de 1877 [7] |
| Liceo, cuartel de bomberos, carros de basura (recolección municipal) | 3 | — |
| Escuela técnica, hospital moderno | 4 | — |
| Policlínica barrial | 5 | — |

Las que no tienen fecha no se verificaron todavía; no se muestran fechas en el juego.

## 8. Base histórica (solo lo verificado)

- **Escuelas**: a principios del siglo XIX había en Montevideo «un corto número de escuelas de
  primeras letras (probablemente menos de diez), todas ellas de carácter privado y solamente una
  ubicada fuera de las murallas»; en **1809** se sumó una **escuela pública administrada por el
  Cabildo**, que aseguraba **gratuidad para los alumnos pobres** y les daba los materiales; cerró
  con el sitio iniciado en 1812, se reabrió entre 1817 y 1819 con el mismo reglamento de 1809 y en
  1820 llegó a tener **más de cien estudiantes a cargo de un único maestro** [1]. De ahí la escuela
  de 100 alumnos y la ordenanza de escuelas como «el cabildo paga al maestro y los pobres van
  gratis». En el juego la escuela del cabildo está disponible desde 1726: **licencia de diseño**
  (anticipa 1809).
- **Franciscanos**: no se pudo verificar en las fuentes consultadas que los franciscanos tuvieran
  escuela de primeras letras en Montevideo ni desde cuándo; el **convento con aulas** es una
  **licencia de diseño** (la enseñanza conventual era común en la América hispana).
- **Hospital de caridad**: el hospital de Montevideo se inauguró el **17 de junio de 1788** (la
  obra empezó en 1781), por iniciativa de Mateo Vidal; lo impulsó la **Hermandad de San José y de la
  Caridad**, fundada en 1775; en 1911 se lo renombró en homenaje a Francisco Antonio Maciel (hoy
  Hospital Maciel) [2]. En el juego se puede construir antes de 1788: licencia de diseño.
- **Cárcel**: el edificio del **Cabildo y Reales Cárceles** se empezó a construir en **1804** y
  funcionó como cabildo y cárcel en la época colonial (y como penitenciaría hasta fines del siglo
  XIX) [3]. La cárcel del Cabildo del juego anticipa ese edificio (licencia de diseño); el tamaño y
  el cupo de 30 presos son estimaciones.
- **Alcaldes de barrio y serenos**: no se verificaron fechas ni funciones con fuentes; su papel en
  el juego es una estimación de diseño.
- **Saneamiento**: Montevideo fue «a mediados del siglo XIX la primera ciudad de América Latina» en
  construir una red de colectores (Red Arteaga) [5]; antes, como en toda la ciudad colonial, las
  aguas servidas iban a pozos negros (el modelo colonial del juego).
- Todos los números del modelo (capacidades, alcances, costos, pesos del delito y de la
  insalubridad, probabilidades) son **estimaciones de diseño**.

### Fuentes

1. Wilson González Demuro (2009), «La educación y la prensa periódica oriental en tiempos de
   revolución, 1814-1829», XII Jornadas Interescuelas/Departamentos de Historia, Universidad
   Nacional del Comahue. <https://www.aacademica.org/000-008/1357>
2. Wikipedia en español, «Hospital Maciel». <https://es.wikipedia.org/wiki/Hospital_Maciel>
3. Wikipedia en español, «Cabildo de Montevideo». <https://es.wikipedia.org/wiki/Cabildo_de_Montevideo>
4. Wikipedia en español, «Universidad de la República». <https://es.wikipedia.org/wiki/Universidad_de_la_Rep%C3%BAblica>
5. IMPO, Resolución 396/024 (Red Arteaga, resultando III). <https://www.impo.com.uy/bases/resoluciones/396-2024>
6. Wikipedia en inglés, «Aguas Corrientes». <https://en.wikipedia.org/wiki/Aguas_Corrientes>
7. Wikipedia en español, «José Pedro Varela». <https://es.wikipedia.org/wiki/Jos%C3%A9_Pedro_Varela>

## 9. Guardar y cargar

`ServicesData::barrio` (opcional) guarda basurales, presos, contadores de delitos, alcabala
perdida, contrabando extra y brotes; `BuildingData::contaminacion` (opcional), lo sucio de cada
pozo. Las partidas viejas cargan sin ellos. La escuela, la salud, la insalubridad, la vigilancia y
el delito de cada solar se recalculan al cargar (igual que en la partida original); los
basurales se vuelven a dibujar. Prueba: `guardar_y_cargar_el_barrio`; el recorrido
`--demo-ciudad` compara también este estado («IGUAL»).

## Pendiente

- Agentes visibles: maestro y alumnos que van a la escuela, el médico a caballo, la ronda de los
  serenos y del alcalde, las carretillas de la basura (GDD 4.5).
- Ampliaciones (más salas del hospital, segunda aula) y los edificios de las épocas siguientes.
- Alcantarillas y saneamiento por épocas (colectores, conexión de las casas, agua corriente).
- Delitos con personas (quién roba, quién va preso) en la crónica de la población.
