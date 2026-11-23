# IA de los vecinos y tránsito: diagnóstico y plan

> Documento de sistema. **Implementadas las bases comunes y la época 1, colonial** (F0, F1 y F2
> de §5, ver «Estado» abajo); el resto sigue siendo plan. Complementa a
> [agentes-y-conduccion.md](agentes-y-conduccion.md) (lo que ya está hecho),
> [poblacion.md](poblacion.md) (personas, rasgos, felicidad), al [GDD](../GDD.md) §4.3 y §4.6 y
> a los documentos de cada época en [../epocas/](../epocas/).
> Pedido: *«mejorar la IA para que tenga actitudes más realistas, sobre todo más adelante con
> el tránsito»*. Estado: **v0.2, 23 de noviembre de 2026** (plan del 18 de noviembre +
> implementación colonial).

La regla de diseño sigue siendo la de agentes-y-conduccion.md: **si un jugador sigue a cualquier
vecino durante un día, lo que hace tiene que tener sentido**. A eso este plan le suma una
segunda regla para las épocas con autos: **los atascos tienen que salir de causas que el
jugador pueda ver y arreglar** (un cruce mal resuelto, un carril que se angosta, una parada
saturada), nunca de errores de la IA (todos en el mismo carril, gente que desaparece, vehículos
que se esfuman cuando se traban).

---

## Estado de la implementación (23 de noviembre de 2026)

Código en `engine/src/city/agentes/`: `mod.rs` (simulación), `agenda.rs` (agenda encadenada,
recados y vida social), `perfil.rs` (perfil de conducta y azar por contador), `portones.rs`
(portones con aduana), `ruta.rs` (tiempos medidos y rutas), `datos.rs` + el archivo de datos
`assets/datos/transito_colonial.json` (portones, tropas, horarios por oficio, recados, clima,
cruces y adelantamiento, cada número con su estado ESTIMADO o DOCUMENTADO). La ciudad los
alimenta con `City::tick_agentes` (población, muralla, lluvia del mes, epidemia).

| Punto | Estado | Cómo |
|---|---|---|
| D1 ubicuidad | **Hecho** | Agenda encadenada: el plan del día es una lista de actividades (destino, hora mínima de salida, estancia) y la salida siguiente se programa **al llegar** (`Vecino::plan`, `Estado::salidas`). Prueba `ningun_vecino_esta_en_dos_lugares` (un día hábil y un domingo, cuadro a cuadro: un agente por vecino como máximo y toda vuelta después de su llegada). |
| D2 personas | **Hecho** | Con la población fundada, cada vecino es una persona (`CLAVE_PERSONA` + id) de `habitantes_de_lote`, con su trabajo (`lugar_de_trabajo`, o el campo, el puerto, el cabildo o la iglesia según el oficio), horario por oficio (datos), clase y ganado del hogar, parientes (visitas) y quién hace la compra del hogar. Se recalcula cada mes. Sin población, vecinos sintéticos como antes (con perfil). |
| Perfil (§3) | **Hecho** | `Perfil` de 12 bytes: prisa, prudencia, cortesía, paciencia, velocidad a pie (edad y salud), valor del tiempo (clase), flexibilidad (oficio), sociabilidad, devoción, conocimiento (años en la ciudad), caballo propio y edad. Usa v0 y T del IDM, el hueco crítico, el adelantamiento, la velocidad a pie y los recados. No usa sexo ni origen. |
| Azar por contador (D16) | **Hecho** para los vecinos | `azar(semilla, persona, día, motivo)` (*splitmix64*): plan, horarios, fidelidad a la pulpería, lluvia del día, revisión en el portón, barro. Carretas y carros siguen con la secuencia guardada. |
| Personajes = personas | **Hecho** | El agente lleva la persona; `dibujo::quien` la viste (sexo, edad, oficio, riqueza) con `ropa::vestir`. Prueba `los_agentes_son_las_personas_y_se_visten_como_ellas`. |
| D5–D7 tiempos | **Hecho** | Se guarda la **demora relativa** (tardó / libre de ese tipo de vehículo), **por franja** (madrugada, mañana, tarde, noche) y **con olvido** exponencial hacia lo libre (τ = medio día de juego). Las rutas usan una **foto por hora del reloj** (misma ruta con o sin caché, y la partida cargada elige igual). Pruebas `los_tiempos_olvidan_un_atasco_viejo`, `el_jinete_no_evita_la_calle_que_dejaron_las_carretas`. |
| D3 escala | Opción A | La ficha muestra la duración física y la del reloj aparte. |
| D4 destino | En parte | Con persona, el trabajo es el suyo; sin persona, el comercio o taller sale con peso exp(−d/600 m) (gravitatorio). |
| D15 desaparecer | **Hecho** | Si cambia la red, el agente recalcula desde donde está (`Eleccion::Recalculada`); solo si no hay camino termina el viaje ahí (estadística `perdidos`). |
| Portones con aduana | **Hecho** | Punto de servicio por portón terminado (`City::portones_terminados`): cola FIFO por sentido (la fila del carril), revisión según la carga (datos), derecho cobrado, cierre de noche (6 a 21 h, ESTIMADO), espera media, máximo de cola, contrabando estimado. Pruebas `el_porton_forma_cola_y_cobra`, `de_noche_el_porton_esta_cerrado`, `los_contrabandistas_evitan_el_porton`. |
| Contrabandistas | **Hecho** | Una fracción de las carretas (datos) busca una ruta sin tramos con portón; si no hay, entra por el portón. |
| Arreos | **Hecho** | `Tipo::Arreo`: tropa de 40 a 160 cabezas (0,6 m por cabeza), 3 km/h, del campo a los corrales de extramuros por rutas que no tocan tramos intramuros (radio de los datos o la distancia a la muralla); nadie la adelanta; los troperos vuelven a caballo. Dibujo: vacas en columnas de tres y troperos a los costados (`dibujo::tropa`). Pruebas `el_arreo_no_entra_intramuros`, `nadie_adelanta_un_arreo_pero_si_una_carreta`. |
| Adelantamiento | **Hecho** | Jinete con prudencia < 0,6 detrás de una carreta o un carro lento, con el carril contrario libre 40 + 60·prudencia m, lugar para volver y sin portón ni cruce antes de terminar. |
| Prioridad en cruces | **Hecho** | En los cruces con calle empedrada y de tierra, pasan primero los de la empedrada; los de la de tierra esperan en la línea un hueco ≥ hueco crítico (+ lo que tardan en despejar el cruce) que baja con la espera; la salvaguarda de un minuto (y 90 s para el hueco) sigue. Prueba `en_el_cruce_la_calle_empedrada_tiene_prioridad`. |
| Peatones que cruzan | **Hecho** | En las esquinas esperan un hueco (según su prudencia) frente al primero de cada fila que llega; los vehículos frenan antes del cruce si hay alguien en la calzada. |
| Clima | **Hecho** | Si llueve se sortea por día con los días de lluvia del mes; con lluvia bajan las salidas optativas (más los prudentes), a pie más lento; con dos días seguidos, barro: carretas más lentas en la tierra y algunas se empantanan unos minutos. Epidemia: salidas optativas a la mitad. Prueba `con_lluvia_salen_menos_a_pasear`. |
| Vida social | **Hecho** | Compra en la pulpería del barrio (una de las tres más cercanas, con fidelidad), recova, pulpería a la vuelta del trabajo (sociabilidad), misa del domingo y de semana (devoción), paseo de la plaza el domingo, visitas a parientes. |
| UI | **Hecho** | Ficha del portón (abierto o cerrado, cola por sentido, espera media, pasados, derechos, contrabando); capa «Cola en portones»; ficha del vecino con su persona, carácter, cómo eligió la ruta y la **agenda del día** (lo hecho con sus horas y lo que falta). |
| Pendiente | — | Aguadores y serenos como agentes; incendios (curiosos y baldes), delito de noche y protestas; carretas empantanadas que esperan ayuda; salida de misa con fuerza social; colas por carril en lugar de ordenar todo (D16, rendimiento); D8–D13 de las épocas con autos. |

**Mediciones** (prueba `rendimiento_con_el_tope_de_agentes`, 3.000 agentes al azar en la
ciudad de prueba, compilación optimizada, máquina compartida con carga ~8 en 4 núcleos):
**0,22–0,25 ms por paso** sin foco y 0,22 con el foco lejos (todos de a saltos); con el foco
en la plaza (todos cerca, con la posición anterior para el dibujo) 0,47–0,52 ms, como antes de
este cambio (lo que pesa es la pose de cada agente). En la demostración (`--demo-ciudad`,
~1.950 vecinos-personas, 450 en la calle a las 7:30) la simulación cuesta 0,22–0,27 ms por cuadro de media. Recorrido de la IA: `MOTOR_SOLO_IA=1 motor --demo-ciudad <dir>` (portón con cola, ficha del portón, capa, arreo de 111 cabezas sin tramos intramuros, ficha de un vecino con su agenda; guardar y cargar IGUAL). Capturas en `docs/capturas/ia-*.jpg`.

## 0. Resumen

**Lo más importante para la época colonial, ya:**

1. **Arreglar la «ubicuidad»**: hoy un vecino puede estar en la calle dos veces a la vez (la
   vuelta del mercado o de misa sale a hora fija aunque la ida todavía no llegó). Pasar a una
   **agenda encadenada**: la actividad siguiente se programa al llegar, con su duración (ver D1).
2. **Conectar los agentes con la población real** (`Poblacion::habitantes_de_lote`,
   `lugar_de_trabajo`, rasgos, edad, clase, oficio, felicidad) y derivar de cada persona un
   **perfil de conducta** compacto y determinista (§3).
3. **Tiempos medidos que olvidan y que distinguen**: hoy un atasco medido una vez queda para
   siempre si nadie vuelve a pasar, y lo que tarda una carreta encarece la calle para los
   jinetes. Media con olvido, por franja horaria y normalizada por tipo (D5–D7).
4. **Los portones de la muralla con aduana** como primer cuello de botella jugable (cola visible,
   tiempo de revisión según la carga, horario de cierre, contrabandistas que los evitan), los
   **arreos de ganado** por los caminos de afuera y el **adelantamiento** de jinetes ya diseñado.
5. **Recados y vida social**: pulpería, recova, aguador, misa y paseo de la plaza encadenados a
   la rutina según rasgos (sociabilidad, devoción, laboriosidad), con reacción al clima (barro,
   lluvia), a incendios y a epidemias.

**Lo más importante para el tránsito de las épocas siguientes:**

1. **Decidir la escala temporal** antes de los autos: hoy un viaje dura 18 veces más en el
   reloj del juego que en la realidad, así que en hora pico casi toda la ciudad estaría en la
   calle a la vez (D3, §4.0).
2. **Calles con carriles y conectores de carril**, cambio de carril **MOBIL** con elección
   anticipada del carril para el giro (lo que más le falla al referente del género).
3. **Cruces con reglas**: prioridad (principal/secundaria, PARE, ceda), **semáforos con fases**
   y **rotondas** con aceptación de huecos; reservas por zona de conflicto en lugar de «uno a
   la vez por cruce».
4. **Simulación híbrida**: micro (IDM + MOBIL) cerca de la cámara, **mesoscópica de colas**
   (como MATSim) en el resto, con los mismos tiempos de paso; sin hacer desaparecer a nadie.
5. **Demanda por actividades y elección modal** (logit con inercia, costo según ingreso),
   **transporte público con líneas, paradas, frecuencias y tiempos de ascenso**, estacionamiento,
   aprendizaje día a día hacia un equilibrio estocástico y heterogeneidad de conductores.

---

## 1. Diagnóstico del modelo actual

Lo que hay está en `engine/src/city/agentes/` (`mod.rs`, `idm.rs`, `ruta.rs`, `dibujo.rs`,
`tests.rs`) y la red en `engine/src/city/roads.rs`. Números de línea de la versión v0.9.0
(`db47905`).

### 1.1 Lo que ya es realista (y hay que conservar)

| Qué | Dónde | Comentario |
|---|---|---|
| IDM de Treiber con la fórmula completa (exponente 4, distancia deseada `s0 + vT + vΔv/2√(ab)`) | `idm.rs:71-80` | Correcto. Frenada limitada a `−4b`. |
| Integración que nunca superpone vehículos (avance ≤ hueco − 0,2 m) | `idm.rs:84-92` | Seguro; pierde algo de física (ver D12). |
| Parámetros por tipo (a pie, jinete, carreta, carro) y temperamento (velocidad, separación) | `idm.rs:38-45`, `mod.rs:53` | Buena base para la heterogeneidad. |
| Filas por carril ordenadas, seguir al último del carril siguiente | `mod.rs:810-821`, `896-906` | El «efecto cola» que cruza el cruce está bien resuelto. |
| Cruces de 3+ calles por orden de llegada, sin bloquear el cruce (salida libre) y salvaguarda de un minuto | `mod.rs:823-873` | Evita *gridlock* en la colonia. |
| Curvas por aceleración lateral (1,2 m/s²), freno antes de doblar, tierra más lenta | `mod.rs:928-933`, `1262-1283` | Comportamiento creíble. |
| Ruta por tiempo **medido** (media móvil), percepción personal ±15 % en 8 variantes | `ruta.rs:62-114` | Reparte el tránsito sin azar por cuadro. |
| Aprendizaje: si tardó > 1,5× lo previsto, cambia de semilla de ruta | `mod.rs:1015-1024` | Idea correcta (inercia + cambio ante mala experiencia). |
| Casa y trabajo fijos, horario habitual con variación diaria, misa del domingo | `mod.rs:1134-1188` | Evita el «trabajo libre más cercano». |
| Determinismo total y estado guardado; prueba de un día sin choques | `mod.rs:16-18`, `tests.rs:136` | Imprescindible; todo lo nuevo tiene que mantenerlo. |
| Legibilidad: ficha del agente y de la calle (medido frente a libre) | `mod.rs:1099-1209` | Base de las capas de tránsito. |

### 1.2 Lo que es simplista o está mal (por gravedad)

**D1. Un vecino puede estar dos veces en la calle (error).** `viaje_de_vecino`
(`mod.rs:658-685`) marca `lugar` al **salir**, y las vueltas están en la agenda a hora fija
(`rutina`, `mod.rs:1183-1186`: mercado + 0,8–1,5 h; misa + 1,8–2,4 h). No se comprueba si el
vecino ya tiene un agente en camino. Con la escala temporal actual (D3), 0,8 h de reloj son
160 s simulados: **cualquier ida a pie al mercado de más de ~220 m** (o a misa de más de
~500 m) larga la vuelta antes de llegar, y el vecino aparece caminando en los dos sentidos. La
prueba `un_dia_de_la_ciudad_sin_choques_y_determinista` no lo detecta.

**D2. Los agentes no son las personas de la población.** `habitante_de_lote`
(`mod.rs:1134-1164`) inventa nombre, oficio, rol (30 % sale, sin mirar la edad), caballo
(40 % si el solar es nivel ≥ 3, si no 8 %) y temperamento al azar del solar. La población del
motor ya tiene `Persona` con edad, salud, riqueza, `Rasgos` (laboriosidad, ambición, devoción,
sociabilidad, prudencia), oficio, clase, `trabajo` y felicidad, y la API
`habitantes_de_lote` / `lugar_de_trabajo` está lista (`poblacion/mod.rs`, marcada
`allow(dead_code)`); `Agentes::asignar_trabajo` también está sin usar (`mod.rs:1043`). El
pendiente está anotado en poblacion.md §13.

**D3. Escala temporal: los viajes duran 18 veces más en el reloj.** El día dura 480 s reales
(`session.rs:137`) y los agentes se mueven a 10× (`ESCALA_TIEMPO`, `mod.rs:37`): 1 s simulado
= 0,3 min de reloj (`MINUTOS_POR_SEGUNDO`). A pie (1,35 m/s) se hacen **270 m por hora de
reloj**: cruzar la ciudad intramuros (~1 km) lleva casi 4 horas del juego. Consecuencias:
(a) D1; (b) la ficha muestra «355 min» para una caminata real de 20; (c) en épocas con autos,
la **cantidad de gente en la calle a la vez** es proporcional a la duración en reloj: con el
factor 18 la hora pico junta prácticamente a todos los que viajan esa mañana. Es el mismo
dilema que tiene el referente del género (día visual corto frente a viajes físicos), y hay que
decidirlo antes de la época 4 (§4.0).

**D4. Demanda pobre y sin encadenar.** Solo trabajo (lunes a sábado), mercado (12–20 %) y misa
(60 %); ninguna cadena (trabajo → pulpería → casa), ninguna salida de noche, ningún recado del
hogar. El trabajo se elige **al azar entre todos los comercios y talleres** (`mod.rs:545-553`),
sin peso por distancia: viajes larguísimos poco creíbles (en las ciudades reales la
probabilidad de un destino cae con la distancia: modelo gravitatorio).

**D5. Los tiempos medidos no olvidan.** `Tiempos::registrar` (`ruta.rs:79-86`) es una media
móvil (α = 0,25) que solo cambia cuando alguien recorre el carril **entero**. Si un carril se
atascó una vez y por eso nadie vuelve a elegirlo, queda «atascado» para siempre (evitación
fantasma). Falta un olvido hacia el tiempo libre con el paso del tiempo.

**D6. Un solo tiempo para todos los vehículos.** Se registra el tiempo de cualquier vehículo
(`mod.rs:984`): una calle que usan las carretas (1,1 m/s) queda medida a velocidad de carreta y
los jinetes la evitan aunque las carretas ya se hayan ido. Con autos y ómnibus será peor
(un ómnibus que para en cada parada encarece la calle para los autos). Hace falta el tiempo
**por clase** o, mejor, la **demora relativa** (medido / libre de ese tipo).

**D7. Un solo tiempo para todo el día.** La media no distingue la hora pico de la madrugada, y
la caché se vacía una vez por día (`mod.rs:585`). Los vecinos «saben» del atasco de las 8 a
las 3 de la tarde.

**D8. Cruces: uno a la vez, sin prioridad.** En todo nodo de grado ≥ 3 pasa **un vehículo
por vez** (`mod.rs:834-873`), sin importar que los movimientos no se crucen (dos que doblan a
la derecha desde lados opuestos), sin calle principal ni regla de la derecha. Alcanza para la
colonia (pocas carretas), pero la capacidad de un cruce queda en unos pocos cientos de
vehículos por hora, muy por debajo de los ~1.800–1.900 por hora y carril de un semáforo real
(flujo de saturación base del *Highway Capacity Manual*).

**D9. Distancias fijas pensadas para bueyes.** `ACERCAMIENTO = 22 m` (`mod.rs:45`) y la
velocidad de giro `1,6 m/s` para todos (`mod.rs:931-933`). Un auto a 50 km/h necesita
~50 m para frenar con 2 m/s²: con 22 m frenaría en seco (topeado en −4b). Tienen que
depender de la velocidad y del radio del giro.

**D10. Una sola fila por sentido y nada de carriles.** `RoadKind` solo tiene `Dirt` y `Stone`
(`roads.rs:14-17`); no hay cantidad de carriles, sentido único, vías de tranvía ni
estacionamiento. El adelantamiento de jinetes está diseñado pero no portado (pendiente en
agentes-y-conduccion.md).

**D11. Peatones fantasma.** Velocidad constante (`mod.rs:884-886`), no se ven entre sí, no
cruzan la calzada en las esquinas y los vehículos no les ceden el paso; el desvío lateral sale
de `id % 3` (`mod.rs:1256`).

**D12. Detalles del IDM.** El recorte del avance al hueco (`idm.rs:89`) puede «frenar» de
golpe sin desaceleración (la velocidad nueva sale de `ds/dt`), lo que oculta frenadas
imposibles; no hay tiempo de reacción ni error de estimación (no aparecen las ondas de
*stop-and-go* que sí se ven en la realidad).

**D13. Aprendizaje a ciegas.** Ante un viaje malo se sortea otra semilla de ruta
(`mod.rs:1016-1022`): puede caer en una ruta peor y no recuerda cuáles probó. `previsto`
(`mod.rs:744`) es el tiempo a velocidad deseada sin cruces, así que «demorado» mezcla congestión
con esperas normales.

**D14. Elección de medio fija.** A pie si el destino está a menos de 150 m, si no el «medio» del
vecino (`mod.rs:681`). No hay costo, clima, carga, edad ni hora.

**D15. Agentes que desaparecen.** Si la red cambia, el agente cuyo camino ya no existe se borra
(`mod.rs:496`, `Avance::Perdido`), y con el tope de 3.000 los viajes se «omiten» (`mod.rs:730`).
Se cuenta en estadísticas, pero el vecino no llega a ningún lado (no se reubica ni recalcula).

**D16. Rendimiento y azar.** Cada paso ordena **todos** los vehículos y arma un `HashMap` de
rangos (`mod.rs:812-819`), también los lejanos, que solo avanzan uno de cada cuatro pasos: el
nivel de detalle casi no ahorra (lo dice el propio documento). Con una sola fila por carril y
sin adelantamientos, el orden dentro de un carril solo cambia al entrar: bastaría una cola por
carril. Además, todo el azar sale de **una sola secuencia** (`e.azar`): agregar un sorteo nuevo
en cualquier lado corre todos los demás (cambian las pruebas de oro y los vecinos de las
partidas viejas). Conviene un azar por contador: `hash(persona, día, motivo)`.

**D17. Heurística del A\*.** Usa 4,5 m/s (`ruta.rs:142-149`), válida para la colonia. Con autos
sigue siendo admisible pero el A\* degenera en Dijkstra (explora todo); hace falta la
velocidad máxima de la red por clase y, para ciudades grandes, técnicas como ALT (*landmarks*)
o jerarquías de contracción (§2.6).

---

## 2. Estado del arte, adaptado a un juego

Para cada modelo: qué es, qué tomar y cuánto cuesta. Las referencias completas están en §7.

### 2.1 Seguimiento de vehículos (*car-following*)

- **IDM** (Treiber, Hennecke y Helbing 2000): libre de choques con parámetros sensatos,
  pocos parámetros con sentido físico (`v0, T, s0, a, b`). **Se queda.** Mejoras del mismo
  grupo: **IDM+ / ACC-IDM** (Kesting, Treiber y Helbing 2010) que reacciona menos de más ante
  los que se meten adelante, y el **Human Driver Model** (Treiber, Kesting y Helbing 2006):
  tiempo de reacción, errores de estimación y anticipación a varios vehículos, que produce
  ondas de *stop-and-go* realistas. Integración «balística» recomendada por Treiber y Kanagaraj
  (2015) para pasos de 0,25–0,5 s.
- **Gipps** (1981): velocidad segura con tiempo de reacción explícito; base de AIMSUN. Útil como
  referencia de la «velocidad segura» para frenar ante semáforos.
- **Krauss** (1998), el modelo por defecto de **SUMO**: Gipps simplificado con imperfección del
  conductor (`sigma`, una frenada aleatoria) que reproduce atascos espontáneos con muy pocas
  cuentas. Muy barato; buena alternativa para el nivel de detalle intermedio.
- **Atascos sin cuello de botella**: el experimento de Sugiyama et al. (2008), 22 autos en un
  círculo, muestra que una fila densa genera ondas solas. Es la prueba de calibración ideal: con
  el HDM o con el ruido de Krauss tienen que aparecer; con el IDM determinista, no.

**Para el juego:** IDM con parámetros por persona (§3) + un ruido pequeño y determinista
(estilo Krauss) o el tiempo de reacción del HDM solo en épocas con autos.

### 2.2 Cambio de carril

- **MOBIL** (Kesting, Treiber y Helbing 2007): cambia si la ganancia propia menos `p` (el
  factor de **cortesía**) × la pérdida que causa a los demás supera un umbral, y solo si el
  nuevo seguidor no tiene que frenar más que `b_safe`. Se calcula con el mismo IDM, así que es
  barato y coherente. `p` es un rasgo de personalidad perfecto (egoísta ≈ 0, cortés ≈ 0,5).
- **Gipps (1986)**: cambio de carril por necesidad (giro próximo, carril que se termina,
  obstáculo) con zonas: lejos del giro, cambio libre; cerca, obligatorio y con huecos cada vez
  más chicos. **Para el juego:** MOBIL para los cambios discrecionales + la lógica de Gipps
  para los obligatorios, con una **distancia de anticipación** al giro (p. ej. 150–300 m según
  la velocidad, más larga para los prudentes y los que no conocen la zona).
- Para el adelantamiento por el carril contrario (jinete y carreta, diligencias): la regla ya
  diseñada (hueco libre de varias decenas de metros, depende del temperamento).

### 2.3 Peatones

- **Fuerza social** (Helbing y Molnár 1995): cada peatón empujado hacia su destino y repelido
  por los demás y por las paredes; reproduce carriles espontáneos en multitudes y cuellos de
  botella en puertas. Caro (vecinos cercanos por cuadro). Alternativas de juego: **ORCA**
  (van den Berg et al. 2011) o *continuum crowds* (Treuille et al. 2006).
- **Diagrama fundamental del peatón** (Weidmann 1993): velocidad libre media ~1,34 m/s que cae
  con la densidad hasta detenerse cerca de 5,4 personas/m². Barato: alcanza con la densidad de
  cada tramo de vereda.
- Velocidades por edad (Knoblauch, Pietrucha y Nitzburg 1996): los mayores cruzan sensiblemente
  más despacio que los jóvenes (del orden de 1,25 frente a 1,5 m/s al cruzar).
- **Para el juego:** veredas como «carriles» con velocidad según densidad (Weidmann) y
  aceptación de huecos para cruzar; fuerza social solo en lugares y momentos con multitud y
  pocos agentes (la plaza en una fiesta, la salida de misa, el estadio) y cerca de la cámara.

### 2.4 Demanda por actividades

- **MATSim** (Horni, Nagel y Axhausen 2016): cada agente tiene un **plan diario** (actividades
  con lugar, hora y duración, y viajes entre ellas), se simula todo el día, se puntúa el plan
  (utilidad de las actividades menos el costo de viajar, penalidades por llegar tarde) y al día
  siguiente una parte de los agentes modifica su plan (ruta, hora, modo). Es exactamente el
  bucle «día a día» que el juego necesita, y es determinista si el azar lo es.
- **Cadenas de viajes / tours** (Bowman y Ben-Akiva 2001): el día se organiza en giras desde
  la casa (casa → trabajo → pulpería → casa); el modo se elige por gira (si sale a caballo, vuelve
  a caballo).
- **Hora de salida**: el modelo del cuello de botella de Vickrey (1969) y las penalidades por
  llegar antes o tarde (Small 1982): con congestión, la gente adelanta la salida y la hora
  pico se **ensancha** sola. Es la forma realista de que el tránsito se reparta en el tiempo.
- **Elección de destinos**: modelos gravitatorios / logit de destino con caída por distancia.

### 2.5 Elección de medio (modo)

- **Logit multinomial** (McFadden 1974; Ben-Akiva y Lerman 1985): probabilidad de cada modo ∝
  `exp(U)`, con `U = −β_t·tiempo − β_c·costo/ingreso − β_e·espera − β_c·caminata + comodidad +
  constante del modo`. Los tiempos de espera y de caminata pesan más que el tiempo a bordo
  (típicamente 1,5–2,5 veces en la literatura). **Inercia**: un bonus al modo de ayer (hábito).
- **Propiedad del vehículo** como decisión de largo plazo (por hogar, una vez por año), no por
  viaje: evita los «autos de bolsillo» (§2.12).

### 2.6 Rutas con congestión y aprendizaje

- **Equilibrio de Wardrop** (1952): nadie puede mejorar cambiando de ruta. **Equilibrio
  estocástico** (Daganzo y Sheffi 1977): como cada uno percibe los costos con error, se reparten
  según un logit; justamente lo que hace la percepción ±15 % actual.
- **Aprendizaje día a día** (Horowitz 1984; Cascetta 1989): cada uno mezcla lo que esperaba con
  lo que vivió, y solo una fracción cambia cada día. Con todos cambiando a la vez aparecen
  oscilaciones (todos a la calle B, que se atasca, todos de vuelta a la A).
- **Para el juego:** que cada vecino recuerde **2–3 rutas** por viaje habitual con su costo
  vivido (media con olvido), elija con un logit con inercia, y solo explore (A\* con su
  percepción) con baja probabilidad o tras un viaje malo. Medir la **brecha relativa** (*relative
  gap*) como indicador de convergencia.
- **Búsqueda de caminos a escala**: A\* con heurística ALT (Goldberg y Harrelson 2005) o
  jerarquías de contracción (Geisberger et al. 2008) con pesos que cambian (versión
  *customizable*, Dibbelt, Strasser y Wagner 2016) cuando la red llegue a decenas de miles de
  tramos.

### 2.7 Control de cruces

- **Prioridad y aceptación de huecos**: quien viene por la secundaria espera un hueco mayor
  que el **hueco crítico** (Raff y Hart 1950); el hueco aceptado **baja con la espera**
  (Mahmassani y Sheffi 1981): la impaciencia es medible y es un rasgo jugable.
- **Semáforos**: ciclo, fases y verdes; tiempos óptimos de Webster (1958) para planes fijos;
  más tarde, semáforos actuados (detectores) y **onda verde** en avenidas.
- **Rotondas**: los que entran ceden a los que circulan; capacidad según el flujo que circula
  (modelos de aceptación de huecos del HCM).
- **Zonas de conflicto**: en lugar de «uno por cruce», cada movimiento (entrada → salida) ocupa
  unas celdas del cruce; dos movimientos que no comparten celdas pasan a la vez. Es lo que
  hacen, con distintos nombres, SUMO (*foes* entre conexiones) y los simuladores de
  intersecciones autónomas (Dresner y Stone 2008, reservas de espacio-tiempo).

### 2.8 Estacionamiento

- Shoup (2006) revisó estudios en los que, en centros congestionados, una parte importante del
  tránsito (el promedio de esos estudios es cercano al 30 %, cifra discutida) estaba **buscando
  lugar**. En MATSim hay módulos de búsqueda de estacionamiento (Waraich y Axhausen 2012).
- **Para el juego:** el auto es persistente; al llegar busca lugar en la cuadra, luego en las
  vecinas (en espiral), o va a un garaje con tarifa. El tiempo de búsqueda y la caminata final
  cuentan en el viaje y en la elección del modo. En la colonia: **dónde atar el caballo** y
  dónde dejar la carreta (los huecos de carretas junto a la recova y las barracas).

### 2.9 Transporte público

- Espera media ≈ la mitad del intervalo para llegadas al azar; con horarios conocidos (intervalos
  largos) la gente llega a la hora (Osuna y Newell 1972 y siguientes).
- **Tiempo en la parada** (*dwell*) = apertura/cierre + pasajeros × segundos por pasajero
  (*Transit Capacity and Quality of Service Manual*, TCRP 165); más alto si se paga a bordo.
- **Amontonamiento** (*bus bunching*, Newell y Potts 1964): el atrasado junta más gente, se
  atrasa más, y el de atrás lo alcanza. Aparece solo si se modela el *dwell* por pasajero; es un
  problema real y legible que el jugador puede combatir (carriles exclusivos, cobro antes de
  subir, regulación de intervalos).
- Capacidad y hacinamiento: más allá de la capacidad, la gente espera el siguiente; el
  hacinamiento resta comodidad en el logit.

### 2.10 Heterogeneidad de conductores

Treiber y Kesting (2013, *Traffic Flow Dynamics*) muestran que con distribuciones de `v0` y `T`
la capacidad y la estabilidad cambian notablemente. Rasgos útiles: velocidad deseada,
separación (`T`), aceleración y frenada cómodas, tiempo de reacción, cortesía (MOBIL `p`),
hueco crítico e impaciencia, **cumplimiento** (respetar el rojo, el PARE, el límite) y
familiaridad con la zona (anticipa el carril).

### 2.11 Choques y cuasi-choques

Simular choques con física no aporta y rompe la garantía de «sin superposición». Lo estándar
en seguridad vial es medir **indicadores sustitutos**: tiempo hasta la colisión (TTC; Hayward
1972) y frenadas fuertes; el SSAM de la FHWA (Gettman et al. 2008) los saca de simulaciones.
**Para el juego:** contar cuasi-choques por cruce (TTC < 1,5 s, frenada > 4 m/s², rojo pasado)
→ capa de **puntos negros**; los **accidentes** son eventos raros con probabilidad ∝
cuasi-choques × exposición, que cortan un carril unos minutos y mandan a la policía o a la
ambulancia. En la colonia: carreta volcada, caballo desbocado, eje roto en el barro.

### 2.12 Cómo lo resuelven otros juegos (y sus errores conocidos)

- ***SimCity* (2013)**: los habitantes no eran persistentes: cada día iban al trabajo libre o a
  la casa libre más cercanos, y la búsqueda de caminos privilegiaba el camino más corto, lo
  que formaba filas absurdas; fue muy criticado y Maxis corrigió parte con parches. Es el fallo
  que nuestro diseño ya evita con casa y trabajo fijos.
- ***Cities: Skylines* (2015)**: los vehículos de la versión base cambian de carril
  prácticamente solo en los nodos (al empezar un tramo), lo que concentra los cambios y forma
  filas en un carril mientras el otro va vacío; la búsqueda de rutas casi no mira la congestión;
  los vehículos trabados **desaparecen** (*despawn*), lo que esconde los *gridlocks*; y los
  ciudadanos sacan un auto «del bolsillo» al salir de un edificio sin haberlo estacionado. El mod
  comunitario *Traffic Manager: President Edition* (TM:PE) agrega conectores y flechas de carril,
  señales de prioridad, semáforos programables, rutas sensibles a la congestión, la opción de no
  hacer desaparecer vehículos y un modelo de estacionamiento. La lección: lo que los jugadores
  más reclamaron son **herramientas de control de cruces y carriles** y una IA de carriles que
  anticipe.
- ***Cities: Skylines II* (2023)**: estacionamiento simulado, cambio de carril a lo largo del
  tramo y un costo de ruta que combina tiempo, dinero, comodidad y «comportamiento» con pesos por
  ciudadano (según los diarios de desarrollo). Tuvo críticas fuertes de rendimiento al salir y
  quejas sobre comportamientos de carril, en parte corregidas después. Lección: costos por
  persona sí, pero con **capas de depuración** que muestren por qué alguien eligió una ruta.
- ***A/B Street*** (código abierto, en Rust, sobre mapas reales de OSM): simulación por eventos
  discretos a nivel de carril, con semáforos editables y *gridlock* visible; su autor documenta
  lo difícil que es evitar bloqueos en redes reales. Es una referencia directa para nuestro
  nivel mesoscópico y para el editor de cruces.

---

## 3. Actitudes realistas: de la persona a la conducta

### 3.1 Perfil de conducta

Cada vecino con persona (`Persona` de `poblacion/modelo.rs`) tiene un **perfil** de 16 bytes
que se recalcula al cerrar cada mes (cuando cambian edad, salud, riqueza, felicidad) y cuando
cambia de casa o de trabajo. Nada se sortea por cuadro: todo sale de la persona y de un azar
por contador `h(persona, rasgo)`, así que es determinista, barato y no corre otros sorteos.

```text
Perfil {
  prisa:       u8   // apuro estructural: ambición, laboriosidad, oficio con horario rígido
  prudencia:   u8   // rasgo prudencia (+ edad): separación, frenada, huecos, cumplimiento
  cortesia:    u8   // MOBIL p: (sociabilidad + prudencia)/2, baja con la infelicidad
  paciencia:   u8   // segundos de espera antes de bajar el hueco crítico
  v_pie:       u8   // velocidad a pie (cm/s/2): edad, salud, carga
  valor_tiempo:u8   // cuánto vale el tiempo frente al costo: ingreso del hogar
  flexibilidad:u8   // margen de horario según el oficio
  social:      u8   // probabilidad de paradas sociales (pulpería, plaza, visitas)
  devocion:    u8   // misa de semana, procesiones
  conocimiento:u8   // años en la ciudad: anticipación de carril, rutas conocidas
  medios:      u8   // bits: tiene caballo, carro, (más tarde) bici, auto, abono
  ...
}
```

### 3.2 Tabla de correspondencias

| Atributo de la persona | Conducta | Fórmula orientativa (determinista) |
|---|---|---|
| **Prudencia** | Estilo de manejo | `T × (0,8 + 0,6·prud)`, `s0 × (0,8 + 0,5·prud)`, `b × (1,1 − 0,3·prud)`; adelanta solo si `prud < 0,6` y el hueco es ≥ `40 m + 60·prud`; hueco crítico `+1,5·prud` s; cumplimiento (rojo, PARE) `0,85 + 0,15·prud` |
| **Ambición, laboriosidad** | Prisa | `v0 × (0,95 + 0,1·prisa)`; sale antes; acepta huecos más cortos cuando va tarde |
| **Sociabilidad** | Paradas sociales | Pulpería a la vuelta del trabajo, mate en la vereda, paseo de la plaza el domingo, visitas a parientes (la población tiene familias: visitar a padres o hijos casados) |
| **Devoción** | Misa y fiestas | Misa el domingo con probabilidad ∝ devoción; misa de semana para los muy devotos; procesiones que cortan calles |
| **Edad** | Velocidad a pie, autonomía | Niños con un adulto (o a la escuela en grupo), a pie ~1,1 m/s; adultos 1,3–1,45; mayores 1,0–1,2; los mayores no salen de noche ni con temporal; nadie menor de 12 sale solo a caballo |
| **Salud** | Velocidad y salidas | `v_pie × (0,7 + 0,3·salud)`; enfermo: no va a trabajar, va al hospital o lo visitan |
| **Riqueza / clase** | Medios, valor del tiempo | Caballo propio (y más tarde carruaje, auto) por hogar; `valor_tiempo` ∝ ingreso: los ricos pagan peaje, diligencia o taxi; los pobres caminan más |
| **Oficio** | Horario y destinos | Panaderos y labradores muy temprano; aguadores con recorrido; funcionarios y soldados con horario rígido; artesanos en casa con salidas cortas; marineros según los barcos; peones del campo a caballo por el portón; pulperos que abren a la madrugada y cierran tarde |
| **Felicidad** | Humor al manejar y salir | Infeliz: cortesía y paciencia −20 %; más pulpería, menos misa (o al revés según devoción); con >35 % de descontentos, **protesta**: grupo que marcha a la plaza o al cabildo y corta el tránsito |
| **Conocimiento** (años en la ciudad, origen) | Rutas y carriles | El recién llegado usa calles principales, anticipa menos el carril y no conoce atajos; el criollo usa la percepción de ruta más fina |
| **Hogar** | Recados compartidos | Una persona por hogar hace la compra diaria (en hogares pudientes, quien tiene oficio de servicio); los chicos acompañan |

> **Decisiones de cuidado:** no se usa el sexo ni el origen para el estilo de manejo ni para la
> velocidad. Las diferencias de rutina por época (quién iba a la pulpería, quién salía de noche)
> se modelan como **costumbres de la época** configurables en datos, no como rasgos de grupo, y
> sin juicios. Coherente con la decisión del GDD de representar a todas las personas como
> libres.

### 3.3 Conducta según la situación

- **Impaciencia**: al esperar en un cruce, el hueco crítico baja `0,5 s` por cada `paciencia` s
  de espera, hasta un mínimo (Mahmassani y Sheffi 1981). Reemplaza en parte a la salvaguarda de
  un minuto, que queda como red de seguridad.
- **Llegar tarde**: si el tiempo previsto ya no alcanza, `v0 + 5 %`, menor separación y hueco
  crítico más corto (acotado por la prudencia). Los prudentes salen antes en vez de apurarse.
- **Clima** (de `climate.rs`): con lluvia, los viajes **optativos** (paseo, visita, pulpería)
  bajan según la prudencia; a pie, más lento y buscando la calle empedrada; en tierra mojada
  `v0 × 0,6` para carretas (y probabilidad de **empantanarse** en el barro tras varios días de
  lluvia: la carreta queda detenida hasta que llegan otros a sacarla); con temporal, casi nadie
  sale. Calor de verano: los paseos se mudan al atardecer.
- **Noche**: sin faroles, los prudentes no salen o evitan calles oscuras y barrios con delito
  alto (el delito ya es por solar, `services/barrio.rs`); el sereno hace su ronda.
- **Incendio**: los curiosos (sociabilidad alta, prudencia baja) se acercan hasta un radio, los
  vecinos con baldes acuden (en la colonia el fuego lo apagan los vecinos), los demás desvían.
  La calle frente al incendio se corta.
- **Epidemia**: viajes optativos −50 % (más entre los prudentes), cuarentena del puerto y del
  lazareto (sus viajes se cortan), cortejos fúnebres.
- **Delito**: se evitan de noche los tramos con delito alto; los comerciantes ricos de noche
  van acompañados o a caballo.
- **Imprevistos**: calle cortada (obra, inundación del sistema de agua/terreno) → los agentes
  que la tienen en su ruta **recalculan en el momento** desde el próximo cruce (en lugar de
  desaparecer, D15).

### 3.4 Costo

- Perfil: 16 bytes por vecino, recalculado mensualmente (una pasada lineal, < 1 ms para
  100.000 personas).
- Parámetros IDM del agente: se derivan al salir (ya hay `v0` y `t` por agente).
- La agenda encadenada se evalúa por eventos (al llegar), no por cuadro.
- La única cuenta por paso nueva en la colonia es la aceptación de huecos en los cruces, que ya
  recorre solo al primero de cada fila.

---

## 4. Hoja de ruta por época

### 4.0 Bases comunes a todas las épocas

**Escala temporal (decisión a tomar antes de la época 4).** Hoy el factor es 18 (§1.2 D3).
Opciones:

| Opción | Efecto | Costo |
|---|---|---|
| A. Dejarlo y compensar (salidas anticipadas, agenda encadenada, mostrar duraciones físicas en las fichas) | Simple; la hora pico se llena demasiado en épocas modernas | Ninguno |
| B. Subir la escala de los agentes cuando la cámara está lejos o a velocidades 2–3 (p. ej. ×30–×60) | Viajes más cortos en reloj sin que se vea raro de cerca | Más pasos micro por segundo (proporcional); el nivel meso casi no cambia |
| C. Alargar el día del juego en épocas tardías (p. ej. 16–24 min reales) | Más realista; afecta el ritmo y otros sistemas (clima, economía mensual) | Decisión de diseño general |

**Recomendación:** A ya (es necesario igual), y B+C a evaluar con mediciones en la fase 2. Que
la ficha muestre la duración **física** del viaje («20 min a pie») y el reloj aparte.

**Niveles de detalle (LOD) de la simulación.**

| Nivel | Dónde | Modelo | Tope orientativo |
|---|---|---|---|
| Micro | ≤ 1–1,5 km de la cámara | IDM + MOBIL, cruces con zonas de conflicto, peatones por vereda | 5.000–10.000 agentes |
| Meso | Resto de la ciudad | **Modelo de colas por carril** (Gawron 1998; Cetin, Burri y Nagel 2003; el QSim de MATSim): cada tramo tiene tiempo libre, capacidad de flujo y de almacenamiento; un vehículo sale cuando pasó el tiempo libre, hay capacidad y lugar adelante (los derrames hacia atrás salen solos). Alternativa: *cell transmission model* (Daganzo 1994) | 100.000–300.000 viajes en curso, por eventos |
| Agregado | Mapa del país, muy lejos | Flujos entre zonas por franja horaria | Ilimitado |

- **Frontera micro/meso** como en los simuladores híbridos (Mezzo, Burghout, Koutsopoulos y
  Andréasson 2005): al entrar a la zona micro, el vehículo aparece en la entrada del tramo con
  la velocidad de salida del meso; al salir, entra a la cola del tramo meso. Los tiempos medidos
  son **los mismos** en los dos niveles (la prueba de coherencia está en la fase 2).
- **Nadie desaparece**: si un vehículo queda trabado, se reporta (capa de *gridlock*) y se
  resuelve con reglas (cede el que lleva más tiempo, recalcula ruta), nunca se borra.
- **Seguir a un vecino** funciona en cualquier nivel: si está en meso, la cámara lo encuentra en
  su tramo y lo pasa a micro.
- **Muestreo**: si los viajes en curso superan el tope meso, se simula una muestra con la
  capacidad escalada (práctica estándar de MATSim), pero cada persona conserva su plan y su
  memoria.

**Azar por contador** (`hash(semilla_partida, persona, día, motivo)`) en lugar de la secuencia
única, con una prueba de oro que guarde unas pocas cifras del día.

**Capas y herramientas de tránsito** (crecen por época): flujo por tramo (ancho = vehículos/h),
velocidad relativa (medido/libre, de verde a rojo), demora por cruce, cuasi-choques, uso de
paradas y líneas, origen–destino de una calle seleccionada («¿quién pasa por acá y a dónde
va?»), y la ruta y sus alternativas de un agente seleccionado con su costo.

### 4.1 Época 1, colonial (≈1720–1830)

**Agentes:** a pie, jinetes, carretas de bueyes, carros, **arreos de ganado**, aguadores
(carro y barril), serenos, lancheros, guardas de los portones (ver 01-colonial.md §11).

**Modelos:**
- Agenda encadenada (§0 punto 1) y personas reales (D2) con su perfil (§3).
- Recados: compra diaria en la **pulpería** más cercana con algo de fidelidad (la del barrio),
  recova a media mañana, aguador que recorre las casas sin aljibe, panadero de madrugada.
- Destino del trabajo con la persona (`lugar_de_trabajo`); sin persona, logit de destino con
  caída por distancia (D4).
- **Portones de la muralla con aduana** (la muralla y sus portones ya existen en
  `services/` y `lots.rs`): el portón es un **punto de servicio** en el tramo: cola FIFO por
  sentido, tiempo de revisión según la carga (carreta de cueros 1–3 min de reloj, jinete unos
  segundos, a pie casi nada), derecho cobrado por vehículo (ya hay «Derechos de los portones»
  en el presupuesto), **cierre de noche** (los que llegan tarde esperan afuera o duermen en la
  pulpería de extramuros). Los **contrabandistas** (pocas carretas, más con aranceles altos)
  eligen rutas que evitan portones y guardas. La cola es visible y **enseña la mecánica de
  rutas** (abrir otro portón la reparte).
- **Arreos de ganado**: un agente «tropa» (cabezas, troperos a caballo) de largo variable
  (decenas de metros), velocidad ~2–4 km/h, que ocupa toda la calzada: nadie lo adelanta; los
  demás esperan o desvían. Prohibido intramuros: de la estancia (punto de entrada del país) al
  saladero o los corrales por caminos de afuera. Lo dibuja el agente de personajes y animales.
- **Adelantamiento** por el carril contrario (ya diseñado) con el umbral de la prudencia.
- **Prioridad en cruces**: la calle empedrada (o la de más tránsito medido) es la principal; los
  de la secundaria esperan un hueco (aceptación de huecos con impaciencia). Grado 3+ sigue con
  la salvaguarda.
- **Peatones que cruzan** en las esquinas: aceptación de huecos frente a los vehículos; los
  vehículos lentos no les ceden formalmente (no había reglas), pero frenan si hay alguien
  adelante (IDM con el peatón como obstáculo).
- **Clima**: barro, carretas empantanadas, menos salidas optativas (§3.3).
- **Vida social**: paseo de la plaza el domingo por la tarde, salida de misa (la plaza se llena:
  fuerza social con pocos agentes), fiestas patronales.

**Datos:** tiempos de revisión del portón, horarios de cierre, tamaño de tropas, horarios por
oficio en un archivo de datos (moddable), probabilidades de recados por clase.

**UI:** ficha del portón (cola, espera media, recaudación, contrabando estimado); capa
«cola en portones»; la ficha del vecino muestra su **agenda del día** («6:10 sale al taller,
12:00 pulpería, 17:30 vuelve») y por qué eligió su ruta.

**Rendimiento:** colonia de hasta ~10.000 habitantes: ≤ 3.000 agentes en la calle, todo micro.
Objetivo: ≤ 0,3 ms por paso con 3.000 agentes (hoy 0,2), con colas por carril en lugar de
ordenar todo (D16).

**Pruebas:**
- `ningun_vecino_esta_en_dos_lugares`: en un día completo, ningún vecino tiene dos agentes a la
  vez y toda vuelta sale después de la llegada.
- `los_tiempos_olvidan_un_atasco_viejo`: un carril atascado que nadie usa vuelve al 120 % del
  libre en ≤ 2 días de juego.
- `el_jinete_no_evita_la_calle_que_dejaron_las_carretas`.
- `el_porton_forma_cola_y_cobra` y `de_noche_el_porton_esta_cerrado`.
- `el_arreo_no_entra_intramuros` y `nadie_adelanta_un_arreo`.
- `el_perfil_es_determinista` (misma persona → mismo perfil; agregar un sorteo nuevo no cambia
  los otros).
- `con_lluvia_salen_menos_a_pasear`, `los_prudentes_aceptan_huecos_mas_largos`.

### 4.2 Época 2, Ciudad Nueva (≈1830–1870)

**Fechas verificadas:** el **tranvía de caballos** de Montevideo empezó a funcionar en **1868**
(se cita el 25 de mayo de 1868, con concesiones hacia el Cerro y Paso del Molino y hacia la
Unión), y su auge fue a fines del siglo XIX; los últimos de tracción a sangre se retiraron en
los años veinte. Es decir, cae **al final de la época 2**, aunque 03-inmigracion.md lo pone en la
época 3 («Ciudad moderna»). Propuesta: que se **habilite en 1868** si la partida está en la
época 2 (o como primer transporte de la época 3 si ya pasó), con la crónica como guiño. El
ferrocarril queda como transición a la época 3, como dice 02-ciudad-nueva.md.

**Agentes nuevos:** diligencias (estación y postas: salen y llegan por el borde del mapa,
conectadas al mapa del país), carruajes de alquiler, faroleros, el ensanche con mucho más
cruces, tranvía de caballos (1868).

**Modelos:**
- **Diligencias con horario**: salen a hora fija de la estación, esperan a los pasajeros
  (*dwell*), llevan correo; demoras por barro. Los pasajeros llegan a la estación con margen
  (horario conocido, §2.9).
- **Carruajes de alquiler**: parada fija (la plaza, la estación) con fila de coches; el vecino
  pudiente con prisa y lejos los toma (primera elección de modo con costo: logit de 2–3 opciones).
- **Tranvía de caballos**: vehículo sobre vías **dentro de la calle** (comparte el carril), no
  puede salir de la vía ni adelantar; los demás no pueden adelantarlo donde la calle es angosta;
  paradas a pedido o en esquinas; intervalos de 10–20 min; cambio de caballos en la cochera.
- **Ensanche en damero**: más cruces de cuatro calles → prioridad configurable por calle
  (principal/secundaria) y regla por defecto de la época en datos.

**Datos:** horarios de diligencias, tarifas, intervalos, velocidad del tranvía de caballos (del
orden del paso del caballo al trote suave, a calibrar).

**UI:** **editor de líneas** (primera versión: dibujar la línea sobre calles con vías, poner
paradas, elegir intervalo y cantidad de coches); ficha de la línea (pasajeros/día, coches,
intervalo real frente al programado); capa de prioridades en cruces.

**Rendimiento:** ~10.000–40.000 habitantes: todavía mayormente micro; el meso entra como red de
seguridad.

**Pruebas:** `la_diligencia_sale_a_horario_y_espera`, `nadie_adelanta_al_tranvia_en_calle_angosta`,
`el_tranvia_respeta_el_intervalo` (sin pasajeros) y `con_pasajeros_aparece_el_amontonamiento`
(con *dwell* por pasajero; valida el modelo).

### 4.3 Época 3, Inmigración (≈1870–1930)

**Fechas:** **tranvía eléctrico desde 1906** (empresas La Transatlántica, de capital alemán, y
la Sociedad Comercial de Montevideo, británica); el servicio de tranvías duró hasta **1957**
aproximadamente (fechas exactas a verificar en fuentes de archivo). Primeros automóviles,
muy pocos.

**Modelos:**
- **Red de transporte público genérica** (sirve para todo lo que sigue): líneas, paradas,
  coches con capacidad, *dwell* por pasajero, intervalos, cocheras, regulación del intervalo.
  Los pasajeros: caminar a la parada, esperar, subir si hay lugar, transbordar.
- **Elección de modo** con logit (a pie, tranvía, carruaje, bicicleta de la época para pocos) e
  inercia; boleto según ingreso; pase obrero como política.
- **Rutas de transporte público** en el mismo A\* con tramos de «espera» y «a bordo» (grafo
  multimodal) o, más barato, rutas por líneas precalculadas con 0–1 transbordos.
- **Crecimiento que sigue al tranvía** (03-inmigracion.md §6): la accesibilidad (tiempo medio a
  los trabajos en tranvía) entra al valor del suelo.
- **Agente de tránsito** en esquinas muy cargadas: alterna fases manualmente (prefigura el
  semáforo).

**UI:** editor de líneas completo (horarios por franja, flota), capa de accesibilidad (isócronas
desde un punto), capa de ocupación de coches, ficha de parada (gente esperando, espera media).

**Rendimiento:** ~50.000–300.000 habitantes: micro cerca de la cámara (≤ 5.000), meso para el
resto; pasajeros en meso como contadores por parada y coche (no agentes caminando).

**Pruebas:** `el_logit_reparte_segun_el_tiempo` (doblar el tiempo de un modo baja su parte
según la elasticidad esperada), `los_pasajeros_esperan_medio_intervalo`, `el_coche_lleno_deja_gente`,
`coherencia_micro_meso` (tiempo de paso en un corredor dentro del ±10 %).

### 4.4 Época 4, Art Déco y la Rambla (≈1930–1960)

**Agentes:** ómnibus (estación de ómnibus), autos particulares, camiones de reparto, bicicletas,
bañistas a la playa, hinchas al estadio.

**Modelos (el gran salto del tránsito):**
- **Calles con carriles** (perfil de vía: carriles por sentido, sentido único, vías de tranvía,
  estacionamiento a un lado, bicisenda) y **conectores de carril** en cada cruce, generados
  automáticamente y editables (flechas de carril).
- **MOBIL** + cambios obligatorios con anticipación (§2.2), cortesía por persona: dejar entrar
  al que se incorpora.
- **Cruces por zonas de conflicto**, **semáforos con fases** (plan fijo de Webster, editable),
  **PARE / ceda el paso**, **rotondas**; agentes de tránsito.
- **Estacionamiento** en la calle y garajes; autos persistentes del hogar.
- **Rambla**: avenida costera con pocas intersecciones; si el jugador eligió «avenida rápida»
  (04-art-deco-rambla.md §5), más velocidad y más ruido; si eligió «paseo», peatones y bañistas
  con prioridad en los cruces. Paseo de la tarde en verano (vida social de §3).
- **Eventos** (estadio, carnaval): demanda puntual de decenas de miles de viajes; el tránsito
  colapsa si no se preparó (requisito del documento de época).
- **Ruido de conducción** estilo Krauss/HDM para que aparezcan ondas de *stop-and-go*.
- Elección de **hora de salida** (Vickrey) para que la hora pico se ensanche.

**UI:** herramientas de cruce (prioridad, semáforo y sus fases con una línea de tiempo, flechas
de carril), capa de demoras por cruce, capa de búsqueda de estacionamiento, «¿por qué se atasca
acá?» (el carril que se angosta, el cruce sin fase de giro, el estacionamiento en doble fila).

**Rendimiento:** 300.000–800.000 habitantes; micro ≤ 8.000 agentes (≤ 2 ms por paso en un
núcleo, o repartido en hilos por carril); meso hasta ~150.000 viajes en curso por eventos
(≤ 5 ms por segundo de juego).

**Pruebas:**
- `el_semaforo_llega_al_flujo_de_saturacion` (≈ 1.800–1.900 veh/h/carril de verde en una
  aproximación recta con autos homogéneos, ±10 %).
- `el_anillo_de_sugiyama_genera_ondas` (con ruido) y `sin_ruido_no` (con el IDM puro).
- `los_cambios_de_carril_de_ultimo_momento_son_pocos` (< 5 % de los obligatorios en los
  últimos 30 m).
- `la_rotonda_cede_al_que_circula`, `nadie_desaparece_en_un_gridlock` (se detecta y se resuelve).
- `el_auto_vuelve_a_donde_lo_estaciono` (sin autos de bolsillo).

### 4.5 Época 5, Modernismo (≈1960–1990)

**Modelos:**
- **Congestión masiva** y vías rápidas con **incorporaciones** (*merge*) por cremallera.
- **Aprendizaje día a día** completo (memoria de rutas con logit e inercia) y medición de la
  convergencia (brecha relativa) para que no oscile.
- **Ómnibus diésel** con flota, recorridos largos, terminales; colectivos interdepartamentales
  que entran desde el mapa del país.
- **Crisis del petróleo** (evento del documento de época): el costo del auto sube → el logit
  pasa gente al ómnibus de forma medible.
- **Cuasi-choques y accidentes** (§2.11) y la capa de puntos negros.

**Rendimiento:** ~1 millón de habitantes: micro ≤ 10.000, meso ≤ 300.000 viajes en curso o
muestra escalada; agregado por zonas para el país.

**Pruebas:** `wardrop_converge` (red de dos rutas: brecha < 5 % en ≤ 10 días de juego, sin
oscilación de más del 10 %), `la_crisis_del_petroleo_sube_el_omnibus`,
`los_puntos_negros_salen_donde_hay_conflictos`.

### 4.6 Época 6, Contemporánea (≈1990–hoy)

**Modelos:**
- **Prioridad al transporte público**: carriles exclusivos, prioridad semafórica (el verde se
  estira para el ómnibus que llega), cobro antes de subir (baja el *dwell*), corredores de
  ómnibus eléctricos.
- **Bicicletas** en bicisendas (IDM con parámetros propios) y en la calle (adelantamiento con
  distancia lateral); **peatonales** (fuerza social en la zona micro).
- **Elección modal sostenible**: el logit incluye salud, comodidad y costo; las políticas
  (estacionamiento caro, peatonales, boleto) mueven el reparto de forma legible.
- **Monopatines y reparto** (muchos viajes cortos en el último tramo).
- Semáforos **actuados** y ondas verdes.
- Turistas de crucero: grupos que caminan del puerto al casco histórico.

**UI:** panel de reparto modal con historia, herramienta de corredores (carril exclusivo +
prioridad en un clic), mapa de accesibilidad en transporte público.

**Pruebas:** `la_prioridad_semaforica_baja_la_demora_del_omnibus` (−X % medible sin subir más
de Y % la de los autos), `la_bicisenda_aumenta_el_uso_de_bici`.

---

## 5. Fases de implementación (priorizadas)

**Conflictos con trabajos en curso.** Hay agentes trabajando ahora en: (a) **dibujo de
personajes y animales** en las partes de dibujo de `engine/src/city/agentes/` (`dibujo.rs`, el
shader de agentes); (b) la **época 2**; (c) el **mapa del país** (`engine/src/pais/`, nuevo);
(d) **agua y terreno** (`terrain.rs`, `water.wgsl`, el *streaming*). **Ninguna fase empieza
antes de que se integren sus ramas.** Puntos de contacto concretos:
- (a) Los tipos nuevos (`Tipo::Arreo`, aguador, sereno, diligencia, tranvía) agregan modelos
  en `dibujo.rs`: la lógica se hace acá y los modelos los pone (o los revisa) el agente de
  personajes; se acuerda la lista de `Tipo` y el campo de animación (paso, trote, tropa).
- (b) La época 2 define edificios (estación de diligencias, cocheras) y quizás transportes: este
  plan solo agrega su conducta una vez que existan; hay que acordar quién define `Tipo::Diligencia`
  y el horario.
- (c) El mapa del país define las **entradas** (caminos al interior, postas, estancias): hoy
  `Lugares::campo` es «el extremo de calle más alejado de la plaza» (`mod.rs:1306-1309`); pasa a
  ser la lista de entradas que publique el mapa del país, con sus flujos (carretas, arreos,
  diligencias).
- (d) Agua y terreno: el puerto (`Lugares::puerto`, hoy por `is_water`) y las calles que se
  inundan (cortes de tramo → recálculo en el momento, §3.3).

| Fase | Contenido | Archivos | Depende de | Criterios medibles |
|---|---|---|---|---|
| **F0. Correcciones** | Agenda encadenada (D1), duraciones físicas en las fichas, tiempos con olvido + por clase + por franja (D5–D7), azar por contador (D16), recálculo en lugar de borrar (D15), colas por carril en lugar de ordenar todo | `agentes/mod.rs`, `ruta.rs`, `tests.rs` | (a) integrado | 0 vecinos duplicados en 7 días de juego; atasco viejo olvidado en ≤ 2 días; ≤ 0,15 ms por paso con 3.000 agentes; partidas viejas cargan |
| **F1. Personas y actitudes** | Agentes desde `Poblacion` (persona, trabajo, casa), perfil (§3), recados y vida social, clima, incendio, epidemia, delito de noche, protestas | `agentes/`, `poblacion/` (solo lectura de API), `game.rs` (cableado) | F0, (b) integrado | 100 % de los agentes con persona cuando hay población; distribución de horarios por oficio; −30 % de viajes optativos con lluvia; los «seguir a un vecino» de 10 personas al azar pasan una revisión manual |
| **F2. Colonia jugable** | Portones con aduana y cierre, contrabandistas, arreos, aguadores, serenos, adelantamiento, prioridad en cruces, peatones que cruzan, carretas empantanadas | `agentes/`, `services/` (portones), `dibujo.rs` con (a) | F1, (c) para las entradas | Cola visible en un portón con > 15 carretas/h; espera media del portón en la ficha; 0 arreos intramuros; adelantamientos solo con hueco ≥ umbral |
| **F3. Infraestructura para escalar** | Perfil de vía con carriles y sentido único en `roads.rs` (con migración de partidas), conectores de carril, zonas de conflicto, meso de colas + frontera híbrida, A\* con ALT, capas de tránsito | `roads.rs`, `agentes/` (nuevo `meso.rs`, `cruces.rs`) | F0 | Coherencia micro/meso ±10 %; 100.000 viajes meso en curso con ≤ 5 ms por segundo de juego; 0 desapariciones; decisión de escala temporal tomada con mediciones |
| **F4. Época 2** | Diligencias, carruajes de alquiler, tranvía de caballos (1868), editor de líneas v1, logit de 2–3 modos | `agentes/` (nuevo `lineas.rs`), UI | F2, F3, (b) | Intervalo real dentro del ±20 % del programado sin pasajeros; amontonamiento observable con *dwell* por pasajero; reparto modal estable día a día (±5 %) |
| **F5. Época 3** | Transporte público genérico, tranvía eléctrico (1906), grafo multimodal, accesibilidad → valor del suelo, agente de tránsito | `agentes/`, `lots.rs` (valor) | F4 | Espera media ≈ medio intervalo (±15 %); elasticidad del logit en rango; isócronas correctas en una red de prueba |
| **F6. Época 4** | Autos persistentes, estacionamiento, MOBIL, semáforos, rotondas, Rambla, eventos, ruido HDM/Krauss, hora de salida | `agentes/`, `roads.rs`, UI de cruces | F5 | Flujo de saturación 1.800–1.900 veh/h/carril ±10 %; ondas en el anillo de Sugiyama; < 5 % de cambios obligatorios en los últimos 30 m; 0 autos de bolsillo |
| **F7. Épocas 5–6** | Aprendizaje día a día con memoria de rutas, vías rápidas, cuasi-choques y accidentes, prioridad al transporte público, bicis, peatonales, políticas de movilidad | `agentes/`, UI | F6 | Brecha relativa < 5 % en ≤ 10 días; prioridad semafórica que baja la demora del ómnibus de forma medible; 1 millón de habitantes con ≤ 10 % de un núcleo a velocidad 1 |

**Orden recomendado:** F0 → F1 → F2 dan la mejora visible para la época colonial (lo que el
jugador tiene hoy). F3 puede ir en paralelo a F2 si lo hace otra persona (toca `roads.rs` y
archivos nuevos), pero tiene que estar antes de F4.

---

## 6. Riesgos y decisiones abiertas

- **Escala temporal** (§4.0): decidir antes de F6; afecta a todo el tránsito moderno.
- **Partidas viejas**: los cambios de `Estado` (agenda encadenada, tiempos por clase y franja,
  perfil) necesitan `serde(default)` y regeneración, como ya se hace con los vecinos.
- **Determinismo con hilos**: si el paso micro se reparte en hilos, el resultado tiene que ser
  idéntico (fase de cálculo en paralelo, fase de escritura en orden por id, como ya está
  estructurado `paso`).
- **Sobreajuste del realismo**: cada conducta nueva tiene que verse en la ficha o en una capa;
  si el jugador no la puede leer, no vale su costo.
- **Calibración**: los números de §3.2 son orientativos; se fijan con las pruebas de §4 y con
  sesiones de juego.
- **Datos históricos**: las fechas del tranvía (1868 de caballos, 1906–1957 eléctrico) están
  verificadas en fuentes de divulgación; conviene confirmar con bibliografía de historia del
  transporte uruguayo antes de ponerlas en la crónica.

---

## 7. Fuentes

**Tránsito y conducción**
- Treiber, M., Hennecke, A. y Helbing, D. (2000). *Congested traffic states in empirical
  observations and microscopic simulations*. Physical Review E 62, 1805–1824. (IDM)
- Treiber, M., Kesting, A. y Helbing, D. (2006). *Delays, inaccuracies and anticipation in
  microscopic traffic models*. Physica A 360, 71–88. (Human Driver Model)
- Kesting, A., Treiber, M. y Helbing, D. (2007). *General lane-changing model MOBIL for
  car-following models*. Transportation Research Record 1999, 86–94.
- Kesting, A., Treiber, M. y Helbing, D. (2010). *Enhanced intelligent driver model to access
  the impact of driving strategies on traffic capacity*. Phil. Trans. R. Soc. A 368, 4585–4605.
- Treiber, M. y Kesting, A. (2013). *Traffic Flow Dynamics*. Springer.
- Treiber, M. y Kanagaraj, V. (2015). *Comparing numerical integration schemes for
  time-continuous car-following models*. Physica A 419, 183–195.
- Gipps, P. G. (1981). *A behavioural car-following model for computer simulation*.
  Transportation Research B 15(2), 105–111.
- Gipps, P. G. (1986). *A model for the structure of lane-changing decisions*. Transportation
  Research B 20(5), 403–414.
- Krauss, S. (1998). *Microscopic modeling of traffic flow: investigation of collision free
  vehicle dynamics*. Tesis doctoral, Universität zu Köln / DLR.
- Lopez, P. A. et al. (2018). *Microscopic traffic simulation using SUMO*. IEEE ITSC.
- Sugiyama, Y. et al. (2008). *Traffic jams without bottlenecks*. New Journal of Physics 10,
  033001.

**Peatones**
- Helbing, D. y Molnár, P. (1995). *Social force model for pedestrian dynamics*. Physical
  Review E 51, 4282–4286.
- Weidmann, U. (1993). *Transporttechnik der Fussgänger*. ETH Zürich, IVT Schriftenreihe 90.
- Knoblauch, R., Pietrucha, M. y Nitzburg, M. (1996). *Field studies of pedestrian walking speed
  and start-up time*. Transportation Research Record 1538, 27–38.
- van den Berg, J., Guy, S., Lin, M. y Manocha, D. (2011). *Reciprocal n-body collision
  avoidance*. Robotics Research, Springer. (ORCA)
- Treuille, A., Cooper, S. y Popović, Z. (2006). *Continuum crowds*. ACM SIGGRAPH.

**Demanda, modo y rutas**
- Horni, A., Nagel, K. y Axhausen, K. W. (eds.) (2016). *The Multi-Agent Transport Simulation
  MATSim*. Ubiquity Press (acceso abierto).
- Bowman, J. L. y Ben-Akiva, M. (2001). *Activity-based disaggregate travel demand model system
  with activity schedules*. Transportation Research A 35(1), 1–28.
- Vickrey, W. (1969). *Congestion theory and transport investment*. American Economic Review
  59(2), 251–260.
- Small, K. A. (1982). *The scheduling of consumer activities: work trips*. American Economic
  Review 72(3), 467–479.
- McFadden, D. (1974). *Conditional logit analysis of qualitative choice behavior*. En
  Zarembka (ed.), *Frontiers in Econometrics*.
- Ben-Akiva, M. y Lerman, S. (1985). *Discrete Choice Analysis*. MIT Press.
- Wardrop, J. G. (1952). *Some theoretical aspects of road traffic research*. Proc. Institution
  of Civil Engineers 1(3), 325–378.
- Daganzo, C. y Sheffi, Y. (1977). *On stochastic models of traffic assignment*. Transportation
  Science 11(3), 253–274.
- Horowitz, J. L. (1984). *The stability of stochastic equilibrium in a two-link transportation
  network*. Transportation Research B 18(1), 13–28.
- Cascetta, E. (1989). *A stochastic process approach to the analysis of temporal dynamics in
  transportation networks*. Transportation Research B 23(1), 1–17.
- Goldberg, A. y Harrelson, C. (2005). *Computing the shortest path: A\* search meets graph
  theory*. SODA. (ALT)
- Geisberger, R., Sanders, P., Schultes, D. y Delling, D. (2008). *Contraction hierarchies*.
  WEA. Dibbelt, J., Strasser, B. y Wagner, D. (2016). *Customizable contraction hierarchies*.
  ACM JEA 21.

**Cruces, estacionamiento, transporte público, seguridad**
- Raff, M. S. y Hart, J. W. (1950). *A volume warrant for urban stop signs*. Eno Foundation.
- Mahmassani, H. y Sheffi, Y. (1981). *Using gap sequences to estimate gap acceptance
  functions*. Transportation Research B 15(3), 143–148.
- Webster, F. V. (1958). *Traffic signal settings*. Road Research Technical Paper 39, HMSO.
- Transportation Research Board (2016). *Highway Capacity Manual*, 6.ª ed.
- Dresner, K. y Stone, P. (2008). *A multiagent approach to autonomous intersection management*.
  Journal of Artificial Intelligence Research 31, 591–656.
- Shoup, D. (2006). *Cruising for parking*. Transport Policy 13(6), 479–486.
- Waraich, R. A. y Axhausen, K. W. (2012). *Agent-based parking choice model*. Transportation
  Research Record 2319, 39–46.
- Newell, G. F. y Potts, R. B. (1964). *Maintaining a bus schedule*. Proc. 2nd Australian Road
  Research Board Conference.
- Osuna, E. E. y Newell, G. F. (1972). *Control strategies for an idealized public
  transportation system*. Transportation Science 6(1), 52–72.
- TCRP Report 165 (2013). *Transit Capacity and Quality of Service Manual*, 3.ª ed.
- Hayward, J. C. (1972). *Near-miss determination through use of a scale of danger*. Highway
  Research Record 384. (TTC)
- Gettman, D. et al. (2008). *Surrogate Safety Assessment Model and Validation*. FHWA-HRT-08-051.

**Simulación mesoscópica e híbrida**
- Gawron, C. (1998). *An iterative algorithm to determine the dynamic user equilibrium in a
  traffic simulation model*. International Journal of Modern Physics C 9(3), 393–407.
- Cetin, N., Burri, A. y Nagel, K. (2003). *A large-scale agent-based traffic microsimulation
  based on queue model*. Swiss Transport Research Conference (STRC).
- Daganzo, C. (1994). *The cell transmission model*. Transportation Research B 28(4), 269–287.
- Burghout, W., Koutsopoulos, H. y Andréasson, I. (2005). *Hybrid mesoscopic–microscopic traffic
  simulation*. Transportation Research Record 1934, 218–225.

**Juegos** (comportamientos descritos por la comunidad, los diarios de desarrollo y la prensa
especializada; sin cifras internas): *SimCity* (Maxis, 2013); *Cities: Skylines* (Colossal
Order, 2015) y el mod *Traffic Manager: President Edition*; *Cities: Skylines II* (2023);
*A/B Street* (Dustin Carlino, código abierto, Rust).

**Historia del transporte en Montevideo** (divulgación, a confirmar con archivo)
- Prensa Latina (2024), «Un tranvía llamado Andrés»: inicio del tranvía de caballos el 25 de
  mayo de 1868. https://www.prensa-latina.cu/2024/03/23/un-tranvia-llamado-andres/
- Blog *daos1964*, «El primer tranvía a caballo en Uruguay»: concesiones al Cerro y Paso del
  Molino y a la Unión; auge en 1893–1897; fin de la tracción a sangre en 1925.
  https://daos1964.blogspot.com/2022/10/el-primer-tranvia-caballo-en-uruguay.html
- Montevideo Portal: fin de los tranvías eléctricos en 1957.
  https://www.montevideo.com.uy/Noticias/Arquitecto-uruguayo-propone-una-linea-de-tranvia-en-Montevideo-uc729132
- Wikipedia, *List of town tramway systems in South America* (tranvía eléctrico de 1906 a 1957;
  verificar la fila). https://en.wikipedia.org/wiki/List_of_town_tramway_systems_in_South_America
