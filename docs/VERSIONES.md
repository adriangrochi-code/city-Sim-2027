# Versiones

Cada build dice su versión en el menú principal, el título de la ventana, `motor --version`,
los informes (`bench.md`, `tirones.txt`) y las partidas guardadas: número (de
`engine/Cargo.toml`), commit de git y fecha, por ejemplo `v0.9.0 (a6f6997, 2026-11-18)`.
El paquete se llama `city-sim-2027-vX.Y.Z-<commit>-windows.zip` (`tools/empaquetar-motor.sh`).

Cómo numerar (mientras el juego esté en desarrollo, 0.x):
- **0.X.0**: un conjunto de sistemas nuevos o un cambio grande (subir X).
- **0.X.Y**: arreglos y ajustes sobre la anterior (subir Y).
- Cada versión que se manda a probar se anota acá y lleva una etiqueta de git `vX.Y.Z`.

## v0.9.0 (2026-11-18)
Primera versión numerada. Juego en el motor de vóxeles sobre la geografía real de Montevideo:
- Mundo: relieve, arroyos, bañados, costa por tramos, monte de 1726, clima, mareas y viento;
  300 km de relieve real alrededor (la tierra llega al horizonte).
- Ciudad: calles con enganche a 45°/90°, zonas con aptitud, vecinos y carretas, población con
  familias, desigualdad y felicidad, servicios (agua, salud, educación, seguridad,
  saneamiento, puerto, muralla, iglesia), desarrollo e industrias, recursos del suelo reales,
  materiales de construcción y arquitectura variada por época, riqueza y origen.
- Render: vóxel de 0,25 m, flora de 0,125 m, sombras en cascada, descarte de lo tapado (Hi-Z).
- Arreglos de esta tanda: huecos de terreno al mover la cámara, muelle que se engancha a la
  orilla, cursor libre por defecto, luz de atardecer. El juego no representa la esclavitud.
