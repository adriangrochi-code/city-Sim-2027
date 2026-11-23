# Arquitectura de la época 2: la Ciudad Nueva (1830–1880)

> Guía de fuentes y decisiones del kit de vóxeles de la época 2 (casas neoclásicas e
> italianizantes, rehabilitación de las casas coloniales, ensanche, gas, agua corriente,
> colectores y teatro). Complementa a [arquitectura-colonial.md](arquitectura-colonial.md) y a
> [../epocas/02-ciudad-nueva.md](../epocas/02-ciudad-nueva.md).
>
> Marcas: **[F]** dato documentado en una fuente citada abajo; **[E]** estimación de diseño
> del juego (proporciones, frecuencias, colores elegidos para que se lea en vóxeles).

---

## 1. Fechas y hechos verificados

| Hecho | Fecha | Fuente | Uso en el juego |
|---|---|---|---|
| Decreto de demolición de la muralla y trazado de la "Ciudad Nueva" de José María Reyes sobre el ejido, desde la Ciudadela hasta la actual calle Yaguarón, en damero y sin nueva muralla (136 o 160 manzanas, según la fuente) | 1829 (el plano, 1829–1836) | [F] Wikipedia «Centro, Montevideo»; tesina FADU (Murieda, espacios públicos); fascículo FHCE 300 años | Decisión de la muralla al pasar de época; herramienta de **ensanche**; `ANIO_CIUDAD_NUEVA = 1830` |
| Primera Constitución | 1830 | [F] conocimiento general | Fin del "nuevo orden" en el modo fiel (1828–1830) |
| Alumbrado a gas: usina de los hermanos Ísola (Mario, boticario; Demetrio, ingeniero) en la costa norte de la bahía, entre Arapey y Río Negro; primeros faroles en la Plaza Constitución | junio–julio de 1853 (inauguración el 18 o el 22 de julio: las fuentes difieren) | [F] La Izquierda Diario, «Mario Ísola, pionero de la Compañía del Gas» y «El día del gasista»; IM (Parque Mauá) | **Usina de gas**, faroles de gas, carbón importado |
| Red de colectores de Juan José de Arteaga: propuesta 1852, contrato 31-10-1854, Ley de Saneamiento 25-6-1856, obras desde 1856–57; la primera de América Latina (declarada Monumento Histórico Nacional en 2024) | 1854–1857 | [F] IM, «Sobre los 100 años del saneamiento de Montevideo 1913–2013»; BID, blog «Agua»; IMPO, resolución 396/2024 | **Colector y desagüe** |
| Agua corriente del río Santa Lucía (Compañía de Aguas Corrientes: Lezica, Lanús y Fynn, concesión 1867); primera agua en una fuente de la Plaza Constitución | 18 de julio de 1871 | [F] Ministerio de Ambiente, «150 años del abastecimiento de agua corriente»; OSE; Museo del Agua | **Usina de agua corriente** (tanque elevado y fuente pública) |
| Teatro Solís: proyecto de Carlo Zucchi (1840–41), adaptado por Francisco Javier de Garmendia; inaugurado con *Ernani* | 25 de agosto de 1856 (alas laterales de Víctor Rabu, 1874) | [F] Wikipedia «Solís Theatre»; Intendencia de Montevideo (guiones del teatro); Playbill | **Teatro** (pórtico de columnas, sala, caja escénica, alas) |
| Torres y miradores: aparecen en el período luso-brasileño (1817–1829) y son "tema reiterado de la arquitectura montevideana durante todo el siglo XIX" | 1817–1925 | [F] MEC, Academia Nacional de Letras (casa sede) | Miradores más frecuentes en la época 2 |
| Casa de patio ("casa standard" o "casa chorizo"): tipología dominante del segundo ensanche; portal, zaguán y patios en hilera | fines del XIX | [F] Wikipedia «Casa chorizo»; FADU (ensanche de Montevideo); GCBA (tipologías) | **Casa neoclásica de patio** |
| Inmigración de 1860: un tercio de los europeos españoles, un tercio italianos, 27 % franceses | 1860 | [F] Pollero y González Demuro, *Población y sociedad de Montevideo* (2024) | Tablas de llegadas de la época 2 y rasgos de las familias italianas y francesas |
| Columna de la Paz (plaza Cagancha) | 1867 | [F] conocimiento general | Columna en el centro de las plazas del ensanche |

Las fechas discrepantes se dejan anotadas: no hay en estas fuentes un documento primario que
las resuelva (para el juego, 1853 alcanza).

---

## 2. Tipologías de la época 2 en el motor

Las casas se eligen como en la colonia (`structures::estilo::estilo`), con las reglas de la
época tomadas a partir de 1830, y encima una **fachada neoclásica e italianizante**
(`fachada_neo`). La planta sigue la casa de patio de la colonia, que es la base de la casa
standard [F].

| Nivel | Tipo | Qué se ve |
|---|---|---|
| 2 | **Casa neoclásica de patio** | Una planta entre medianeras, tres vanos (puerta y dos ventanas; las ricas, cinco), puerta alta de dos hojas con montante de hierro fundido, zaguán con piso de mármol en damero y cancel de hierro y vidrio, ventanas "de balcón" casi hasta el piso con persianas (algunas abiertas) y reja de hierro fundido plana o volada, guardapolvos, pilastras, cornisa con friso y pretil con balaustres (con macetones o pináculos en las más ornamentadas) [E] |
| 3 | **Casa de altos neoclásica** | Dos plantas, balcón corrido de hierro fundido sobre losa de mármol y ménsulas, puertas-ventana con persianas y montante, pilastras en las dos plantas, balaustrada; mirador en ~18 % de las de clase alta [E] |
| 2 (afueras) | **Quinta italianizante** | Galería de columnas claras con capitel y piso de mármol en damero, azotea con balaustrada, palmera canaria en el jardín del frente [E] |
| comercio | **Almacén de esquina** y **casa con almacén al frente** | Ochava en el 60 % de las esquinas, letrero, puertas de tienda [E] |

Materiales: la fachada nueva necesita revoque (sin cal no hay fachada neoclásica: el ladrillo,
la piedra o el adobe quedan a la vista) y hierro para las rejas (sin hierro, postigos). El
mármol de escalones y zaguanes es importado [E].

### Colores [E]
Los de la paleta de la época (`docs/epocas/02-ciudad-nueva.md` §11), con pesos: ocre claro
(#D9C29A) 30 %, gris piedra "símil piedra" (#A8A39A) 21 %, hueso 18 %, rosado
("pompeyano") 12 %, blanco 10 %, almagre 5 %, celeste 4 %; molduras en blanco piedra
(#EEE9DF; sobre muro blanco, gris piedra o hueso), hierro fundido negro verdoso (#2F3530),
persianas verde inglés. Zócalo de mármol gris o revoque gris.

### Familias por origen (`RASGOS`, ahora con `desde`/`hasta`)
| Tradición | Desde | Rasgos | Marca |
|---|---|---|---|
| Italiana | 1830 | molduras, cornisas, balaustradas, colores pompeyanos (más ornamento) | E |
| Francesa (nueva) | 1830 | persianas, rejas de hierro, revoque gris piedra | E |
| Las coloniales (criolla, canaria, gallega, andaluza, norte de España, lusa, guaraní) | — | siguen pesando en el color, las rejas y el ornamento | ver colonial |

---

## 3. Rehabilitación y modernización (la ciudad con memoria)

Las casas coloniales no se derriban: sus dueños las modernizan **en el lugar**. La reforma de
fachada guarda en el contexto de la obra el año (`reforma`) y los materiales de antes
(`antes`): la planta se sigue dibujando con los materiales con que se levantó (mismo tipo,
ancho, fondo, patios, vanos y puerta: **misma huella**), y solo la cara se rehace con la
fachada de la época. Otras reformas: techo de teja (la paja y el cuero se van) y un piso más.

- Los dueños ricos lo hacen solos (3 % por mes; los demás 0,5 %) si hay cal en el depósito;
  la **ordenanza de fachadas** lo multiplica por 4 en el centro. Así queda parte del caserío
  viejo: en la misma calle se mezclan casas de las dos épocas [E].
- El jugador puede pagar cualquier reforma desde la ficha de la casa.
- Fuente del criterio: GDD §4.2 (conservar, rehabilitar, reurbanizar). Que las casas de
  patio coloniales se "modernizaran" con fachadas italianizantes es un proceso conocido del
  XIX rioplatense; las tasas son [E].

---

## 4. Edificios nuevos

| Edificio | Qué tiene en vóxeles | Fuente |
|---|---|---|
| Usina de gas | Gasómetro de hierro con su jaula de columnas, casa de retortas de ladrillo con chimenea de 20 m, carbonera, oficina con fachada de la época y cerco de hierro | Ubicación y fecha [F]; forma [E] (gasómetros del XIX) |
| Teatro | Escalinata de mármol, pórtico de ocho columnas con entablamento y balaustrada, sala con azotea y pretil, caja escénica con techo de teja, alas laterales, faroles de gas | Fechas y arquitectos [F]; proporciones [E] |
| Usina de agua corriente | Casa de bombas de ladrillo con ventanas en arco y chimenea, tanque elevado de hierro sobre pilares, fuente pública de mármol | [F] fechas; [E] forma (la toma real está en el Santa Lucía, a 56 km) |
| Colector y desagüe | Casilla de inspección de ladrillo con bóveda, reja de hierro y el caño maestro hacia el río | [F] fechas; [E] forma |
| Plazas del ensanche | Suelo nivelado, canteros, caminos en cruz y en diagonal, cuatro palmeras canarias, bancos, faroles de gas y la columna central | [F] la Columna de la Paz (1867); [E] el resto |
| Alamedas | Árboles cada 9 m en el cantero central del bulevar | [E] |

## 4b. Ajustes del ensanche, las redes y las fachadas (v0.11.x)

- **El ensanche se empalma solo con la red vieja** (`City::empalmar`, en `epocas/ensanche.rs`):
  al trazarlo, hasta 6 calles empedradas desde las esquinas del borde del trazado hasta las
  esquinas viejas más cercanas (a menos de 260 m, una por cuadra, hacia afuera del trazado y
  prefiriendo las que siguen derecho una calle del ensanche) [E]. Se pagan aparte, si alcanza.
  Antes el ensanche quedaba como una red de calles suelta y las redes de gas, agua y
  colectores, que siguen la calle, no llegaban (la demostración las unía a mano).
- **Las usinas y la boca del colector se empalman con todas las calles a menos de 60 m**
  (`comps_cerca`), no solo con la más próxima. En la demostración de la época 2 el colector
  cubría 0 solares: su sitio junto a la costa daba al camino suelto que la demostración le
  había hecho al muelle. Ahora cubre todos (266 de 266) y el camino del muelle sale de la
  esquina más cercana de la ciudad.
- **Detalles de las fachadas** como modelos de micro-vóxeles: farol de gas de brazo en las casas
  ricas y los almacenes de esquina desde 1853, macetas con geranios en los balcones (55 %),
  llamadores (ver [modelos-de-detalle.md](modelos-de-detalle.md#detalles-de-las-fachadas-los-constructores-de-edificios-piden-modelos)).

## 5. Pendiente
- Mercado central, aduana nueva, iglesias de la época, cementerio nuevo, hospital, estación
  de diligencias y muelle de vapores (están en el diseño de la época).
- El teatro por fases (cimientos, sala, fachada, cúpula) y la función de estreno.
- Mansardas y la arquitectura de la época 3 (eclecticismo, conventillos).

## Fuentes
- Wikipedia, «Centro, Montevideo»: https://en.wikipedia.org/wiki/Centro,_Montevideo
- Murieda, V., tesina de espacios públicos (FADU): https://www.fadu.edu.uy/tesinas/files/2015/12/TESINA-ESPACIOS-PUBLICOS-VICTORIA-MURIEDA.compressed.pdf
- FHCE, fascículo 15 de los 300 años de Montevideo: https://fhce.edu.uy/wp-content/uploads/2025/12/300A-Fasciculo15-futuro-2025-04-16-web0-todo_compressed.pdf
- La Izquierda Diario, «Mario Ísola, pionero de La Compañía del Gas»: https://www.laizquierdadiario.com/Mario-Isola-pionero-de-La-Compania-del-Gas
- La Izquierda Diario, «El día del gasista»: https://www.laizquierdadiario.com/El-Dia-del-Gasista
- Intendencia de Montevideo, «Sobre los 100 años del saneamiento de Montevideo 1913–2013»: https://montevideo.gub.uy/sites/default/files/sobre_los_100_anos_del_saneamiento_de_montevideo_1913-2013.pdf
- BID, «Saneamiento de Montevideo»: https://blogs.iadb.org/agua/es/saneamiento-montevideo-40
- IMPO, resolución 396/2024 (Red de Saneamiento Arteaga, Monumento Histórico Nacional): https://www.impo.com.uy/bases/resoluciones/396-2024
- Ministerio de Ambiente, «150 años del abastecimiento de agua corriente a Montevideo»: https://www.gub.uy/ministerio-ambiente/comunicacion/noticias/150-anos-del-abastecimiento-aguas-corrientes-montevideo
- Wikipedia, «Aguas Corrientes»: https://en.wikipedia.org/wiki/Aguas_Corrientes
- Wikipedia, «Solís Theatre»: https://en.wikipedia.org/wiki/Sol%C3%ADs_Theatre
- Playbill, «The Oldest Opera House in the Americas Celebrates 150 Years»: https://playbill.com/article/the-oldest-opera-house-in-the-americas-celebrates-150-years
- MEC, Academia Nacional de Letras (la casa y su mirador): https://www.gub.uy/ministerio-educacion-cultura/academia-nacional-letras/institucional/casa
- Wikipedia, «Casa chorizo»: https://en.wikipedia.org/wiki/Casa_chorizo
- Pollero, R. y González Demuro, W., *Población y sociedad de Montevideo* (2024) — ya citado en `poblacion.md`.
