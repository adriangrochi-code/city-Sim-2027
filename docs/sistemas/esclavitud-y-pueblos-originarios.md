# Esclavitud, libertad y pueblos originarios en Montevideo (1726–1853)

> Documento de sistema. Complementa a [población](poblacion.md) (sección 5) y a la
> [época colonial](../epocas/01-colonial.md) (§15.2). Reúne la investigación histórica, la
> auditoría de lo que el juego tenía, las correcciones hechas y las preguntas que quedan
> abiertas para el diseño. Estado: **implementado en el motor**
> (`engine/src/city/poblacion/historia.rs` y los demás archivos de `poblacion/`).

## 1. La decisión de diseño (se mantiene)

**Representación cuidadosa.** Las personas africanas y afrodescendientes (esclavizadas,
libertas y libres) y las indígenas existen como **personas con nombre, familia e historia**. La
desigualdad legal de la época es una **restricción** sobre sus vidas, **nunca una mercancía**:

- no hay forma de comprar, vender, alquilar, subastar ni transferir personas; `Persona` no tiene
  precio, valor ni dueño (la prueba `no_hay_forma_de_comerciar_personas` lo revisa en el código);
- las llegadas por la trata, los «colonos» y los prisioneros de 1831 son **hechos del mundo** que
  la crónica narra; no los decide el jugador y nadie paga ni cobra en el modelo;
- pueden obtener la libertad, formar familia, tener oficio y ascender;
- la crónica y la interfaz usan un lenguaje digno: «persona esclavizada», «quien la sometía»,
  «de nación Benguela», nunca insultos ni caricaturas.

## 2. Lo que dice la historiografía (resumen verificado)

Las cifras y fechas marcadas **[V]** se verificaron en la fuente citada; **[E]** son estimaciones
de diseño; **[NV]** son datos conocidos pero no verificados en una fuente leída para este trabajo.

### 2.1 La trata hacia el Río de la Plata y Montevideo

| Hecho | Dato | Fuente |
|---|---|---|
| Antes de 1777 | La trata entra sobre todo por Buenos Aires y por la Colonia del Sacramento portuguesa (desde 1680) | [V] Frega et al. 2020, cap. 3 |
| Real Compañía de Filipinas | Creada en 1785; autorizada a traficar esclavizados en 1787; primeros arribos a Montevideo en 1788 | [V] Chagas y Stalla, en Frega et al. 2020, p. 91 |
| Libre introducción | La Corona liberaliza la introducción de esclavizados para el Caribe (1789) y para el Río de la Plata (1791) | [V] Frega et al. 2020, p. 89 |
| Volumen | Al menos 70.000 cautivos llegaron por mar al Río de la Plata entre 1777 y 1812; el 60 % de los desembarcados entre 1791 y 1812 partió de Brasil; al menos 333 viajes llevaron 23.000 personas desde Río de Janeiro a Montevideo y Buenos Aires | [V] Borucki, citado en Frega et al. 2020, p. 87 |
| Destino | Muchos seguían viaje a Buenos Aires, Chile, el Alto Perú y Lima | [V] Frega et al. 2020, p. 94 |
| Prohibición de 1812 | El gobierno de Buenos Aires prohíbe el tráfico (abril de 1812). Montevideo era realista hasta 1814 | [V] Frega et al. 2020, p. 136 |
| Dominación luso-brasileña (1817–1829) | Dejó de aplicarse la libertad de vientres de 1813 y «aumentó la introducción de esclavizados tanto en Montevideo como en la frontera» | [V] Frega et al. 2020, pp. 118-119 y 136 |
| «Colonos» africanos | Primer contrato: 12 de noviembre de 1832. Menores de 16 años, varones no más del 60 %, servicio de 12 años desde los 12; al menos mil ingresados a Montevideo y Maldonado, solo 220 registrados. Prohibidos por Oribe en marzo de 1835 | [V] Frega et al. 2020, pp. 124-125 y 136 |
| Último viaje | El último viaje transatlántico directo desde Angola llegó a Montevideo en 1835 | [V] Frega et al. 2020, p. 87 |
| Después | Contrabando en pequeña escala y entradas por la frontera con Brasil; «contratos de peonaje» desde 1852 | [V] Frega et al. 2020, pp. 125 y 137 |

### 2.2 El «Caserío de los Negros»

Depósito y lugar de cuarentena de la Real Compañía de Filipinas y luego de traficantes
particulares, sobre la costa y la margen izquierda del arroyo Miguelete (hoy Capurro: planta de
ANCAP, parque Capurro, Escuela n.º 47). Usado como cuarentena **desde 1788 hasta el declive del
tráfico hacia 1810**, de uso estacional (diciembre a abril); muchas personas morían de disentería
días después de desembarcar. En 1815 alojó a dos compañías de libertos del gobierno artiguista y,
en la dominación luso-brasileña, tropa. En las décadas de 1830 y 1840 el terreno se fraccionó.
Hoy es sitio de memoria. **[V]** Chagas y Stalla, en Frega et al. 2020, pp. 91-95 (que citan a
Bracco y López Mazz 2014 y a Curbelo y Onega 2005).

### 2.3 Cuántas personas, por fecha

| Fecha | Dato | Fuente |
|---|---|---|
| Fines de la década de 1770 | Población afrodescendiente > 25 %; indígena < 2 % (sobre todo guaraníes de las misiones) | [V] Pollero y González Demuro 2024 |
| 1805 | Las personas esclavizadas eran «cerca de la tercera parte» de Montevideo | [V] Borucki 2005 |
| Antes de 1810 → 1819 | Esclavizadas: del 30 % a «poco menos del 25 %»; masculinidad de 119 a 78 (por la militarización) | [V] Borucki 2005, citando a Frega 2004 |
| Primeras décadas del XIX | Afrodescendientes: 22 a 25 % de Montevideo (otras estimaciones) | [V] Martínez, en Frega et al. 2020, p. 101 |
| Revolución | El 45 % de las viviendas de Montevideo tenía al menos una persona esclavizada | [V] Frega et al. 2020, p. 119 |
| 1830 | Padrón de cuatro cuarteles (*El Caduceo*): 9.660 habitantes, 25 % esclavizados | [V] Borucki 2005 |
| 1829 | Unos 14.000 habitantes en Montevideo y 74.000 en el territorio | [V] Borucki 2005, citando a Mendive y Villa |

Las cifras de 1805 y de 1810–1819 no coinciden del todo entre autores (un tercio frente a 22–25 %
de afrodescendientes): el juego apunta a un rango, no a un número.

### 2.4 Leyes y caminos a la libertad

| Fecha | Disposición | Fuente |
|---|---|---|
| Derecho indiano | Las personas esclavizadas podían tener peculio, casarse, contraer deudas, pedir cambio de amo por malos tratos y comprar su libertad, con el **defensor de pobres, menores y esclavos**. Desde fines del XVIII, domingos y festivos libres para trabajar por su cuenta | [V] Frega et al. 2020, pp. 102-103 |
| Formas de libertad | Las más frecuentes: concesión del amo (en vida o por testamento, a menudo con condiciones), **compra** con el peculio (o con colectas de cofradías y salas de nación) y **las armas** | [V] Frega et al. 2020, p. 103 |
| 1790–1820 | El 67 % de las compras de libertad fue de mujeres; las mujeres, casi el 60 % de las personas liberadas; las familias juntaban recursos para liberar primero a la esposa y madre (la condición se heredaba por vía materna) | [V] Frega et al. 2020, p. 104 |
| 1760 | Se crea el Regimiento de Pardos y Morenos; desde 1762, «manumisión por leva». Primera milicia negra de Montevideo: 80 hombres en 1780 | [V] Frega et al. 2020, pp. 104-106 |
| 1813 | Libertad de vientres de la Asamblea del Año XIII (2 de febrero), aplicada en la Provincia Oriental artiguista; reglamento de libertos (pupilos hasta los 20 años los varones y 16 las mujeres), al parecer no aplicado allí | [V] Frega et al. 2020, pp. 117-118 y 136 |
| 1816 | Ante la invasión portuguesa, padrón de esclavizados en Montevideo y leva: no dejar más de tres a ningún amo; muchos amos los escondieron | [V] Frega et al. 2020, p. 120 |
| 1817–1829 | Bajo Portugal y Brasil, los hijos de esclavizadas vuelven a heredar esa condición | [V] Frega et al. 2020, p. 136 |
| **7 de setiembre de 1825** | La Sala de Representantes (Florida) dispone que nadie nacerá esclavo y prohíbe el tráfico de país extranjero. **No se aplicó en Montevideo ni en Colonia, bajo control brasileño** | [V] Frega et al. 2020, pp. 119 y 136 (el MEC da el 6 de setiembre para la prohibición del tráfico y el 7 para la libertad de vientres) |
| **20 de enero de 1830** | La Asamblea General Constituyente extiende la ley de 1825 a todo el territorio | [V] Frega et al. 2020, p. 136 |
| **18 de julio de 1830** | Constitución, art. 131: «En el territorio del Estado nadie nacerá ya esclavo; queda prohibido para siempre su tráfico e introducción en la República». No declara la abolición. Art. 7: ciudadanos los hombres libres; la ciudadanía se suspende al «sirviente a sueldo, peón jornalero, simple soldado de línea» y a quien no sabe leer ni escribir | [V] Frega et al. 2020, pp. 123, 136 y 175 |
| 14 de junio de 1837 | Ley que reglamenta la prohibición: los introducidos son libres, **bajo patronato** (tres años los mayores; hasta los 25 los menores) | [V] Frega et al. 2020, pp. 125 y 136 |
| 1839 / 1842 | Tratado anglo-uruguayo contra la trata (13 de julio de 1839, ratificado el 21 de enero de 1842) | [V] Frega et al. 2020, p. 137 |
| **12 de diciembre de 1842** | Ley de abolición del Gobierno de la Defensa, en plena Guerra Grande y con el Ejército de Reserva: los liberados se concentran en el Cantón del Miguelete y se clasifican; los **aptos van al ejército**; los demás (mujeres, menores, mayores) quedan **bajo patronato** de sus antiguos amos según el reglamento de 1837. Se reconoce compensación a los amos | [V] Frega et al. 2020, pp. 127-131 |
| **28 de octubre de 1846** | Ley de abolición del Gobierno del Cerrito (Oribe), en el territorio que controlaba (no Montevideo, Maldonado ni Colonia); al día siguiente, «comisiones clasificadoras de esclavos» | [V] Frega et al. 2020, pp. 129 y 137. **Discrepancia**: el MEC da el 26 de octubre |
| 1851–1853 | Tratados con Brasil (devolución de esclavizados fugados); 14 de julio de 1853: ley de compensaciones a los amos; **2 de mayo de 1853**: se elimina el patronato sobre los hijos de los emancipados; sigue el «pupilaje» de huérfanos y pobres («criados») | [V] Frega et al. 2020, pp. 131 y 137 |
| 1862 | Ley que declara piratería el tráfico | [V] Frega et al. 2020, p. 137 |

### 2.5 Trabajo, nombres y vida social

- **Trabajo** [V] (Frega et al. 2020, pp. 101, 107-110): servicio doméstico, artesanos,
  vendedores ambulantes, estibadores, saladeros y panaderías; en el campo, peones y capataces.
  Las mujeres: servicio, lavado y planchado (de la casa y de otras familias), pan y dulces para
  vender, costura, amas de leche. Muchos eran **conchabados** a terceros y la paga «en su mayoría
  iba para el amo». También esclavos públicos del Cabildo. En 1841 los saladeros del Cerro
  tenían 57 a 62 africanos esclavizados cada uno.
- **Nombres** [V] (Frega et al. 2020, cap. 7; Andrews 2007): nombre cristiano impuesto en el
  bautismo; registro con la **«nación»** (nombres de puertos y regiones de embarque: Congo,
  Angola, Benguela, Mina, Calabarí, Mozambique…) y con el apellido o el nombre de quien los
  sometía.
- **Cofradías** [V] (Frega et al. 2020, pp. 199-201): San Benito de Palermo (desde **1773**, en
  el convento de San Bernardino y luego en San Francisco), San Baltasar (en la Matriz, desde
  **1787**) y Nuestra Señora del Rosario. Cuidado en la enfermedad, entierro, fiesta del santo,
  limosnas; ayudaban a quien intentaba obtener la libertad.
- **Salas de nación** [V] (Frega et al. 2020, pp. 201-204; Andrews 2007): desde fines del
  XVIII; entre 15 y 20 en la primera mitad del XIX; ayuda mutua, representación, religión; el
  6 de enero (día de San Baltasar) coronaban a sus reyes, iban a misa a la Matriz y comenzaban
  los **candombes**, también los domingos junto al Cubo del Sur. Tras la abolición se mudan al
  sur del Cordón (Barrio Sur, Palermo) y decaen hacia 1880–1900.

### 2.6 Pueblos originarios

- **Guaraníes de las misiones** en la fundación: Zabala informa en 1728 de «un centenar de
  indios traídos por los jesuitas» para las obras [V, Pollero y González Demuro 2024]; una
  crónica habla de «mil indios tapes» [cifra de crónica, no usada]. **No verifiqué** la forma
  de pago (jornal, ración, turnos) de esas cuadrillas.
- **Población indígena** de Montevideo: < 2 % a fines de la década de 1770 [V, Pollero].
- **Salsipuedes** (11 de abril de 1831): el ejército del Estado, al mando de Rivera, ataca a los
  charrúas; los prisioneros, casi todos mujeres, niñas y niños, son llevados a Montevideo y
  **repartidos entre familias** como sirvientes; muchos niños separados de sus madres (orden del
  29 de abril de 1831). Las cifras no coinciden entre fuentes (unos 150 a 166 llegados a
  Montevideo) [NV en fuente primaria; resúmenes de búsqueda: Wikipedia, INDDHH]. En 1833 cuatro
  charrúas fueron llevados a París [NV].
- **Misiones orientales**: tras la conquista portuguesa de los pueblos orientales (1801) y la
  campaña de Rivera (1828), familias guaraníes fundan Santa Rosa del Cuareim / Bella Unión
  (1829) y luego San Francisco de Borja del Yí (1833) [NV: resúmenes de búsqueda; cifras no
  verificadas].

## 3. Auditoría: lo que había, lo que estaba mal y lo que se corrigió

Rutas relativas a `engine/src/city/poblacion/` salvo que se indique. Las líneas son las del
código **anterior** a esta revisión.

| # | Afirmación o comportamiento | Dónde | Juicio | Evidencia | Corrección |
|---|---|---|---|---|---|
| 1 | Libertad de vientres y prohibición del tráfico en Montevideo desde **enero de 1825** | `economia.rs:701` (`año >= 1825`); `docs/sistemas/poblacion.md` §5 y §8 | **Incorrecto** | La ley es del 7 de setiembre de 1825 y no rigió en Montevideo, en manos del Brasil, hasta el 20 de enero de 1830 | En el modo fiel rige desde enero de 1830; la ley de la Florida se cuenta en setiembre de 1825 sin efecto en la ciudad; la Constitución (art. 131) en julio de 1830. Prueba `en_la_historia_fiel_las_leyes_llegan_en_sus_fechas` |
| 2 | Abolición en **enero de 1842**, todas las personas libres con hogar propio | `economia.rs:708-717` | **Incorrecto e impreciso** | 12 de diciembre de 1842; varones aptos al ejército; mujeres, menores y mayores bajo patronato (reglamento de 1837) | `historia::abolicion_de_1842`: leva de los varones de 15 a 45 años con salud, patronato para los demás (menores hasta los 25, mayores 3 años). Prueba `abolicion_de_1842_con_leva_y_patronato` |
| 3 | La ley del Cerrito «es de 1846» | `poblacion.md` §5 | Correcto, sin contexto | 28 de octubre de 1846, fuera de Montevideo | Se cuenta en la crónica con su contexto |
| 4 | La trata pesa ×1,2 desde 1813 hasta la ley | `origenes.rs:209-216` | **Impreciso** | 1811–1816 casi se corta (revolución, sitios, prohibición de 1812); 1817–1829 aumenta (luso-brasileños); el pico es 1791–1810 (libre introducción) | `factor_trata` por período: 0,5 / 1 / 3,5 (1777–1790) / 6 (1791–1810) / 0,15 (1811–1816) / 1,3 (1817–1829) [E] |
| 5 | Después de la ley no entra nadie | `mod.rs:833` | **Faltaba** | Colonos africanos 1832–1835; contrabando | `historia::llegada_colonos`: en el modo fiel, de noviembre de 1832 a marzo de 1835, niñas, niños y jóvenes de 8 a 16 años, ≤ 60 % varones, libres en el papel y bajo patronato hasta los 24 años |
| 6 | Patronato de libertos pendiente | `poblacion.md` §13 | **Faltaba** | Reglamento de 1837; abolición de 1842; ley de 1853 | Bandera `Persona::patronato` (mes en que vence) y `Persona::sujeta()`: no elige patrón, no se muda, casi sin escuela, su trabajo queda en el hogar (5 % para sí); vence sola; la ley de 1853 la termina para todos; en una herencia pasa con los herederos (se traspasaba). Felicidad: «vive bajo patronato». Prueba `bajo_patronato_no_elige_patron_ni_se_muda` |
| 7 | Población esclavizada: 20,8 % (1796), ~21 % (1806), 20 % (1816), sin baja | calibración, `poblacion.md` §8 | **Impreciso** (bajo en 1805, sin la baja de la revolución) | ~1/3 en 1805; 30 % → < 25 % entre 1810 y 1819; 25 % en 1830 | Con la nueva trata, la leva de 1816 y menos libertades «graciosas»: 9,7 % (1776), 22,6 % (1796), 26,1 % (1806), 18,9 % (1816). Prueba: entre 15 y 35 % en 1805 |
| 8 | Población indígena ~5 % todo el siglo | `origenes.rs:105-106` (misiones 6 %, región 1 %) | **Impreciso** | < 2 % a fines de los 1770 | `factor_origen`: los orígenes indígenas pesan todo en la década de la fundación y 0,3–0,6 después: 1,0–2,0 % desde 1776 (contando la ascendencia por padre y madre) |
| 9 | Caminos a la libertad iguales para varones y mujeres; el peculio sale solo del jornal (los varones ganan más) | `economia.rs:391-396`, `economia.rs:584-616` | **Impreciso** | 67 % de las compras fueron de mujeres; trabajo propio en domingos y festivos; las familias liberaban primero a la esposa y madre | Peculio con trabajo de días de fiesta (más para las mujeres); nuevas vías `Familia` (el hogar libre del cónyuge pone lo que falta) y `Colecta` (cofradía o sala de nación). Ahora ~60 % de las compras son de mujeres. Prueba `caminos_a_la_libertad_de_la_epoca` |
| 10 | La libertad que da el amo en vida se llama «por testamento»; al morir sin herederos, «libertad por testamento» | `economia.rs:606-610`, `demografia.rs:356-362` | **Impreciso** | Concesión graciosa en vida (carta ante escribano); la herencia vacante iba a la Real Hacienda | Vías `Otorgada` y `SinHerederos` con su texto; se documenta que el juego no modela el paso a la Real Hacienda (sería tratar a alguien como bien) |
| 11 | Probabilidad de libertad por testamento 8–43 % por muerte del jefe | `demografia.rs:290` | **Alto** [E] | Con la colecta y la familia, el total de libertades daba ~3 % anual | 2–12 %; libertad otorgada en vida 0,02–0,06 % mensual [E] |
| 12 | Sin apellido se muestran como «Rosa, nacida en el Congo y Angola» y «lo toman al casarse o al obtener la libertad» | `mod.rs:413-421`, `nombres.rs`, `poblacion.md` §5 | **Impreciso** | Se registraban con nombre cristiano y «nación»; el apellido solía ser el de la casa (bautismo) | «Rosa, de nación Benguela» (determinista por id: `nombres::nacion`); al ser libre, 60 % toma el apellido de la casa. No se toma apellido al casarse (se corrigió el texto) |
| 13 | No hay cofradías, salas de nación, reyes ni candombe | — | **Faltaba** | Cofradías 1773 y 1787; salas de nación desde fines del XVIII | Hechos en la crónica (y, fuera del modo fiel, cuando la comunidad es numerosa); la causa «Fe y fiestas» sube para las personas afrodescendientes; colectas para la libertad; Día de Reyes cada enero |
| 14 | No hay milicias ni libertad por las armas | — | **Faltaba** | Regimiento de pardos y morenos (1760), manumisión por leva (desde 1762), leva de 1816 y de 1842 | Vía `Armas`; leva de 1816 (20 % de los varones de 16 a 40 años, [E]); los varones afrodescendientes libres eligen a veces el oficio de soldado (milicia) |
| 15 | Las personas afrodescendientes libres podían ser clérigos o escribanos en la colonia | `demografia.rs:939-975` | **Impreciso** [NV] | La «limpieza de sangre» les cerraba el clero, las escribanías y los oficios reales (conocimiento general, no verificado en una fuente de Montevideo) | Restricción en la época colonial (`elegir_oficio`, `afro_colonial`): pasan a maestro o artesano |
| 16 | El Caserío de los Negros no aparece | — | **Faltaba** | Chagas y Stalla, en Frega et al. 2020 | Hecho de 1788 en el modo fiel; fuera de él, cuando llegaron 300 personas por la trata |
| 17 | Las cuadrillas guaraníes son «libres, con salario» | `poblacion.md` §5 | **No verificado** | Las fuentes leídas no dicen cómo se les pagaba | Se cambió el texto; queda como pregunta |
| 18 | Minuanes y charrúas «se integran como peones» con familias | `origenes.rs`, `poblacion.md` §5 | **Impreciso** | Lo documentado en Montevideo es el reparto forzado de prisioneros de 1831 | Peso de llegada reducido; hecho de Salsipuedes (mayo de 1831): mujeres, niñas y niños repartidos de a uno en familias, bajo una servidumbre modelada como patronato [E: plazo por analogía con 1837] |
| 19 | El tráfico «se corta con la libertad de vientres» y la trata existe en «las dos primeras épocas» | `poblacion.md` §9 | Correcto con matiz | Los colonos y el contrabando siguieron | Documentado |
| 20 | Fondo de manumisión del cabildo | `economy.rs:218` | **Ucrónico** | No hay noticia de un fondo así en Montevideo | Se mantiene como política posible y la descripción lo aclara |
| 21 | La abolición por ordenanza libera sin leva ni patronato | `economy.rs:220` | Decisión de diseño | — | La descripción lo dice y cuenta lo que pasó en 1842 |
| 22 | Incentivos del jugador | `economy.rs`, `desarrollo/ciudad.rs:109, 384, 417` | **Sin premio por esclavizar** | — | El jugador no puede aumentar la trata ni recibir dinero por ella; el trabajo de una persona sometida suma al hogar (restricción histórica) y, por la contribución, apenas al cabildo; los establecimientos del desarrollo **no** emplean personas sometidas (ahora tampoco bajo patronato). Las leyes de libertad molestan a los hacendados (opinión), lo que es una tensión histórica, no un premio |
| 23 | Escuela: «solo cuentan los niños libres» | `services/barrio.rs:6, 266` | Correcto | Pocas oportunidades en las aulas | Ahora excluye también a quienes están bajo patronato |
| 24 | Mortalidad ×1,3 de las personas esclavizadas | `demografia.rs:126` | Plausible [E] | Disentería al desembarcar, trabajo duro | Sin cambio |
| 25 | Nacidos libres por la ley viven con la madre (en la casa de quien la sometía) | `demografia.rs:430-450` | Plausible | Los constituyentes discutieron que serían «libres pero no independientes» | Sin cambio (no se les impone patronato: no se encontró una norma uruguaya de 1830–1842 que lo dispusiera) |
| 26 | `01-colonial.md`: «todos los nombres de lugares, naciones y acontecimientos son ficticios» | `docs/epocas/01-colonial.md:4-5` | **Contradicción** | La población usa fechas y leyes reales en el modo fiel | Pregunta abierta para el diseño (§6) |

**Problemas vistos fuera de este alcance (solo se informan, no se tocaron):**

- `docs/sistemas/arquitectura-colonial-fuentes.md` §5.7 habla de «la Cisplatina 1817–1828»: la
  dominación luso-brasileña de Montevideo va de enero de 1817 a 1829 (evacuación de la ciudad);
  «Cisplatina» es el nombre desde 1821. La fila de 1788 (Caserío de Filipinas) coincide con lo
  investigado aquí.
- Si la arquitectura agrega «cuartos de servidumbre o esclavizados» en el segundo patio, conviene
  que nunca muestren cadenas, cepos ni marcas que se lean como espectáculo; y que el Caserío, si
  llega a construirse, sea un **sitio de memoria** o un hecho narrado, no un edificio que el
  jugador construya o que dé ingresos.

## 4. Qué representa el juego y cómo

| Tema | Cómo |
|---|---|
| Condición legal | `Libre`, `Esclavizada`, `Liberta`; más la bandera `patronato` (mes en que vence). `Persona::sujeta()` = esclavizada o bajo patronato |
| Restricciones | Sin bienes raíces; no elige patrón (trabaja en el comercio o taller del hogar, o en la casa); no se muda por su cuenta (la pareja vive separada); casi sin escuela; su trabajo queda en el hogar (85 % menos su peculio; bajo patronato, 80 %); +30 % de mortalidad si está esclavizada |
| Llegadas forzadas | Trata (hasta 1829 en el modo fiel), colonos (1832–1835), prisioneros de Salsipuedes (1831). Las reciben los hogares más ricos, como en la época; nadie paga ni cobra |
| Caminos a la libertad | Ahorro (carta de libertad: 200 pesos [E] que no cobra ningún hogar del modelo), familia, colecta de la cofradía, fondo del cabildo (ucrónico), carta otorgada en vida, testamento, muerte sin herederos, armas, libertad de vientres, abolición |
| Al ser libre | Toma apellido (60 % el de la casa), arma hogar o se reúne con su cónyuge libre, los hijos chicos van con la madre; puede ascender (salvo clero y escribanías en la colonia) |
| Vida social | Cofradías (1773), San Baltasar (1787), salas de nación (c. 1790 [E]), Día de Reyes en la crónica, «Fe y fiestas» más alta |
| Hechos en la crónica (modo fiel) | Cofradías, Caserío (1788), leva de 1816, ley de la Florida (1825), libertad de vientres en Montevideo (1830), Constitución (1830), Salsipuedes (1831), colonos (1832–1835), ley de 1837, abolición (1842), Cerrito (1846), fin del patronato (1853) |
| Estadísticas | Esclavizadas, libertas, bajo patronato, afrodescendientes (por ascendencia) y con raíz indígena |

Calibración resultante (ciudad sintética, `cargo test --release calibracion -- --ignored --nocapture`):

```
1776  2.270 hab  esclavizadas  9,7 %  afrodescendientes 17,2 %  indígenas 2,0 %
1786  3.032 hab  esclavizadas 16,0 %  afrodescendientes 26,8 %  indígenas 1,3 %
1796  4.000 hab  esclavizadas 22,6 %  afrodescendientes 36,1 %  indígenas 1,2 %
1806  5.351 hab  esclavizadas 26,1 %  afrodescendientes 42,6 %  indígenas 1,0 %
1816  6.995 hab  esclavizadas 18,9 %  afrodescendientes 41,2 %  indígenas 1,4 %
1830  ~9.600 hab esclavizadas ~15 %
1842  ~14.000 hab esclavizadas ~5,6 % antes de la abolición
Libertades en 100 años: ~60 % de las compras (ahorro, familia, colecta) son de mujeres
```

«Afrodescendientes» cuenta a toda persona con un antepasado africano por padre o madre: es más
amplio que las categorías de los padrones (que no registraban como «pardas» a muchas personas
de ascendencia mixta), por eso queda por encima del 22–25 % de algunas estimaciones. Después de
1816 la parte esclavizada baja más rápido que en los padrones (25 % en 1830): la ciudad sintética
tiene más libertades y menos trata luso-brasileña de las que hubo; es un punto a ajustar si se
quiere más fidelidad.

## 5. Fuentes

Leídas para este trabajo:

1. Frega, Ana; Duffau, Nicolás; Chagas, Karla y Stalla, Natalia (coords.), con Borucki, Alex;
   Trías, Mariana; Martínez, Lucía; Thul, Florencia y Bolaña, María José. *Historia de la
   población africana y afrodescendiente en Uruguay*. Montevideo: FHCE (Udelar) y MIDES, 2020.
   ISBN 978-9974-0-1741-2.
   [PDF](https://www.fhce.edu.uy/images/Decanato/Historia_de_la_poblacion_africana_y_afro.pdf).
   Caps. 3 (trata; «El Caserío de Negros»), 4 (condiciones de vida y trabajo; formas de libertad),
   5 (revolución, república y abolición; tabla de disposiciones 1812–1862, pp. 136-137), 6
   (ciudadanía) y 7 (cofradías, salas de nación, candombe).
2. Borucki, Alex. «¿Es posible integrar la esclavitud al relato de la historia económica uruguaya
   previa a 1860?». *Boletín de Historia Económica* (AUDHE), año III, n.º 4, octubre de 2005.
   [PDF](https://www.audhe.org.uy/Boletin_Audhe/Boletin_4/AUDHE_No_04_Borucki.pdf).
3. Pollero, Raquel y González Demuro, Wilson. *Población y sociedad de Montevideo*. Colección
   Nuestra Montevideo, fascículo 4. Intendencia de Montevideo y FHCE, 2024.
   [PDF](https://fhce.edu.uy/wp-content/uploads/2025/12/Fasciculo4-poblacion.pdf) (ya usada en
   `poblacion.md`).
4. Ministerio de Educación y Cultura. «Rememorando leyes de libertad de vientres y prohibición
   del tráfico de esclavos». [gub.uy](https://www.gub.uy/ministerio-educacion-cultura/comunicacion/noticias/rememorando-leyes-libertad-vientres-prohibicion-del-trafico-esclavos)
   (da el 6 y el 7 de setiembre de 1825 y el 26 de octubre de 1846).
5. Andrews, George Reid. «Recordando África al inventar Uruguay: sociedades de negros en el
   carnaval de Montevideo, 1865-1930». *Revista de Estudios Sociales*, n.º 26 (2007), pp. 86-104.
   [OpenEdition](https://journals.openedition.org/revestudsoc/20713) (leído en parte).

Consultadas solo por resúmenes de búsqueda (datos marcados [NV]): sobre Salsipuedes,
[Wikipedia](https://en.wikipedia.org/wiki/Massacre_of_Salsipuedes) y documentos de la INDDHH en
gub.uy; sobre Bella Unión y San Borja del Yí, resúmenes que citan a Padrón Favre (*Censo
guaraní misionero de 1832*, 1991) y a Curbelo Salvo (2024).

Citadas a través de las anteriores, sin leerlas directamente: Borucki, *From Shipmates to
Soldiers* (2015); Borucki, Chagas y Stalla, *Esclavitud y trabajo. Un estudio sobre los
afrodescendientes en la frontera uruguaya (1835–1855)* (2004); Bentancur; Montaño; Isidoro de
María; Rama; Campagna; Ayestarán, *La música en el Uruguay* (1953); Curbelo y Onega (2005);
Bracco y López Mazz (2014); Frega (2004).

## 6. Preguntas abiertas para el diseño

1. **¿Ficticio o real?** `01-colonial.md` dice que lugares, naciones y acontecimientos son
   ficticios, pero el modo fiel usa leyes y fechas reales de Montevideo (también Salsipuedes y el
   Caserío). ¿Se mantienen los nombres reales en el modo fiel y genéricos en la historia
   alternativa (como está ahora)?
2. **Salsipuedes**: ¿cómo se muestra? Hoy es una noticia y unas decenas de personas repartidas.
   ¿Se agrega un aviso previo, una nota de contexto en la interfaz o un sitio de memoria? El plazo
   de su servidumbre (hasta los 25 años los niños, 3 años las mujeres) es una analogía con el
   patronato de 1837, no un dato.
3. **Saladeros y establecimientos**: en la historia usaban mucha mano de obra esclavizada (1841:
   57–62 personas por saladero en el Cerro). El juego no la usa en los establecimientos del
   jugador para no premiarlo. ¿Se muestra al menos en la crónica o en la ficha del saladero?
4. **Ventana de 1813–1817**: la libertad de vientres de la Asamblea del Año XIII rigió en la
   Provincia Oriental artiguista y los luso-brasileños la desconocieron. ¿Se modela (nacidos
   libres 1815–1816 que luego pleitean su condición) o solo se cuenta?
5. **Ciudadanía de 1830**: la Constitución suspendía la ciudadanía de sirvientes a sueldo,
   peones jornaleros, soldados de línea y analfabetos. ¿Pesa en la opinión o en el voto cuando
   existan elecciones en la época 2?
6. **Abolición por ordenanza**: ¿debería tener también leva y patronato (lo histórico) o
   mantenerse como abolición plena (la decisión del jugador «mejor» que la historia)?
7. **Conchabo**: el trabajo de las personas sometidas conchabadas fuera de la casa ya suma al
   hogar como restricción. ¿Se muestra en la ficha del hogar como «trabajo de personas
   sometidas» para que el jugador vea la desigualdad, sin convertirlo en una cifra de ganancia?
8. **Cuadrillas guaraníes de 1726–1730**: falta verificar en qué condiciones trabajaron (pago,
   ración, turnos, quién recibía el jornal).
9. **Calibración después de 1816**: la parte esclavizada baja más rápido que en los padrones
   (25 % en 1830); ¿se ajusta la trata luso-brasileña o las libertades?
