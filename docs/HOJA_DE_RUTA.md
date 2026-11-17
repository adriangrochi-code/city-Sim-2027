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
- [ ] Uruguay a escala país; las calles y solares deben esquivar los arroyos
- [ ] Alcantarillas y saneamiento por épocas (diseñado)
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
5b. [ ] *(en curso)* Población: personas con personalidad e historial, familias y linajes, llegadas por época, economía de cada hogar, desigualdad y felicidad por grupo
5c. [x] Pendiente de las calles medida en tramos de 10 m con la altura promediada (antes, una bocha de piedra o la ondulación fina cortaba el damero del mapa real: la ciudad de la demostración pasa de ~450 a ~1.600 vecinos y nadie queda sin poder salir)
6. [x] Guardar y cargar en el motor (menú principal, menú de pausa con Esc, F5 / F9, opciones y autoguardado)
7. [ ] Hi-Z (descarte de lo tapado), sombras en cascadas, medir en GPU real
8. [x] Vóxel de 0,25 m con capa de detalle de 0,125 m (flora) y monte con nivel de detalle
9. [ ] Hojas de los árboles en micro-vóxeles (sauces con ramas colgantes de cerca), curva tonal

## Fase 3: diseño de todas las épocas

- [x] Épocas 4, 5 y 6 en detalle (Art Déco y la Rambla, Modernismo, Contemporánea): borradores v0.1
- [ ] Kits de vóxeles por época (estética de estructuras de la época 2 en adelante)
- [ ] Mapa del país (escala país) en detalle

## Fase 4: época 2, Ciudad Nueva

- [ ] Ensanche planificado sobre el ejido, gas, teatro, mapa del país con pueblos y caminos
- [ ] Rehabilitación de casas viejas y modernización de edificios coloniales

## Más adelante

- Épocas 3 a 6 jugables, escala país, cooperativo asíncrono, modding, nombre definitivo.
