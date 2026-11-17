# Desarrollo: saberes, tecnologías, industria, inversión y comercio exterior (motor)

> Documento de sistema. Complementa al [GDD](../GDD.md) (sección 4.7, «especialización de la
> ciudad»), a [políticas y recursos](politicas-y-recursos.md), a [población](poblacion.md), a
> [educación, salud y seguridad](educacion-salud-seguridad.md) y a [servicios](servicios.md).
> Estado: **implementado en el motor para la época colonial**; las épocas siguientes quedan
> como datos (tecnologías y perfiles de industria con `epoca`).
> Código: `engine/src/city/desarrollo/` (`datos.rs` tablas, `mod.rs` modelo puro, `ciudad.rs`
> enganche con la ciudad, `tests.rs`), `engine/src/city/services/mod.rs` (seis edificios
> nuevos en el catálogo), `engine/src/structures/colonial.rs` (vóxeles),
> `engine/src/ui/desarrollo.rs` (panel y cajón).

## 0. La idea

El pedido: *«tiene que haber una mecánica de tecnologías nuevas: si la educación se desarrolla y
se quiere desarrollar la industria, hay que crear una escuela industrial, invertir dinero en ella
y favorecer la inversión interna en industrias que no necesariamente necesitan más empleados pero
generan exportaciones, para que haya ese flujo de recursos distintos».*

La cadena del juego es:

```
escuelas de primeras letras ─▶ gente que lee ─▶ escuela de oficios ─▶ oficiales
                                                      │
       inversión del cabildo (cuotas) ─▶ tecnología ──┴─▶ industria nueva habilitada
                                                             │
   incentivos (exención, crédito, aranceles, privilegios) ─▶ inversores privados
                                                             │
           establecimiento: compra insumos, contrata peones, usa oficiales, exporta
                                                             │
  plata de afuera ─▶ aduana del cabildo · ganancia del dueño · salarios de los peones
                     (desigualdad y felicidad en la población)
```

## 1. Saberes

Se cuentan desde la población cada mes (`City::desarrollo_tras_poblacion`):

| Saber | De dónde sale | Para qué |
|---|---|---|
| Saben leer | Adultos (15+) con `educacion ≥ 1` (escuelas de primeras letras y convento) | Requisito de la escuela de oficios, la náutica y la imprenta; alumnos de la escuela de oficios |
| Artesanos | Adultos con oficio de artesano | Requisito del curtido mejorado |
| Oficiales formados | Personas concretas (`Desarrollo::formados`): egresados de la escuela de oficios (30 alumnos, 2 años: ~15 por año, piden saber leer), aprendices de los talleres (0,05 % de los empleos industriales por mes: unos 6 por año cada 1.000 empleos) y alumnos de los maestros de ultramar (~4 por año) | Requisito de casi todas las tecnologías; cada industria necesita algunos para marchar a pleno |
| Pilotos | Egresados de la escuela de náutica (15 alumnos, 2 años; muchachos que leen y escriben) | Hasta +15 % en el precio de lo que se embarca |
| Técnicos, ingenieros | Épocas siguientes (UTU, facultad) | Tecnologías de las épocas 2 a 6 |

Formarse cambia a la persona: pasa a ser artesana (o marinero, el piloto), con «aprendió» en su
historia de vida, y cobra el salario del oficio: la educación se vuelve ingreso del hogar. Si no
hay jóvenes que lean, las plazas quedan vacías (el panel lo avisa). Cualquier joven de la
ciudad puede formarse y cualquier persona puede trabajar en estos establecimientos (el juego no
representa la esclavitud, ver [poblacion.md](poblacion.md), sección 5).

## 2. Tecnologías

Cada tecnología tiene requisitos (saberes, edificios, otras tecnologías, muelle, época), un
**costo en pesos que se paga en cuotas mensuales** (más ladrillos por mes de obra) y lo que da:
un edificio, un producto para exportar o un multiplicador de productividad. Si un mes no alcanza
la plata o los ladrillos, la obra **se para** (como la iglesia) y sigue cuando vuelven.

### Época 1: colonial (jugable)

| Tecnología | Requisitos | Costo | Meses | Da | Historia |
|---|---|---|---|---|---|
| Escuela de dibujo y oficios | 80 que leen, 1 escuela | 1.500 | 6 | Edificio: escuela de oficios | Licencia de diseño [3][4] |
| Curtido con corteza y noques | 20 artesanos, 4 oficiales | 1.200 (+10 lad./mes) | 6 | Suela en los talleres; talleres +15 % | Licencia |
| Horno de ladrillos de bóveda | 1 horno, 6 oficiales | 1.000 (+20 lad./mes) | 4 | +40 % de ladrillos | Licencia |
| Salazón de carnes | 1 estancia, muelle, 8 oficiales | 3.000 (+15 lad./mes) | 10 | Edificio: saladero; producto: tasajo | 1781 (Colla) y 1787 (Montevideo) [1] |
| Grasería | Saladero, 15 oficiales | 1.500 | 6 | Saladero +15 %, doble sebo | Licencia |
| Molino de viento | 1 chacra, 10 oficiales | 2.000 | 8 | Edificio: molino | Licencia |
| Carpintería de ribera | Muelle, 12 oficiales | 3.500 | 10 | Edificio: astillero; producto: carenas | Apostadero naval 1776 [5]; astillero, licencia |
| Escuela de náutica | 200 que leen, carpintería de ribera | 2.500 | 8 | Edificio: escuela de náutica; pilotos | Buenos Aires, 1799 [3]; Montevideo, licencia |
| Imprenta | 300 que leen, 6 oficiales | 3.000 | 10 | Edificio: imprenta; impresos | Primer periódico, 1807 [2] |

### Épocas siguientes (datos; todavía no se juegan)

| Época | Tecnologías | Fechas |
|---|---|---|
| 2. Ciudad Nueva | Lana merina y lavadero, molino a vapor | Sin fecha verificada |
| 3. Inmigración | Ferrocarril, Escuela Nacional de Artes y Oficios, frigorífico, usina eléctrica, facultad de ingeniería | Artes y Oficios 1879 (o 1878) [4]; La Frigorífica Uruguaya, 1902 [6]; ferrocarril, «a verificar» |
| 4. Art Déco | UTU, textil, refinería | UTU: decreto-ley 10.225, 1942 [4] |
| 5. Modernismo | Metalurgia y química | — |
| 6. Contemporánea | Software y servicios, celulosa, energía eólica | — |

## 3. Industrias

Cada industria tiene un **perfil** (`datos::INDUSTRIAS`): empleos, saber que necesita, capital,
salario, insumo, productos, parte exportada, si compite con lo importado (mercado local), el
**recurso del suelo** que pide el sitio y la **molestia** para los vecinos (para el módulo de
recursos del suelo, que se engancha en `desarrollo::ciudad::rinde_del_sitio`, hoy 1,0).

| Industria | Empleos + saber | Capital | Produce por mes | Exporta | Por obrero* |
|---|---|---|---|---|---|
| Talleres (zona industrial) | 12 | 400 | 18 ladrillos, 12 sebo, 9,6 cueros | 0 % | ~3,9 pesos |
| Saladero | 20 + 4 oficiales | 6.000 | 300 tasajo, 50 cueros, 40 sebo (compra 400 de carne) | 95 % del tasajo | ~18 pesos |
| Molino de viento | 4 + 1 | 2.500 | 500 raciones de harina | 0 % | ~25 pesos |
| Carpintería de ribera | 12 + 8 | 6.000 | 6 carenas | 70 % | ~18 pesos |
| Imprenta | 4 + 2 | 3.000 | 80 pliegos | 0 % | ~20 pesos |
| Frigorífico (época 3), textil (4), software y celulosa (6) | — | — | perfiles para comparar | — | — |

\* Valor de lo producido a precios de referencia, dividido por la gente que emplea. Todos los
montos son **estimaciones de diseño**: lo que importa es el contraste. Las industrias de capital
producen mucho por obrero y venden afuera; los talleres dan más empleo por peso invertido y
venden en la ciudad.

**Cada mes, cada establecimiento** (`Desarrollo::mes`):
- marcha = (saber: 40 % sin oficiales, 100 % con todos) × (peones contratados / empleos) × rinde
  del sitio × tecnologías;
- compra su insumo del mercado (el saladero compra ganado: la carne de la ciudad sube de precio);
- produce: los recursos van a la economía (cueros, sebo, harina, ladrillos); los productos propios
  se exportan por el muelle al precio del mundo (sin muelle, por lanchas al 70 %; con bloqueo, se
  malvenden en la ciudad);
- paga derechos de aduana (salvo la exención), salarios, insumos, mantenimiento (0,4 % del capital)
  y la sal del saladero (importada);
- la **ganancia** va al dueño: al presupuesto si es del cabildo, al ahorro y los ingresos del
  hogar si es de una familia (desigualdad), o se va de la ciudad si es de un comerciante de
  afuera («remesas»).

Los **peones** son personas de la población: de 15 a 60 años, sin trabajo y sin
oficio o peones. El establecimiento completa su jornal (el modelo de población ya les paga el
75 % de su oficio por cuenta propia). Si consiguen trabajo en un solar, dejan el establecimiento.

## 4. Inversión e incentivos

**Pública**: el cabildo paga las tecnologías (cuotas), las escuelas (como cualquier servicio) y
puede levantar una industria desde Producción (la ganancia es del cabildo).

**Privada**: cada mes, para cada industria habilitada, un inversor mira (`Desarrollo::evaluar`,
visible en el panel con «•» si se cumple y «×» si no):
- muelle para embarcar (las que exportan);
- oficiales libres suficientes;
- ganado para comprar (el saladero);
- brazos sin trabajo (al menos la mitad de los empleos);
- seguridad (delito medio < 45 %);
- **rinde esperado ≥ 10 % anual** (con los precios de hoy, a plena marcha; un censo rendía
  alrededor del 5 %: estimación);
- capital: un hogar acomodado o principal con el ahorro necesario (la mitad con crédito de
  fomento), o, si el rinde pasa del 18 % y hay muelle y los comerciantes no están enojados, un
  comerciante de afuera;
- sin privilegio exclusivo ya dado; con olor (molestia > 0,5), que los vecinos no estén furiosos.

Si todo se cumple, con probabilidad 0,4 por mes se invierte en la de mejor rinde: el inversor
busca un sitio junto a una calle a la distancia que pide la industria y levanta el edificio con
su plata (la gaceta lo cuenta).

| Incentivo | Efecto | Costo | Opinión (V/C/H) |
|---|---|---|---|
| Exención a industrias nuevas | 10 años sin derechos de aduana por lo que exportan | Renuncia fiscal (se ve en el panel) | 0 / −4 / +6 |
| Crédito de fomento | El cabildo presta la mitad del capital al 4 % anual (cuota 1,5 % por mes); si el negocio pierde, puede no volver | Préstamos + 10/mes | +2 / +3 / +3 |
| Aranceles protectores | Manufacturas importadas con recargo: molino, talleres e imprenta venden 25 % más caro; cobra 10 % de lo importado | Los vecinos pagan más | −4 / −8 / +2 |
| Privilegios exclusivos | Un establecimiento por industria; gana 15 % más y paga canon de 25/mes | Sin competencia | −2 / −6 / −8 |
| Maestros de ultramar | ~4 oficiales por año | 60/mes | 0 / +2 / 0 |
| Libre comercio (ordenanza ya existente) | +30 % de barcos, aduana −30 % | Menos aduana | −5 / +15 / +10 |

Los aranceles y el libre comercio no van juntos. El Reglamento de comercio libre de 1778 abrió
el comercio de los puertos del Río de la Plata con España [7].

## 5. Comercio exterior

- **Productos**: cueros, sebo, carne y trigo (los embarca la economía) y tasajo, suela, carenas e
  impresos (la industria). Precio de referencia × precio del mundo (paseo al azar ±3 % que vuelve
  a 1,0, entre 0,7 y 1,3) × choques × pilotos.
- **Choques de precios** (3 % por mes, ficticios): sobra tasajo en La Habana (×0,55), guerra en
  Europa (cueros ×1,5), cueros de sobra en Cádiz (×0,55), escasez de sebo (×1,6), suela inglesa
  barata (×0,6), escuadras de paso (carenas ×1,6). El precio de los cueros del mundo también mueve
  lo que embarca la economía (`MonthInput::world_hides`).
- **Importaciones**: lo que trae la economía, la sal del saladero y manufacturas (0,25 pesos por
  habitante y por mes, menos lo que ahora se hace en la ciudad, hasta 60 %).
- **Balanza** = exportado − importado (gráfico de 24 meses).
- **Diversificación**: índice de Herfindahl de las exportaciones de 12 meses (suma de los
  cuadrados de las partes). Cuando las exportaciones caen más de 15 % respecto del promedio, se
  enojan hacendados y comerciantes en proporción a la caída: la ciudad que vende un solo producto
  sufre el choque entero.

## 6. Edificios (vóxeles de 0,5 m, medidas estimadas)

| Edificio | Planta | Qué se ve |
|---|---|---|
| Escuela de dibujo y oficios | 20 × 18 m | Casa de azotea ocre con ventanas grandes, taller-escuela de ladrillo con fragua y banco |
| Escuela de náutica | 12 × 16 m | Casa de altos con balcones, mirador en la azotea y mástil de prácticas con verga |
| Saladero | 60 × 45 m | Galpones de ladrillo y teja, grasería con chimenea, tendederos con cueros, corral |
| Molino de viento | 10 × 10 m | Torre encalada, capucha de teja, aspas de lona |
| Carpintería de ribera | 35 × 25 m | Grada con una lancha a medio armar (cuadernas y tablazón), galpón, tablones, caldera de brea |
| Imprenta | 10 × 14 m | Tienda con vidriera y cartel, prensa de madera, pliegos secándose |

Sitios: saladero a más de 600 m de la plaza y con agua (arroyo o costa a 300 m); molino a más de
250 m; carpintería de ribera sobre la costa (a 60 m del agua) y fuera del centro; imprenta a
menos de 500 m (una por ciudad); escuela de náutica, una por ciudad.

## 7. Interfaz

- Categoría **Desarrollo** en la barra de abajo: tarjetas que abren las pestañas del panel y las
  dos escuelas. **Producción** suma saladero, molino, carpintería de ribera e imprenta;
  **Servicios → Educación**, las dos escuelas. Desactivadas con «Falta tecnología».
- Panel **Desarrollo** (pestañas): *Tecnologías* (por época; estado, barra de avance, requisitos
  con «•» y «×», lo que da, costo, nota histórica, botón Invertir), *Saberes* (barras, de dónde salen,
  plazas vacías, gráficos), *Industria* (tabla de perfiles con producción por obrero, los
  establecimientos y lo que miran los inversores), *Fomento* (incentivos como tarjetas, libre
  comercio, renuncia fiscal, préstamos, inversión privada por mes) y *Comercio* (por producto:
  exportado, parte y precio del mundo; balanza; concentración; choque en curso; gráficos apilados).
- Ficha de cada edificio: dueño, peones, marcha, ventas, ganancia, lo que le falta, deuda y años de
  exención; en las escuelas, alumnos, egresados por año y plazas vacías.
- Gaceta: tecnología lograda, obra parada, inversor que abre una industria, choques de precios,
  crédito impago.

## 8. Guardar y cargar

`CityData::desarrollo` (opcional) guarda todo el estado (`Desarrollo` es serializable: tecnologías,
proyectos, incentivos, establecimientos con sus peones, formados, acumulados, precios, choque,
historia de 24 meses, evaluaciones y su azar propio). Las partidas viejas cargan sin desarrollo.
Pruebas: `guardar_y_cargar_el_desarrollo`; el recorrido `--demo-ciudad` compara también este
estado («IGUAL»).

## 9. Pruebas

`engine/src/city/desarrollo/tests.rs`: la tecnología pide saberes e inversión (y se para sin
plata); la industria de capital produce más por obrero y exporta; los incentivos atraen inversión
y bajan la aduana (renuncia fiscal); un choque de precios golpea más a la ciudad sin
diversificar; la escuela de oficios forma oficiales en la población; guardar y cargar.

## Fuentes

Solo se dan como hechos los verificados; el resto está marcado como licencia de diseño o
estimación.

1. INAC (Instituto Nacional de Carnes), «Beginnings of domestic supply»: primer saladero en 1781
   sobre el arroyo Colla, de Francisco de Medina; en 1787 Antonio Maciel instala otro junto al
   Miguelete, en Montevideo; el tasajo se exporta a Cuba. <https://www.inac.uy/innovaportal/v/5958/17/innova.front/beginnings-of-domestic-supply>
   (Otras fuentes dan 1786 para Medina; se usa la del INAC.)
2. Wikipedia, «The Southern Star (Montevideo)»: primer periódico de la ciudad, 1807, durante la
   ocupación británica. <https://en.wikipedia.org/wiki/The_Southern_Star_(Montevideo)>
3. Argentina.gob.ar, Armada, «Marina mercante»: el Consulado de Buenos Aires (1794), con su
   secretario Manuel Belgrano, abre la Escuela de Náutica en noviembre de 1799.
   <https://www.argentina.gob.ar/armada/intereses-maritimos/marina-mercante>
4. Wikipedia, «University of Labor of Uruguay» y estudios citados allí: Escuela de Artes y Oficios
   de 1879 (algunas fuentes, 1878); UTU por el decreto-ley 10.225 del 9 de septiembre de 1942.
   <https://en.wikipedia.org/wiki/University_of_Labor_of_Uruguay>
5. M. Á. De Marco, *La historia contemplada desde el río* (Academia Nacional de la Historia): el
   Apostadero naval de Montevideo, 1776. <https://repositorio.anh.org.ar/bitstream/anh/511/1/La%20historia%20contemplada%20desde%20el%20r%C3%ADo%20-%20Miguel%20%C3%81ngel%20De%20Marco.pdf>
6. INAC, «The era of refrigerated meat»: La Frigorífica Uruguaya, Cerro de Montevideo, 1902.
   <https://www.inac.uy/innovaportal/v/5959/17/innova.front/the-era-of-refrigerated-meat>
7. Reglamento y aranceles reales para el comercio libre de España a Indias, 12 de octubre de
   1778 (texto en archive.org). <https://archive.org/stream/reglamentoyaranc00spai/reglamentoyaranc00spai_djvu.txt>

## Pendiente

- Mapas de recursos del suelo (pasturas, arcilla, costa, viento) en `rinde_del_sitio`.
- Agentes visibles: arreos de ganado al saladero, carretas de tasajo al muelle, aprendices.
- Stock propio de productos que espera barco (hoy se exporta cada mes, salvo bloqueo).
- Tecnologías y edificios de las épocas 2 a 6 jugables; técnicos e ingenieros.
- Contaminación del arroyo por el saladero y la curtiembre (con el módulo de recursos y
  externalidades).
