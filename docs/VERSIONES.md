# Versiones

Cada build dice su versión en el menú principal, el título de la ventana, `motor --version`,
los informes (`bench.md`, `tirones.txt`) y las partidas guardadas: número (de
`engine/Cargo.toml`), commit de git y fecha, por ejemplo `v0.9.0 (a6f6997, 2026-11-18)`.
El paquete se llama `city-sim-2027-vX.Y.Z-<commit>-windows.zip` (`tools/empaquetar-motor.sh`).

Cómo numerar (mientras el juego esté en desarrollo, 0.x):
- **0.X.0**: un conjunto de sistemas nuevos o un cambio grande (subir X).
- **0.X.Y**: arreglos y ajustes sobre la anterior (subir Y).
- Cada versión que se manda a probar se anota acá y lleva una etiqueta de git `vX.Y.Z`.

## v0.11.0 (2026-11-23)
**Atlas ecológico histórico del Uruguay** (`docs/sistemas/atlas-ecologico.md`):
- Capa «Ecosistemas» en el mapa del país (tecla M): los 18 códigos del atlas más 3 propios
  (PL01 plantación, UR01 urbano, TR01 transformado sin dato), por período (1750–1800 base,
  1860–1900, desde 1900 plantación de Canelones, desde 1950, desde 1985 observado).
- Cada celda con evidencia (documentada / inferida / desconocida), confianza y fuente;
  mosaico secundario como punto, lo desconocido rayado; ficha con MapBiomas del departamento.
- Palmares de butiá (Rocha) y yatay (Quebracho, Guichón), monte de quebrada, serrano, parque,
  pastizal ralo y denso, chircales; alrededor de la ciudad, terreno según la capa base.
- Límite: las palmeras no llegan al terreno 3D (el entorno no alcanza Castillos ni Paysandú).

**Técnicas del mod LOD, fases 0, 1, 2 y 5** (`docs/sistemas/tecnicas-minecraft-lod.md`):
- Calidad Mínimo a Ultra en Opciones, con la recomendada (★) según el equipo; la primera vez
  se aplica sola. Auto-ajuste según el límite (CPU o GPU), consciente del vsync.
- Caché de columnas en RAM con límite de bytes: acercar y alejar ya no regenera nada.
- Lo tapado por el relieve se genera al final (−15 a −22 % de columnas a nivel de calle).
- Prioridad a lo que se ve y subida a la GPU con tope por cuadro.
- Pendiente: el vuelo rápido tiene picos peores (hasta 24 ms en render por CPU) en la
  subida/quitado de columnas del renderer; todo calibrado sin GPU real.

## v0.10.1 (2026-11-18)
**Personajes y animales en micro-vóxeles de 1/16 m** (unas 27 alturas por persona):
- 19 atuendos por época, sexo, edad, oficio y riqueza (paisano con poncho y chiripá, vecino
  con casaca y tricornio, señora con mantilla, levita, dama con peinetón…), de pie y montados;
  la misma persona viste igual el mismo año.
- Animación: caminar sin que patinen los pies, caballos y vacas al paso, pastar, gallinas que
  picotean, ruedas de carreta que giran lo recorrido.
- Animales derivados de la ciudad: rodeo de cada estancia según su rinde, tropillas, ovejas
  desde 1840, yuntas, gallinas, perros, caballos en el palenque de las pulperías y botes en
  el muelle que flotan con la marea.
- Biblioteca de modelos de detalle (faroles, toneles, aljibes, palenques, carretas, botes…)
  lista para que los edificios la usen.
- Micro-vóxeles hasta 110 m, cajas simples hasta 600 m; sombra de contacto suave.

## v0.10.0 (2026-11-18)
Época 2, **Ciudad Nueva**, jugable en el motor:
- Época explícita en la ciudad (se guarda; las partidas viejas son coloniales): requisitos,
  acontecimiento del nuevo orden (modo fiel 1828–1830 o historia alternativa), decisión de la
  muralla, pantalla de resumen de la época colonial; las llegadas y las tecnologías de la
  época 2 se activan con el paso.
- Ensanche sobre el ejido con bulevar arbolado y plazas; casas neoclásicas e italianizantes
  (casa de patio, casa de altos con balcón corrido de hierro fundido, quinta, almacén de
  esquina); rasgos de familias italianas y francesas.
- Rehabilitación en el lugar de las casas coloniales (fachada nueva con la misma huella,
  techo de teja, un piso más) y ordenanza de fachadas.
- Usina y faroles de gas con luz de noche en el shader (grilla de luces cerca de la cámara),
  menos delito y carbón importado; usina de agua corriente y colectores por la red de
  calles; teatro.
- 9 materiales nuevos en la paleta (91 de 96). Recorrido `MOTOR_EPOCA2=1 motor --demo-ciudad`.

**Mapa del país** (tecla M): todo el Uruguay a ~1 km por celda con relieve, ríos, bañados,
monte y departamentos reales; pueblos, caminos reales, represas y ferrocarril en su año;
carretas que llevan cueros, sebo, tasajo y ganado a Montevideo; obras del cabildo por época.
Sigue la época de la ciudad. En la ciudad, los solares y el damero esquivan cauces y bañados.

**Agua, suelo y caminos en vóxeles**: agua de la bahía, el río y los arroyos como vóxeles
(la marea sube en pasos de 0,25 m sin rearmar mallas, nunca sobre tierra seca); suelo con
aspecto de vóxel de cerca (Opciones → Aspecto del terreno); calles de piedra con vereda,
cordón y cuneta, caminos de tierra con huellas de carreta; rampas en los puentes.

Personajes y animales en micro-vóxeles quedan para la v0.10.1.

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
