# Materiales de construcción

> Documento de sistema. Complementa al [GDD](../GDD.md), a [zonas](zonas.md),
> [servicios](servicios.md), [recursos del suelo](recursos-del-suelo.md) y
> [políticas y recursos](politicas-y-recursos.md). Estado: **implementado en el motor**
> (`engine/src/city/materiales/`), época colonial.

## Principio

**Todo edificio pide materiales, y se levanta con los que hay.** Si falta el material preferido
se usa el mejor que haya a mano; en el peor caso, **barro y paja**, que están en todos lados.
Construir nunca se prohíbe: lo que cambia es *cómo* queda el edificio. Una casa levantada
cuando no hay cal queda de ladrillo sin revoque; si solo hay madera, es una casa de tablas; si
no hay nada, un rancho de barro y paja. El edificio **guarda con qué se hizo** y eso decide
cuánto arde, cuánto dura, cuánta humedad tiene adentro, cuánto vale, cuántos pisos aguanta y
qué pide para arreglarse. Y se ve: los vóxeles muestran sus materiales reales.

## Los materiales

Las cantidades van en *unidades de obra* (las de los ladrillos de la economía). Precios y
cantidades iniciales son **estimaciones de diseño**.

| Material | De dónde sale en la ciudad colonial | Puerto | Al fundar |
|---|---|---|---|
| Barro | El suelo del mismo solar: gratis y siempre | — | sin límite |
| Paja | Pajonales y bañados del ejido | — | 600 |
| Adobe | Barro con paja moldeado y secado al sol; no seca con lluvia ni frío | — | 60 |
| Cuero | Las estancias (los mismos cueros que se exportan; existencias de la economía) | — | — |
| Madera | Casi no hay monte de obra cerca: palos y leña del monte criollo; los tirantes llegan de afuera | Paraguay y Corrientes por Buenos Aires, Brasil | 120 |
| Ladrillo | Hornos de ladrillos (existencias de la economía) | Buenos Aires | 200 |
| Teja | Los mismos hornos (15 por mes cada uno) y los talleres | Buenos Aires | 0 |
| Piedra | Canteras sobre el basamento (existencias de la economía) | Buenos Aires | 150 |
| Cal | No hay caliza en el mapa: llega de las caleras de la Banda Oriental (Colonia) | Colonia, Buenos Aires | 30 |
| Arena | Playas, médanos y orillas de los arroyos | — | 80 |
| Hierro | Rejas, clavos y herrajes: de España | España | 8 |
| Vidrio | De Europa; raro y caro | Europa | 0 |

**Lo que da el lugar por mes** (`EntornoMateriales`, estimación): paja 400 + 0,3 por habitante;
adobe (50 + 0,08 por habitante) × clima (secado) × (0,5 + arcilla media de los solares); arena
30 + 120 × arena media; madera 6 + 0,004 por habitante; tejas 15 por horno (según su rinde) y
0,4 por empleo de taller, por el clima. La cal, el hierro y el vidrio solo llegan por el puerto.

## Partes del edificio y sus cualidades

Cada edificio tiene **paredes**, **techo**, si está **revocado y blanqueado con cal** y si
lleva **rejas de hierro**. Todas las cifras son **estimaciones de diseño**.

| Paredes | Receta por módulo | Fuego | Contagio | Dura | Humedad | Prestigio | Pisos |
|---|---|---|---|---|---|---|---|
| Piedra | 30 piedra + 4 arena | ×0,8 | 0 | 150 años | 0,20 | 1,0 | 2 con cal |
| Ladrillo | 30 ladrillo + 3 arena | ×0,9 | 0 | 80 años | 0,15 | 0,7 | 2 con cal |
| Adobe | 30 adobe | ×1,0 | 0 | 30 años (×1,5 revocado) | 0,40 | 0,3 | 1 |
| Tablas | 14 madera | ×1,8 | +0,35 | 20 años | 0,25 | 0,25 | 1 |
| Barro y paja | 30 barro + 4 paja | ×1,1 | +0,05 | 10 años (×1,5 revocado) | 0,60 | 0 | 1 |

| Techo | Receta por módulo | Igniciones por mes | Contagio | Dura | Humedad | Prestigio |
|---|---|---|---|---|---|---|
| Teja | 12 teja + 3 madera (tirantes) | 0,03 % | 0,25 | 50 años | 0 | 0,6 |
| Azotea | 12 ladrillo + 3 cal + 4 madera | 0,01 % | 0,08 | 40 años | 0,05 | 0,8 |
| Tablas | 8 madera | 0,10 % | 0,6 | 15 años | 0,10 | 0,2 |
| Paja | 10 paja | 0,15 % | 1,0 | 6 años | 0,15 | 0 |
| Cuero | 6 cuero | 0,07 % | 0,4 | 4 años | 0,20 | 0,05 |

- **Revoque**: 3 de cal por módulo. Protege el adobe y el barro de la lluvia y quita un cuarto
  de la humedad. Es también el mortero: **sin cal no hay casas de altos**.
- **Rejas**: 1 de hierro por módulo (casas de una planta y de altos). Sin hierro, ventanas sin
  reja.
- **Dura**: el edificio dura lo que la parte más débil (el techo cuenta con dos veces y media su
  vida, porque se cambia antes que las paredes).
- **Tamaño** (módulos): rancho o pulpería 1; casa de una planta 2; taller grande 2,5; casa de
  altos 4; densidad alta ×1,3. Un servicio: (ladrillos + piedra de su ficha) / 42.

## Recetas con alternativas

Cada edificio pide una lista de preferencias para paredes y techo; se toma la **primera cuya
receta entera esté en el depósito** (lo reservado para las paredes no se cuenta dos veces para
el techo).

| Edificio | Paredes, en orden | Techo, en orden |
|---|---|---|
| Rancho (residencial 1) | adobe > tablas > barro y paja | paja > cuero (con la ordenanza «sin paja» cerca de la plaza: teja > tablas > cuero > paja) |
| Pulpería y taller (nivel 1) | ladrillo > adobe > tablas > barro | el de su dibujo > teja > azotea > tablas > paja > cuero |
| Casa de una planta, tienda, curtiembre (nivel 2) | ladrillo > piedra > adobe > tablas > barro | casa de azotea: azotea > teja > …; casa de teja: teja > azotea > … |
| Casa de altos, almacén de altos (nivel 3) | piedra > ladrillo (solo con cal) | azotea > teja > tablas > paja > cuero |
| Barraca (industrial 3) | ladrillo > piedra > adobe > tablas > barro | teja > azotea > … |
| Servicios | ladrillo > piedra > adobe > tablas > barro (piedra primero si su ficha pide más piedra) | teja > azotea > tablas > paja > cuero |

Si nada alcanza, **paredes de barro y techo de paja**: siempre se puede. Los **altos** necesitan
paredes de ladrillo o piedra asentadas con cal: si no las hay, las casas no pasan de una planta
(la ficha lo dice) y, si dos casas compiten por la última cal, la segunda queda en una planta.

La iglesia matriz, la muralla y el muelle son **grandes obras**: siguen esperando sus ladrillos
y su piedra mes a mes, como antes (no se sustituye la piedra de una muralla).

## Efectos de los materiales

| Efecto | Cómo |
|---|---|
| **Incendios** | Igniciones = 0,15 % × fuego del techo × fuego de las paredes; contagio = el del techo + el de las paredes. El techo de la ficha y la capa de fuego salen de los materiales |
| **Inundaciones** | El agua se lleva las paredes de barro y de adobe; las de ladrillo, piedra y tablas aguantan |
| **Desgaste** | El estado baja 1/(vida × 12) por mes. Bajo 40 %, el dueño lo manda arreglar con un cuarto de su receta; si no hay, sigue bajando. En 0 **se viene abajo** y se levanta de nuevo, en el mismo nivel, con lo que haya (puede salir mejor o peor). Las casas de altos sin materiales para otros altos quedan apuntaladas |
| **Salud** | La humedad suma un cuarto a la insalubridad del solar, que pesa en la mortalidad y la felicidad de quienes viven ahí |
| **Valor** | La población ve el valor del solar × (0,85 + 0,3 × prestigio): alquileres y ganas de vivir ahí |
| **Pisos** | Los altos piden ladrillo o piedra y cal |
| **Mantenimiento** | Los arreglos piden materiales (y mueven la demanda y los precios) |

## Economía

- **Construir gasta**: cada obra saca del depósito lo que usa (ladrillo, piedra y cuero de la
  economía; el resto, del depósito de materiales). Las casas ya no gastan ladrillos por mes.
- **Demanda**: lo que pidieron todas las obras (se haya conseguido o no) se suaviza mes a mes. La
  de ladrillo, piedra y cuero llega a la economía como `pedidos`: sube el precio y lo que se
  encarga a los barcos, sin descontarse dos veces.
- **Puerto**: con muelle y barcos (sin bloqueo), cada barco trae hasta 90 unidades de cada
  material importable que falte: la demanda de un mes y medio más una reserva (madera 40, cal 20,
  hierro 4). Sin puerto, lo que no da el lugar no llega.
- **Precio**: según los meses de reserva, como los demás recursos (0,6 a 3); lo que solo llega
  por el puerto no baja de 1.
- **Gaceta**: un aviso al empezar cada falta («Falta cal: las casas nuevas se levantan sin
  revoque y no hay azoteas», «Faltan ladrillos: las casas nuevas se levantan de adobe o de
  tablas», «Falta hierro: las ventanas nuevas no llevan rejas»…) y otro cuando llega por el
  puerto.

## En los vóxeles

El catálogo dibuja cada edificio con sus materiales de siempre (el *nominal*: cal, ladrillo,
teja o azotea; el rancho de adobe y paja). Si la obra real es otra, se arma una **`Variante`**
(`structures::Variante`: paredes, revoque, ladrillo visto, techo, rejas y la semilla del solar,
elegida de forma determinista) y `structures::aplicar_variante` cambia los vóxeles por material.
La `Variante` está pensada para sumarle después parámetros de estilo (proporciones, fachadas,
colores de cal) sin tocar los materiales:

| Obra real | Paredes | Techo |
|---|---|---|
| Piedra con cal / sin cal | sillería (`STONE`) / mampostería de piedra suelta (`MAMPOSTERIA`, nuevo) | — |
| Ladrillo encalado / sin revoque | revoque de cal / ladrillo visto (`BRICK`) | — |
| Adobe encalado / sin revoque | cal / adobe | — |
| Tablas | tablas (`TABLAS`, `TABLAS_GRIS`, nuevos) | tablas |
| Barro y paja | barro con paja (`QUINCHA`, nuevo) | — |
| Teja · azotea · paja · cuero | — | teja · baldosa de azotea · paja · cueros |
| Sin hierro | las rejas pasan a ventana simple | — |

La casa de una planta sigue a su techo: con azotea es la casa de azotea (plana, con pretil); con
cualquier otro techo, la casa a dos aguas. La paleta pasó de 64 a 70 materiales (80 lugares en
el render); el shader dibuja las tablas (juntas y veta), las pajitas del barro y las juntas de
la mampostería.

## Interfaz

- **Ficha de un solar**: «Paredes de adobe, techo de teja · Arde poco, húmeda; dura ~30 años sin
  arreglos · estado 100 %» y, si se sustituyó, «Se levantó con lo que había: faltaba ladrillo».
- **Ficha de un servicio**: lo mismo, debajo de sus datos.
- **Tarjetas de servicios y pista al colocar**: la receta y, si falta algo, con qué se hará
  («Falta ladrillo: se hará con paredes de adobe…»). Ya no aparecen desactivadas por falta de
  ladrillos o piedra.
- **Ficha «Materiales» en la barra** (entre la piedra y el pan): «al día» o «falta cal»; el globo
  lista las existencias. Al hacer clic, el **panel de materiales**: existencias, precio, lo que
  dio el lugar, lo que trajo el puerto, lo pedido y lo que faltó (con cuántas obras se
  sustituyó), qué se está sustituyendo, con qué está hecha la ciudad (paredes y techos en %,
  casas húmedas), lo que da el lugar por mes y la madera y la cal de los últimos meses.

## Guardar y cargar

Cada solar guarda su obra (`obra`, opcional) y la ciudad su depósito (`materiales`, opcional).
Las partidas viejas cargan cada casa con los materiales de su dibujo (el rancho de adobe y paja;
las casas, de ladrillo encalado con teja o azotea), así se ven igual que antes. Todo es
determinista: el recorrido de la demostración guarda, carga y compara (IGUAL).

## Frontera con el catálogo de minerales

El catálogo detallado de materiales del suelo (granito, arena, arcilla roja, caliza…) se arma
aparte en `city/recursos/`. Este sistema solo usa:

- `Disponible::hay(material)`: existencias a mano (hoy: economía + depósito propio);
- `FuenteLocal::disponibilidad_local(material)`: rinde del lugar por mes (hoy:
  `EntornoMateriales`, con las capas de arcilla y arena de los solares, hornos y canteras);
- `Material::clave()` / `Material::de_clave()`: «piedra», «ladrillo», «teja», «cal», «arena»,
  «madera», «adobe», «paja» (más «barro», «cuero», «hierro», «vidrio»), la misma clave
  `construccion` del catálogo.

Para conectarlo, `EntornoMateriales::disponibilidad_local` puede sumar lo que dé
`Recursos::materiales_cerca(p, radio)` agrupado por esa clave (por ejemplo, una calera sobre
caliza daría cal local).

## Base histórica

Verificado (consultado el 17/11/2026; de algunas fuentes solo se pudo leer el fragmento que
muestra el buscador, porque el servidor negó la descarga del PDF, y así se marca):

- **Ladrillo con mortero de barro y de cal**. Un estudio sobre los ladrillos de Montevideo en el
  siglo XIX, con permisos de obra del Archivo de la Ciudad, afirma que hacia fines del siglo
  XVIII era habitual usar ladrillos macizos en muros, pisos y cubiertas, con morteros
  inicialmente de barro, y que los ladrillos tomados con arena y cal son una constante desde la
  fundación. <https://core.ac.uk/download/153564808.pdf> (fragmento del buscador). → En el juego,
  el ladrillo sin cal existe (sin revoque) pero no sostiene altos.
- **Madera escasa y traída de afuera**. La tesina «La construcción en madera en Uruguay»
  (FADU-UdelaR) señala la escasez de madera local y que los tirantes de techo se traían de los
  montes del Santa Lucía y de Corrientes; describe azoteas sobre tirantes transversales con
  ladrillos asentados en mortero.
  <https://www.fadu.edu.uy/tesinas/files/2012/09/Tesina-La-Construcción-en-Madera-en-Uruguay.pdf>
  (fragmento del buscador). → Madera casi toda importada; la azotea pide madera, ladrillo y cal.
- **Cal de la Banda Oriental**. La Compañía de Jesús tuvo a mediados del siglo XVIII la estancia
  «Calera de las Huérfanas», con hornos de cal, en el actual departamento de Colonia, y hay
  estudios de una calera de principios del siglo XVIII en Colonia del Sacramento
  (<https://ojs.fhce.edu.uy/index.php/aarq/article/download/2106/2717/7985>, fragmento del
  buscador). No se encontró una fuente que documente el envío de cal a Montevideo: que llegue
  «de Colonia» es una **suposición** marcada así en el juego.
- **Cuántas casas de azotea y de altos**. Un relato histórico estima, hacia fines del siglo XVIII
  o comienzos del XIX, unas 300 casas de azotea de una planta y unas 60 de dos plantas en
  Montevideo, y menciona ranchos con paredes de ladrillo y cumbreras traídas del Santa Lucía
  (<https://biblioteca.org.ar/libros/157501.pdf>, fragmento del buscador).
- **Evolución urbana**. Luque Azcona, E. J., «Evolución urbana de Montevideo en el siglo XVIII»
  (Simposio de Medina, 1999), y su trabajo sobre la construcción de la ciudad colonial
  (1723-1810): referencia para seguir.
  <https://www.americanistas.es/wp-content/uploads/2024/07/s02-006-Luque-Azcona-Emilio-Evolucion-urbana-de-Montevideo-en-el-siglo-XVIII-un-mundo-de-frontera-luchas-y-contrabando-Simposio-de-Medina-1999.pdf>
- **Ranchos y casas** (del documento de la [época colonial](../epocas/01-colonial.md)): rancho
  de barro (adobe o terrón) y paja; casa de azotea de ladrillo encalado; casa de altos.

**Estimaciones** (diseño, sin fuente): todas las recetas, precios, cantidades iniciales, rindes
del lugar, vidas útiles, humedades, prestigios y la capacidad de los barcos; el hierro «de
España (Vizcaya)» y el vidrio «de Europa» son supuestos razonables sin fuente consultada; el
techo de cuero de los ranchos se modela como alternativa cuando falta paja, sin fuente
consultada sobre su frecuencia.

## Lo que falta

- Calera (con caliza del catálogo de minerales) y horno de tejas propio; el pozo de arena y el
  corte de paja como lugares en el mapa.
- Rehabilitar en el lugar (revocar una casa de ladrillo visto cuando llega la cal, cambiar la
  paja por teja) sin esperar a que se venga abajo.
- Materiales de las épocas siguientes (hierro fundido, hormigón, vidrio plano, chapa).
- Que el precio de los materiales encarezca las mejoras que pagan los hogares.
