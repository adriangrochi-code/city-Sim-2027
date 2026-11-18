# Hoja de ruta

> Estado al 17 de noviembre de 2026. Se actualiza con cada avance.
> Dos líneas de trabajo: el **prototipo de Godot** (para diseñar y probar la jugabilidad) y el
> **motor de vóxeles en Rust** (donde va a vivir el juego final).

## Hecho

| Área | Qué está |
|---|---|
| Diseño | GDD, las 6 épocas en detalle, agentes y conducción, edificios ampliables, motor y estética de vóxeles |
| Prototipo: época colonial | Mapa por parcelas, calles curvas, solares, crecimiento en 3 niveles, vecinos con conducción realista, agua, incendios, puerto y flujos, muralla y aduana, serenos, campo, iglesia por fases, ampliar y modernizar, nuevo orden, guardar/cargar |
| Motor de vóxeles | Mundo por chunks con LOD, streaming en hilos, catálogo colonial en vóxeles, ventana interactiva, sandbox, optimizado (dibujo indirecto, caras compactas, descarte en GPU) |
| Distribución | Versión de prueba para Linux (probada) y Windows (compilada) con `tools/empaquetar.sh` |
| Mundo real en el motor | MDT de 30 m de la IDE, hidrografía oficial (arroyos, cañadas, bañados, playas, islas), curvatura, puentes, ríos con corriente, mareas y ciclo lunar, sequías e inundaciones, clima con datos del INUMET, viento |
| Paisaje de 1726 | Monte nativo calibrado con datos y testimonios (4,2 % del mapa, galerías por clase de cauce), 9 especies de árboles con variantes, flora baja nativa por zona, suelos y afloramientos (CONEAT), costa por tramos (arena, mixta, piedra, restingas) — ver `docs/sistemas/vegetacion-historica.md` y `costa.md` |
| Entorno amplio | 300 × 300 km de relieve real alrededor del mapa (MDT de la IDE + Copernicus para la costa argentina, monte y bañados), 11 niveles de detalle (la tierra llega al horizonte, ~90 km), sin cortes rectos entre grillas ni contra el agua — ver `docs/sistemas/mapa-real.md#entorno-amplio` |
| Mapa del país | El Uruguay a 1 km con datos de la IDE, el MTOP, la DGF y DINAMIGE; pueblos, caminos, puertos, estancias y saladeros por fecha; carretas al puerto de Montevideo que alimentan la economía de la ciudad; modo «Mapa del país» (M) — ver `docs/sistemas/mapa-pais.md` |
| Calidad visual | Vóxel de 0,25 m, flora en micro-vóxeles de 0,125 m, copas con racimos, nivel de detalle del monte, agua sin tapar tierra en ningún nivel, niebla con sol |

## Fase 1: cerrar la época colonial (prototipo)

- [x] Lazareto y epidemias que llegan por barco (cuarentena)
- [x] Cadenas de producción: horno de ladrillos (las casas de azotea y la iglesia los necesitan) y atahona (harina para la ciudad)
- [ ] Probar la versión de Windows en un equipo real y medir FPS con GPU real

## Fase 1b: gobernar la ciudad (prototipo)

- [x] Recursos con mercado (pan y carne con precio), pósito de granos y sequías
- [x] Opinión de tres sectores (vecinos, comerciantes, hacendados) con consecuencias
- [x] Ordenanzas del cabildo (9 en la época colonial) — ver `docs/sistemas/politicas-y-recursos.md`
- [x] Mejorar calles sin demoler (empedrar con piedra de cantera, poner faroles con costo mensual)
- [x] Mapa por defecto con la geografía real de Montevideo (24 km, ver `docs/sistemas/mapa-real.md`)
- [x] Casas y calles a tamaño real (cuadra de 100 varas, calles de 10 y 12 varas)
- [x] Uruguay a escala país; las calles y solares deben esquivar los arroyos — mapa del país en el motor (relieve de la IDE a 1 km, agua, 19 departamentos, ríos, monte, bañados, 33 pueblos con su año de fundación verificado, caminos reales, ferrocarril por año, recursos; estancias, saladeros y carretas que llevan cueros, sebo y tasajo al puerto) y solares fuera de cauces, orillas y bañados, damero cortado en los arroyos y aviso de puentes al trazar (ver [sistemas/mapa-pais.md](sistemas/mapa-pais.md))
- [ ] Alcantarillas y saneamiento por épocas (diseñado; **en parte**: la versión colonial —letrinas y pozos negros por tipo de casa, pozos de agua que se contaminan, basurales, recolección con carretilla y basurero— ya está en el motor; faltan los colectores y el agua corriente de las épocas siguientes, que ya figuran como datos)
- [ ] Alumbrado por épocas: gas, eléctrico, sodio, LED (diseñado; la versión colonial ya existe)

## Fase 2: el juego sobre el motor de vóxeles

El paso clave: dejar de depender del prototipo y jugar directamente sobre los vóxeles.

1. [x] Calles curvas, rectas y damero en el motor (grafo de calles, cruces, calzada en vóxeles)
2. [x] Solares que siguen la calle, zonificación con pincel y crecimiento con el catálogo de vóxeles (rancho → azotea → altos), con edificios girados a cualquier ángulo
3. [x] Interfaz del juego en el motor: categorías con cajón de tarjetas, barra de fichas, ficha de información, gaceta, ordenanzas y presupuesto con impuestos
3b. [x] Economía en el motor: existencias, mercado, comercio por el puerto, impuestos, presupuesto, opinión y ordenanzas
3c. [x] Mapa real de Montevideo en el motor (geografía, agua, clima, vegetación y costa reales)
4. [x] Vecinos y vehículos con el modelo de conducción IDM (portar desde el prototipo)
5. [x] Servicios, agua, incendios, puerto, muralla, campo e iglesia (primera versión; ver [sistemas/servicios.md](sistemas/servicios.md))
5b. [x] Población: personas con personalidad e historial, familias y linajes, llegadas por época, economía de cada hogar, desigualdad y felicidad por grupo. Decisión (17 de noviembre de 2026): el juego no representa la esclavitud; todas las personas son libres y se quitaron la condición legal, el patronato, las leyes y ordenanzas de libertad, las llegadas forzadas y sus hechos de la crónica (ver [sistemas/poblacion.md](sistemas/poblacion.md), sección 5)
5d. [x] Trazado de calles práctico: enganche a 45° y 90° de la calle de arranque y del damero, largo alineado con las esquinas vecinas o el módulo de la cuadra, guías punteadas, largo y ángulo en pantalla, Shift para trazar libre
5c. [x] Pendiente de las calles medida en tramos de 10 m con la altura promediada (antes, una bocha de piedra o la ondulación fina cortaba el damero del mapa real: la ciudad de la demostración pasa de ~450 a ~1.600 vecinos y nadie queda sin poder salir)
5e. [x] Educación, salud, seguridad y saneamiento coloniales: escuela de primeras letras y convento (cupo de alumnos y alcance), botica, médico y hospital de caridad (por la red de calles), garita del alcalde de barrio y cárcel del Cabildo junto a los serenos, delito por solar (pobreza, desigualdad, desocupación, oscuridad, baldíos, puerto) con robos, asaltos y contrabando en la gaceta, pozos que se contaminan con los pozos negros, basurales, recolección y basurero; efectos en la lectura, la mortalidad, la salud y la felicidad de cada persona; pestañas del cajón, capas y fichas; las épocas siguientes como datos (ver [sistemas/educacion-salud-seguridad.md](sistemas/educacion-salud-seguridad.md))
5f. [x] Desarrollo: saberes de la población (leen, artesanos, oficiales formados, pilotos), árbol de tecnologías por época (9 coloniales jugables: escuela de oficios, curtido, horno de bóveda, saladero, grasería, molino, carpintería de ribera, náutica, imprenta; las épocas 2 a 6 como datos) con inversión del cabildo en cuotas que se para sin plata; industrias con perfil (empleo, capital, saber, insumos, exportación) e inversores privados que deciden con motivos visibles; incentivos (exención, crédito de fomento, aranceles protectores, privilegios, maestros de ultramar); comercio exterior por producto con precios del mundo, choques, balanza y diversificación; seis edificios nuevos en vóxeles; panel Desarrollo (ver [sistemas/desarrollo-y-tecnologia.md](sistemas/desarrollo-y-tecnologia.md))
5g. [x] Recursos del suelo y zonas: calidad de la tierra con el índice de productividad CONEAT y la aptitud para cultivos (MGAP), geología de DINAMIGE y mapas de piedra, arcilla, arena y canto rodado; chacras, estancias, hornos y canteras rinden según su sitio (sin piedra no hay cantera); molestias de cada industria (olor, ruido, humo y agua sucia que baja por el arroyo) y valor del suelo por solar (acceso, servicios, molestias, recursos) que ordena el crecimiento y los alquileres; capas de datos, fichas y vista previa del sitio (ver [sistemas/recursos-del-suelo.md](sistemas/recursos-del-suelo.md))
5h. [x] Catálogo de los recursos naturales del Uruguay (47 recursos y 39 productos: rocas, calizas, áridos, arcillas, minerales industriales, gemas, metales, combustibles, agro, ganado y monte) con geología, departamentos, usos, producción, historia, precio y fuentes (MEDIDO / DOCUMENTADO / ESTIMADO); 1.975 títulos mineros con su sustancia (catastro de DINAMIGE) y la geología del país a 1 km para el mapa futuro; en el mapa de Montevideo, once materiales con tipo (granito, gneis, balasto, canto rodado, arena, arcilla roja y gris, tosca, conchilla, caliza, turba); la cantera saca lo que hay (adoquines y sillería del granito, arena…), el horno cuece según su barro (tejas con la arcilla gris), horno de cal que pide caliza o conchilla y cal importada si no hay; productos nuevos en el comercio; capas por grupo de material y ficha del terreno con los materiales (ver [sistemas/recursos-de-uruguay.md](sistemas/recursos-de-uruguay.md))
5i. [x] Materiales de construcción: cada edificio pide materiales (barro, paja, adobe, cuero, madera, ladrillo, teja, piedra, cal, arena, hierro, vidrio) y se levanta con lo que hay, sustituyendo en orden hasta barro y paja (construir nunca se prohíbe); la obra de cada casa y servicio decide el fuego, lo que dura (desgaste, arreglos y derrumbes), la humedad (salud), el prestigio (valor), los pisos (sin cal no hay altos) y el mantenimiento, y se ve en los vóxeles (ladrillo visto, adobe, tablas, barro con paja, mampostería, techos de tablas y cuero); demanda, precios e importación por el puerto, avisos de falta en la gaceta, ficha y panel de materiales (ver [sistemas/materiales-de-construccion.md](sistemas/materiales-de-construccion.md)). Falta: calera y horno de tejas propios, rehabilitar en el lugar
5j. [x] Variedad de la arquitectura colonial: tipologías de la época (rancho de cuero, rancho de adobe, terrón o quincha, casa de teja con hastial o en "L", casa de azotea de uno o dos patios con zaguán y aljibe, casa de altos con balcones, cuartos de alquiler, pulpería de esquina con pilar o con ochava, casa con tienda, quinta extramuros con galería) elegidas por época, barrio, clase y origen de la familia y sus materiales reales; fachada procedural (vanos, ritmo, puerta, zaguán, rejas plana o voladiza, postigos, balcones de hierro o de madera, cornisas, pretiles, gárgolas, portadas, zócalos), color de cal con los pesos de cada época, carpinterías, desgaste del blanqueo, revoque caído y tejas viejas; altillos y miradores (raros y nunca antes de 1790), ampliaciones al fondo y regla anti-monotonía entre vecinos; el contexto se guarda con la obra; cuerpos huecos: menos de la mitad de vóxeles (ver [sistemas/arquitectura-colonial.md](sistemas/arquitectura-colonial.md)). Falta: casas dañadas por los sitios (1807–1814), cuarteles y almacenes de bóveda, techo a cuatro aguas, balcones corridos de esquina
6. [x] Guardar y cargar en el motor (menú principal, menú de pausa con Esc, F5 / F9, opciones y autoguardado)
7. [ ] Hi-Z (descarte de lo tapado), sombras en cascadas, medir en GPU real — hechos el Hi-Z (dos pasadas en el mismo cuadro, sin errores al mover la cámara: hasta 62 % menos caras a nivel de la calle) y las sombras en cascadas (2 a 4, estables, en caché por cascada, calidad en Opciones; sombras hasta 8 veces la distancia de la cámara); arreglados los huecos de terreno al hacer zoom con placa real (ver [sistemas/motor-voxel.md](sistemas/motor-voxel.md) §17). Falta: medir en GPU real (`medir-fps.bat` ya junta las tablas)
8. [x] Vóxel de 0,25 m con capa de detalle de 0,125 m (flora) y monte con nivel de detalle
9. [x] Hojas sueltas de 0,125 m en la superficie de las copas (tiemblan con el viento)
10. [ ] Ramas colgantes del sauce en micro-vóxeles, curva tonal

## Fase 3: diseño de todas las épocas

- [x] Épocas 4, 5 y 6 en detalle (Art Déco y la Rambla, Modernismo, Contemporánea): borradores v0.1
- [ ] Kits de vóxeles por época (estética de estructuras de la época 2 en adelante)
- [x] Mapa del país (escala país) en detalle: datos, fuentes, simulación agregada, obras por época y su modo en el motor ([sistemas/mapa-pais.md](sistemas/mapa-pais.md)). Falta: ciudades secundarias jugables, ferrocarril del jugador, represas y rutas (épocas 3-6)

## Fase 4: época 2, Ciudad Nueva

- [ ] Ensanche planificado sobre el ejido, gas, teatro, mapa del país con pueblos y caminos (**en parte**: el mapa del país ya deja fundar pueblos, trazar caminos con puentes y abrir puertos desde la Ciudad Nueva; faltan diligencias y postas)
- [ ] Rehabilitación de casas viejas y modernización de edificios coloniales

## Más adelante

- Épocas 3 a 6 jugables, escala país, cooperativo asíncrono, modding, nombre definitivo.
