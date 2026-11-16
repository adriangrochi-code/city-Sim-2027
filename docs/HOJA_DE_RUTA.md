# Hoja de ruta

> Estado al 16 de noviembre de 2026. Se actualiza con cada avance.
> Dos líneas de trabajo: el **prototipo de Godot** (para diseñar y probar la jugabilidad) y el
> **motor de vóxeles en Rust** (donde va a vivir el juego final).

## Hecho

| Área | Qué está |
|---|---|
| Diseño | GDD, las 6 épocas en detalle, agentes y conducción, edificios ampliables, motor y estética de vóxeles |
| Prototipo: época colonial | Mapa por parcelas, calles curvas, solares, crecimiento en 3 niveles, vecinos con conducción realista, agua, incendios, puerto y flujos, muralla y aduana, serenos, campo, iglesia por fases, ampliar y modernizar, nuevo orden, guardar/cargar |
| Motor de vóxeles | Mundo por chunks con LOD, streaming en hilos, catálogo colonial en vóxeles, ventana interactiva, sandbox, optimizado (dibujo indirecto, caras compactas, descarte en GPU) |
| Distribución | Versión de prueba para Linux (probada) y Windows (compilada) con `tools/empaquetar.sh` |

## Fase 1: cerrar la época colonial (prototipo)

- [x] Lazareto y epidemias que llegan por barco (cuarentena)
- [x] Cadenas de producción: horno de ladrillos (las casas de azotea y la iglesia los necesitan) y atahona (harina para la ciudad)
- [ ] Probar la versión de Windows en un equipo real y medir FPS con GPU real

## Fase 2: el juego sobre el motor de vóxeles

El paso clave: dejar de depender del prototipo y jugar directamente sobre los vóxeles.

1. [x] Calles curvas, rectas y damero en el motor (grafo de calles, cruces, calzada en vóxeles)
2. [x] Solares que siguen la calle, zonificación con pincel y crecimiento con el catálogo de vóxeles (rancho → azotea → altos), con edificios girados a cualquier ángulo
3. [ ] Interfaz del juego en el motor (barra de herramientas, información, plano colonial)
4. [ ] Vecinos y vehículos con el modelo de conducción IDM (portar desde el prototipo)
5. [ ] Servicios, agua, incendios, puerto, muralla, campo e iglesia
6. [ ] Guardar y cargar en el motor
7. [ ] Hi-Z (descarte de lo tapado), sombras en cascadas, medir en GPU real

## Fase 3: diseño de todas las épocas

- [x] Épocas 4, 5 y 6 en detalle (Art Déco y la Rambla, Modernismo, Contemporánea): borradores v0.1
- [ ] Kits de vóxeles por época (estética de estructuras de la época 2 en adelante)
- [ ] Mapa del país (escala país) en detalle

## Fase 4: época 2, Ciudad Nueva

- [ ] Ensanche planificado sobre el ejido, gas, teatro, mapa del país con pueblos y caminos
- [ ] Rehabilitación de casas viejas y modernización de edificios coloniales

## Más adelante

- Épocas 3 a 6 jugables, escala país, cooperativo asíncrono, modding, nombre definitivo.
