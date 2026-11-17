# Población: personas, familias, linajes, desigualdad y felicidad

> Documento de sistema. Complementa al [GDD](../GDD.md) (secciones 4.6 y 4.7), a
> [políticas y recursos](politicas-y-recursos.md) y a [zonas](zonas.md). Estado: **implementado
> en el motor** (`engine/src/city/poblacion/`, paneles en `engine/src/ui/poblacion.rs`).

## 1. Qué hace

Cada habitante es una **persona** con nombre, apellidos, sexo, fecha de nacimiento, padres,
cónyuge, hijos, hogar, lugar de nacimiento, condición legal, estamento, oficio, educación,
salud, riqueza propia, personalidad, felicidad y una **historia de vida**. Las personas viven
en **hogares** con su propia economía (ingresos, gastos, ahorro, deuda, solares, ganado). Mes a
mes nacen, mueren, se casan, heredan, aprenden a leer, eligen oficio, trabajan, se mudan,
emigran y llegan familias nuevas de los orígenes de cada época. Los **linajes** salen de seguir
padre y madre hacia atrás; los apellidos se heredan a la usanza hispana (paterno + materno).

La población se conecta con la ciudad por dos estructuras planas (`Entorno` e `InformeMes`):

| La ciudad le da a la población | La población le devuelve a la ciudad |
|---|---|
| Solares (tipo, nivel, capacidad, empleos, valor del suelo, distancia a la plaza) | Gente sin casa más la que sobra en casas hacinadas → el `pool` de "esperan casa" que hace crecer las casas (demanda residencial) |
| Precio del pan y de la carne (mercado de `economy.rs`) | Felicidad media por sector → se **suma** a las causas de la opinión de vecinos, comerciantes y hacendados (no la reemplaza) |
| Agua, epidemia, sequía, tasa de la contribución, ordenanzas vigentes | Mejoras de casa que pagan sus dueños (rancho → azotea → altos), si el solar lo permite |
| Personas que llegan este mes (las decide la ciudad: trabajo, opinión, impuestos, felicidad) | Líneas del presupuesto: políticas sociales, venta de solares, alquileres del cabildo |
| | Noticias para la gaceta y la crónica |

## 2. Modelo de datos

- `Persona` (`modelo.rs`): ~150 bytes más su historial. Los nombres y apellidos son índices en
  un registro de textos (`Poblacion::textos`); padres, cónyuge, hijos y hogar son ids (`u32`).
  Almacenamiento compacto: `Vec<Persona>` indexado por id (el id es la posición), sin `Rc` ni
  grafos de punteros. Las personas muertas o que se fueron quedan (el árbol del linaje sigue
  completo) y, pasados 40 años, guardan solo los hechos principales de su vida.
- `Hogar`: miembros, jefe, origen, alojamiento (solar, sin casa, cuartel, campamento, campo),
  ingresos y gastos del mes con desglose, ahorro, deuda, solares propios, ganado, clase.
- `Evento`: mes, tipo y persona relacionada (12 bytes); el texto se arma al mostrarlo.
- Personalidad (`Rasgos`, 5 bytes): **laboriosidad, ambición, devoción, sociabilidad y
  prudencia**. Pesan en el trabajo (productividad), el ahorro, la compra de solares y mejoras,
  los casamientos, la emigración, los legados piadosos y las libertades por testamento, y en
  cuánto le importa a cada uno cada causa de felicidad.
- Índices derivados (no se guardan): personas vivas, hogares por solar, trabajadores por solar
  (índices compactos en tres vectores, búsqueda binaria).
- Azar determinista propio (`Rng` con su estado guardado): guardar y cargar da la misma
  población y la misma continuación (prueba `guardar_y_cargar_da_la_misma_poblacion` y la de
  `city/save.rs`).

## 3. Demografía (mensual)

- **Muertes**: probabilidad anual por edad (`q_anual`) × salud × clase (pobres ×1,25,
  principales ×0,75) × sin casa × hambre × condición (esclavizadas ×1,3) × epidemia; el
  hospital de caridad baja la de pobres y modestos.
- **Nacimientos**: mujeres en pareja de 15 a 49 años, fecundidad por edad (`fecundidad`) ×
  salud × felicidad; al menos 15 meses entre partos; mortalidad materna ~1 %.
- **Casamientos y uniones**: las solteras y viudas se casan según la edad
  (`nupcialidad`); se elige entre 16 candidatos con puntaje: diferencia de edad cercana a 8
  años, mismo grupo de origen (×2), misma raíz (×1,5), misma clase (×1,8; dos clases de
  distancia ×0,35), condición legal, oficio y casa. La **mezcla es posible** (solo pesa
  menos). Las uniones pueden ser casamiento en la iglesia o **unión consensual** (más
  frecuentes en los sectores populares). Dote: 8–20 % del ahorro de la familia de la novia.
- **Hogares**: los recién casados arman casa (y buscan solar) salvo que el novio ya sea jefe;
  dos jefes juntan sus casas. Las personas esclavizadas no pueden mudarse: la pareja vive
  separada hasta que las dos sean libres.
- **Herencia** (derecho castellano): la mitad de los gananciales es del cónyuge que sigue al
  frente de la casa; de la otra mitad, **cuatro quintos son la legítima** de los hijos por
  partes iguales y **el quinto de libre disposición** va a misas y obras pías si el difunto era
  devoto. Los herederos que viven en otra casa cobran en efectivo (lo que falta es deuda: de
  ahí los **pleitos** que cuenta la crónica). Sin nadie libre en la casa, el hogar se
  disuelve y todo se reparte (solares, ganado, dinero); sin herederos, los solares vuelven al
  cabildo. **La riqueza se conserva exactamente** (prueba `la_herencia_conserva_la_riqueza`).
- **Migraciones**: la guarnición rota y se completa; las cuadrillas guaraníes de la obra
  vuelven a sus pueblos (algunos se quedan como vecinos); los hogares libres descontentos se
  van (menos si tienen bienes).
- **Crecer**: escuela de primeras letras (6–12 años) según la clase y las escuelas del cabildo;
  oficio a los 15 (el del padre o, si sabe leer y es ambicioso, uno letrado: comercio,
  escribanía, magisterio); a lo largo de la vida, un peón o artesano que sabe leer puede pasar
  a pulpero o comerciante (movilidad social).

## 4. Economía de cada hogar

Montos en pesos de plata por mes (estimaciones de orden de magnitud, ver §8):

- **Ingresos**: salario o ganancia de cada oficio × productividad (felicidad y laboriosidad);
  las mujeres cobran ×0,6; sin vacante en un comercio o taller, por cuenta propia ×0,75;
  rentas de casas alquiladas; margen de los dueños de comercios y talleres por empleado;
  ganado (cría logística hasta ~2.000 cabezas; la sequía lo achica).
- **Gastos**: comida = adultos equivalentes × 2 pesos × (0,45 pan + 0,15 carne + 0,40 resto)
  **al precio del mercado** de `economy.rs`; alquiler (al dueño o al cabildo); contribución;
  otros gastos según la clase y la prudencia.
- **Ahorro, deuda, hambre**: lo que sobra se ahorra; lo que falta se pide prestado hasta un
  límite; lo que no se consigue se come de menos (meses de hambre → salud, muertes,
  felicidad).
- **Vivienda**: los ricos eligen el mejor suelo, los pobres el más barato; sin lugar, algunos
  se agregan a casas ajenas (hacinamiento); los inquilinos hacinados se mudan cuando hay lugar.
  Los fundadores recibieron merced de solar; los demás compran si pueden, si no alquilan. Los
  ricos compran casas para alquilar; comerciantes y artesanos compran sus negocios.
- **Mejoras**: el dueño que juntó lo suficiente paga la mejora y la ciudad la construye si el
  solar lo permite (calle empedrada, valor del suelo, densidad, antigüedad).
- **Clase**: pobre, modesta, acomodada o principal según patrimonio e ingreso por adulto.
  **Estamento**: vecino (jefe con casa propia poblada), morador, militar, clero.

## 5. Representación cuidadosa de la esclavitud y de los pueblos indígenas (decisión tomada)

**Decisión del usuario**: las personas africanas y afrodescendientes (esclavizadas y libres) y
las indígenas (guaraníes de las misiones, minuanes y charrúas) existen en la simulación como
**personas con nombre, familia e historia**. La desigualdad legal de la época es una
**restricción** sobre sus vidas, **nunca una mercancía**.

Cómo está implementado:

| Principio | Implementación |
|---|---|
| Ninguna persona tiene precio | `Persona` no tiene campos de precio, valor ni dueño (la prueba `no_hay_forma_de_comerciar_personas` lo verifica en el código fuente). El patrimonio de los hogares no incluye personas |
| No hay comercio de personas | No existe ninguna función para comprar, vender, subastar ni transferir personas, ni ninguna ordenanza o tarjeta de la interfaz que lo haga. Las llegadas por la trata son un **flujo del mundo** que el juego narra: no lo decide el jugador, nadie paga ni cobra en el modelo (la prueba verifica que la riqueza no cambia) |
| La esclavitud es una restricción | Condición legal `Esclavizada`: no puede tener solares, no elige patrón (trabaja donde lo manda el hogar), no se muda por su cuenta; su trabajo suma al ingreso del hogar donde vive; casi sin acceso a la escuela; más mortalidad. Pesa como ninguna otra causa en su felicidad ("vive sin libertad") |
| Son personas | Nombre cristiano como en los registros de la época y su lugar de origen (Congo y Angola, Costa da Mina, Mozambique); llegan sin apellido y lo toman al ser libres (sus hijos también). Forman pareja, tienen hijos, ahorran un **peculio**, tienen oficio, personalidad e historia |
| Caminos a la libertad | **Ahorro** (carta de libertad: la suma exigida por la ley sale del peculio y no la cobra ningún hogar del modelo), **testamento** (al morir quien la sometía, según su devoción; y sin herederos), **fondo de manumisión** del cabildo (política), **libertad de vientres** (desde la ley, nadie nace esclavizado) y **abolición**. Al ser libre: toma apellido, arma su hogar o se reúne con su familia y busca casa y trabajo; puede ascender como cualquiera |
| Las leyes históricas | En el modo fiel a la historia: libertad de vientres y prohibición del tráfico en **1825** (ratificadas en la Constitución de **1830**) y abolición en **1842** (gobierno de la Defensa; la ley del Cerrito es de **1846**). El jugador puede adelantarlas por ordenanza; **no se pueden derogar**. En la historia alternativa llegan con la época de la Ciudad Nueva |
| La crónica narra con respeto | Sin insultos ni caricaturas: "Llegan en un barco 12 personas del Congo y Angola traídas por la trata, arrancadas de sus pueblos y sometidas a la esclavitud"; "Rosa, lavandera, obtiene su carta de libertad con lo que ahorró durante 23 años"; "Tras la muerte de X, Y sigue sin libertad y pasa a vivir con la familia Z" |
| Indígenas | Las cuadrillas guaraníes de las misiones (libres, con salario) trabajan en la obra del fuerte en la fundación; la mayoría vuelve a sus pueblos y algunos se quedan como vecinos. Minuanes y charrúas aparecen como personas que se integran (peones). Apellidos de raíz guaraní alternados con castellanos, como en los registros |

## 6. Felicidad

Cada persona tiene una felicidad (0 a 1) que sale de once causas, cada una entre 0 y 1:
**comida** (lo que gana el hogar frente al costo de comer, el precio del pan, el hambre),
**vivienda** (sin casa, cuartel, rancho/azotea/altos, hacinamiento), **agua**, **salud**,
**trabajo** (desocupado, por cuenta propia), **familia** (pareja, hijos, viudez, duelo por
muertes recientes, soledad), **libertad** (la esclavitud la hunde; la libertad la sube),
**comparación** con su barrio y con los de su mismo origen (desigualdad percibida), **fe y
fiestas** (para los devotos; la iglesia matriz) y **ordenanzas** que la tocan (pan barato,
hospital, escuelas, reparto, contribución progresiva, préstamos, fondo de manumisión, faroles a
cargo de los vecinos, prohibición de la paja) y **seguridad** (el barrio: el centro es más seguro
que las afueras; no tener techo; haber perdido la casa en un incendio o una inundación).

- La **personalidad** cambia los pesos: la prudencia agranda comida, vivienda y seguridad; la laboriosidad
  y la ambición, el trabajo; la sociabilidad, la familia; la ambición, la comparación; la
  devoción, la fe.
- Total = 0,65 × media ponderada + 0,35 × la peor causa importante (una sola cosa grave —no
  ser libre, el hambre, no tener casa— no se compensa con lo demás). Cambia de a poco (35 % por
  mes hacia el valor nuevo).
- **Motivos**: las dos causas que más pesan por debajo de lo aceptable ("le preocupa el precio
  del pan", "vive hacinado", "no tiene casa", "vive sin libertad", "ve que sus vecinos viven
  mejor"…), en la ficha de cada persona y contados para toda la ciudad.
- **Agregados** por edad, sexo, origen, clase, condición legal y barrio (centro, intramuros,
  afueras, sin casa), con historial mensual.
- **Consecuencias**: la felicidad media de cada sector se suma a la opinión de
  `economy.rs` (±15 puntos); la felicidad de la ciudad atrae o espanta llegadas (×0,8 a
  ×1,15); los descontentos emigran; rinde más quien está contento (productividad); la natalidad
  baja un poco con la infelicidad; con más del 35 % de adultos descontentos hay protestas.

## 7. Desigualdad y políticas

**Métricas** (panel *Población → Desigualdad*, con historia anual): Gini de la riqueza y del
ingreso (entre hogares), parte de la riqueza del 10 % más rico, pobreza (personas en hogares
pobres) por origen y por condición legal (las personas esclavizadas aparecen como "sin derecho
a bienes"), riqueza media por origen.

**Políticas del cabildo** (ordenanzas de `economy.rs`, se aprueban y derogan como las demás;
el costo real sale en el presupuesto según la población):

| Ordenanza | Efecto | Costo | Opinión (vec./com./hac.) |
|---|---|---|---|
| Reparto de solares a familias pobres | Los hogares pobres sin casa reciben en propiedad los ranchos libres | 25 pesos/mes | +8 / −3 / −5 |
| Contribución progresiva | Pobres exentos; acomodados ×1,5, principales ×2,5 (recargo para el cabildo) | La exención | +8 / −10 / −10 |
| Pan barato para los pobres | Los hogares pobres o con hambre pagan el pan a precio normal | La diferencia | +10 / −3 / 0 |
| Hospital de caridad y asistencia a pobres | Menos muertes y mejor salud de pobres y modestos (en la ciudad del motor: pobres y modestos aprovechan del todo la botica, el médico y el hospital cercanos, ver [educacion-salud-seguridad.md](educacion-salud-seguridad.md)) | 20 + 30 por 1.000 hab. | +6 / 0 / 0 |
| Escuelas de primeras letras | +20 puntos de chance de aprender a leer por año de niñez: más oficios letrados (en la ciudad del motor: el cabildo paga al maestro y los pobres van gratis a la escuela de su barrio; hacen falta escuelas) | 15 + 0,25 por niño | +5 / +2 / 0 |
| Préstamos del cabildo | Préstamo barato a pobres y modestos: menos hambre en los meses malos | Intereses y morosidad | +6 / −4 / 0 |
| Fondo de manumisión | Completa la suma de la carta de libertad a quien juntó más de un tercio | 40 pesos/mes | +3 / −2 / −8 |
| Libertad de vientres | Nadie nace esclavizado y se prohíbe el tráfico. No se deroga | — | +4 / −4 / −8 |
| Abolición de la esclavitud | Todas las personas esclavizadas son libres. No se deroga | — | +5 / −5 / −12 |

**Épocas siguientes** (como datos en `politicas.rs`, para cuando existan esas épocas): escuela
pública gratuita, reglamento de conventillos, jornada de ocho horas, salario mínimo,
jubilaciones y pensiones, asignaciones familiares, vivienda cooperativa por ayuda mutua,
control de precios, alquiler protegido, transferencias a hogares vulnerables.

## 8. Calibración

Fuente verificada principal: **Raquel Pollero y Wilson González Demuro, *Población y sociedad
de Montevideo*, colección Nuestra Montevideo, fascículo 4 (Intendencia de Montevideo / FHCE,
2024)**, que resume la tesis de Pollero (*Historia demográfica de Montevideo y su campaña,
1757-1860*, FCS-Udelar, 2016)
([PDF](https://fhce.edu.uy/wp-content/uploads/2025/12/Fasciculo4-poblacion.pdf)).

| Dato | Valor de la fuente | Fuente | En el motor |
|---|---|---|---|
| Fundadores de Buenos Aires | 6 familias, 33 personas, antes de los canarios | Pollero y G. D. (verificado) | 6 familias, 33 personas al fundar |
| Canarios | Navío *Nuestra Señora de la Encina* (1726); los contingentes canarios suman algo más de 200 personas | Pollero y G. D. (verificado). La fecha (noviembre de 1726), el segundo barco (marzo de 1729, ~30 familias) y el número de familias del primero (12, 20 o 25 según la fuente) **no coinciden entre fuentes** | 20 familias / 96 personas en noviembre de 1726; 25 / 110 en marzo de 1729 (modo fiel) |
| Montevideo en 1728 | ~350 personas: un centenar de canarios, otro de indios traídos por los jesuitas, cien soldados, ~50 de oficios | Zabala, citado por Pollero y G. D. (verificado) | Guarnición de 100 soldados y cuadrilla de 100 guaraníes de las misiones al fundar |
| "Mil indios tapes" de las reducciones para el fuerte | Crónica citada en *La Mañana* | (cifra de crónica, no usada) | — |
| Población hacia 1760 | ~2.000 (1.991 en 1757) | Pollero y G. D. (verificado) | La ciudad sintética llega a ~1.100 en 1756; en el juego depende de lo que zonifique el jugador |
| Fines del siglo XVIII | "algo menos de diez mil" | Pollero y G. D. | ~4.600 en 1806, ~8.600 en 1826 (sintética) |
| Tasa bruta de mortalidad | 25 a 35 ‰ | Pollero y G. D. (verificado) | 27–32 ‰ (1746–1816, ciudad sintética) |
| Mortalidad infantil | ~1/5 antes del primer año | Pollero y G. D. (verificado) | 200–290 ‰ |
| Esperanza de vida al nacer | 33 a 41 años | Pollero y G. D. (verificado) | 37,6 años con la tabla `q_anual` (prueba) |
| Natalidad | > 40 ‰; ~5 hijos por mujer | Pollero y G. D. (verificado) | 28–38 ‰ (población muy masculinizada: algo por debajo) |
| Edad de los esposos | Mujeres jóvenes con hombres mayores; diferencia media ~9 años a mediados del XIX; 20 % de matrimonios de 1812–13 con 20+ años de diferencia | Pollero y G. D. (verificado) | Preferencia por 8 años de diferencia (±7) |
| Masculinidad | > 150 hombres por 100 mujeres hacia 1760; 124 en 1860 | Pollero y G. D. (verificado) | 340 al fundar (soldados y cuadrillas) → ~140 a fines del XVIII |
| Población joven | > 40 % menor de 20 años | Pollero y G. D. (verificado) | Edad media 25–28 años |
| Uniones libres | Nacimientos de uniones libres: > 40 % (1780s), ~20 % (fines del XVIII), ~40 % (primeras décadas republicanas), 15 % (1860) | Pollero y G. D. (verificado) | Unión consensual: 40 % en hogares pobres, 20 % en modestos, 3 % en acomodados (estimación) |
| Población afrodescendiente | > 25 % a fines de la década de 1770; ~30 % esclavizada hacia 1810 (Buenos Aires y Montevideo) | Pollero y G. D. (verificado) | ~18 % esclavizada en 1786, ~21 % en 1806 (sintética; algo por debajo) |
| Trata | ≥ 70.000 personas esclavizadas entraron al Río de la Plata entre 1777 y 1812 | Borucki, citado por Pollero y G. D. (verificado) | La trata pesa ×3 entre 1777 y 1812 (estimación) |
| Leyes de libertad | Libertad de vientres y prohibición del tráfico 1825, ratificadas en 1830; abolición 1842 (Defensa) y 1846 (Cerrito) | Pollero y G. D. (verificado) | Eventos del modo fiel y ordenanzas |
| Indígenas | < 2 % a fines de los 1770s; la mayoría guaraníes de las misiones | Pollero y G. D. (verificado) | Cuadrillas de la fundación y 6 % de las llegadas coloniales |
| Crisis de mortalidad | 18 entre mediados del XVIII y 1880 (viruela, sarampión, fiebre amarilla 1857, cólera 1868) | Pollero y G. D. (verificado) | Hook `choque_epidemia`; la ciudad sintética tiene una cada 7 años |
| Orígenes en 1836 | 50 % orientales, 37 % extranjeros: de ellos la mitad europeos (63 % canarios, 13 % portugueses), ~30 % africanos, 15 % de la región | Pollero y G. D. (verificado) | Pesos de la tabla colonial (estimación guiada por estos datos) |
| Orígenes en 1860 | 48 % extranjeros; europeos: 1/3 españoles, 1/3 italianos, 27 % franceses | Pollero y G. D. (verificado) | Tabla de la Ciudad Nueva |
| Inmigración judía | ~50.000 entre fines del XIX y mediados del XX; ~10.000 entre 1933 y 1941 | Resultado de búsqueda (no verificado en la fuente primaria) | Peso de la tabla de Art Déco (estimación) |
| Herencia | Legítima de cuatro quintos en el derecho castellano histórico (el Código Civil la bajó a dos tercios) | [elderecho.com](https://elderecho.com/) (verificado); la mitad de gananciales del cónyuge: conocimiento general, **no verificado** | 4/5 legítima, 1/5 libre, 1/2 gananciales |
| Salarios y precios | — | **Estimación** de orden de magnitud: peón 7, artesano 12, pulpero 14, comerciante 32 pesos/mes; subsistencia 2 pesos por adulto; casas 40 / 450 / 1.500 pesos; carta de libertad 200 pesos | — |
| Riqueza y oficios por origen | — | **Estimación** (`origenes.rs`) | — |
| Tablas por edad | — | **Modelo propio** ajustado a las cifras de arriba | `q_anual`, `fecundidad`, `nupcialidad` |

Resultado de la ciudad sintética (`cargo test --release calibracion -- --ignored --nocapture`,
inmigración del 2 % anual, una epidemia cada 7 años), década por década:

```
1736 hab    529 TBN 15 TBM 27 TMI 250 edad 26.5 hogar 2.4 h/m 197 esclavizadas  3.6 % Gini 0.63 pobreza 0.08 felicidad 0.59
1756 hab  1 097 TBN 28 TBM 32 TMI 290 edad 29.0 hogar 2.6 h/m 172 esclavizadas  9.3 % Gini 0.65 pobreza 0.07 felicidad 0.61
1776 hab  2 079 TBN 31 TBM 27 TMI 231 edad 27.6 hogar 3.1 h/m 151 esclavizadas 11.6 % Gini 0.72 pobreza 0.07 felicidad 0.60
1796 hab  3 603 TBN 32 TBM 32 TMI 250 edad 26.8 hogar 3.5 h/m 147 esclavizadas 20.8 % Gini 0.74 pobreza 0.09 felicidad 0.57
1816 hab  6 046 TBN 34 TBM 31 TMI 205 edad 26.7 hogar 3.5 h/m 142 esclavizadas 20.3 % Gini 0.74 pobreza 0.11 felicidad 0.56
```
(Los números exactos cambian con los ajustes; la prueba `crece_desde_la_fundacion_a_ritmo_plausible`
fija los rangos: natalidad 28–50 ‰, mortalidad 18–40 ‰, mortalidad infantil 120–300 ‰,
edad media 20–33, hogares de 2 a 7 personas, ciudad masculinizada.)

## 9. Épocas e inmigración

`origenes.rs` tiene una tabla por época con el peso de cada origen, el tamaño de las familias,
la riqueza al llegar (mediana y dispersión), la alfabetización y los oficios:

| Época | Orígenes principales |
|---|---|
| Colonial | Canarias, Buenos Aires, Galicia, Andalucía, Cantábrico, Cataluña, País Vasco, Portugal, Brasil, misiones guaraníes, minuanes y charrúas, interior; llegadas forzadas por la trata (Congo y Angola, Costa da Mina, Mozambique) |
| Ciudad Nueva | Canarias, Galicia, País Vasco, Francia, Italia, Buenos Aires, Brasil, Portugal, islas británicas, interior |
| Inmigración | Italia (la mayor), Galicia, País Vasco, Cataluña, Andalucía, Francia, islas británicas, Alemania, Siria y Líbano, comunidades judías, Armenia, Brasil, Buenos Aires, interior |
| Art Déco | Interior, Galicia, Italia, refugiados judíos, Siria y Líbano, Armenia, Alemania |
| Modernismo | Sobre todo del interior; Buenos Aires, Brasil, Paraguay |
| Contemporánea | Interior, América Latina no fronteriza (Perú, Venezuela, Cuba, República Dominicana), Buenos Aires, Brasil, Paraguay, retornados |

La trata solo existe en las dos primeras épocas y se corta con la libertad de vientres.

## 10. Para los demás sistemas (API)

- Agentes móviles: `Poblacion::habitantes_de_lote(lote)`, `hogares_de_lote(lote)`,
  `trabajadores_de_lote(lote)`, `lugar_de_trabajo(persona) -> Option<LoteId>`,
  `casa_de(persona)`, `persona(id)`, `vivos()`. Un agente guarda `persona: Option<u32>`.
- Servicios: `choque_epidemia(virulencia, Option<&[lotes]>)`, `choque_incendio(lote)` (deja al
  hogar sin casa), `choque_inundacion(lotes_mojados, lotes_perdidos)` (ya conectado a las
  sudestadas y crecientes de `City::flood`), `choque_hambruna(severidad)`. Devuelven las muertes.
- Ciudad: `City::habitantes()`, `City::lotes_info()`, `City::entorno_actual()`.

## 11. Rendimiento

- Colonial: 100 años desde la fundación en ~4,3 s (3,5 ms por mes de media) con ~8.650
  habitantes al final y 18.850 personas registradas (vivas y muertas); ~10 MB en memoria
  (proceso de prueba: 46 MB); la población ocupa ~22 MB en la partida guardada (JSON).
- Épocas tardías: con 100.000 personas, ~75–90 ms por mes (un mes de juego dura 10 s).
- Para cientos de miles: `limite_individuos` (250.000 por omisión) pasa a una **población
  agregada** por edad y sexo (proyección anual con las mismas tablas) a las personas que llegan
  solas sin familia; las familias siguen individuales para conservar los linajes. Las
  personas muertas hace más de 40 años guardan solo los hechos principales.

Banco de pruebas: `cargo test --release banco_poblacion -- --ignored --nocapture`.

## 12. Interfaz

- Ficha **Población** en la barra superior (habitantes y felicidad) → panel con pestañas:
  *Resumen* (pirámide, nacimientos y muertes del año, orígenes, clases, condición legal,
  historia anual), *Desigualdad* (Gini, 10 % más rico, pobreza por origen y condición,
  políticas vigentes y futuras), *Felicidad* (por grupo y motivos principales, peso en la
  opinión) y *Crónica*.
- Ficha de un solar → botón **Vecinos**: las familias de la casa, su economía del mes
  (desglose), dueña o inquilina, y sus miembros.
- **Ficha de una persona**: datos, felicidad desglosada en sus doce causas (la duodécima, «Educación», con las escuelas del barrio) con los motivos,
  personalidad, familia (abuelos, padres, cónyuge, hermanos, hijos, nietos), linaje con sus
  generaciones y descendientes vivos, e historia de vida.
- La gaceta recibe las noticias notables (llegadas, libertades, casamientos entre familias
  principales, pleitos de herencia, leyes, epidemias, protestas).

## 13. Pendiente

- Barrios con nombre y mapa de capas (felicidad, pobreza, hacinamiento por solar).
- Estancias y chacras propias del jugador: hoy el ganado de los hacendados es abstracto.
- Cuando existan los agentes móviles: que la rutina de cada persona use `lugar_de_trabajo`.
  La escuela, la salud, la insalubridad y el delito ya son por solar (ver
  [educacion-salud-seguridad.md](educacion-salud-seguridad.md)).
- Épocas 2–6: activar sus políticas (`FUTURAS`), salarios y precios por época, conventillos.
- Patronato de los libertos (1825–1840) y otros matices jurídicos (capacidad de las mujeres
  casadas, hijos naturales sin acceso a la educación) como restricciones adicionales.
