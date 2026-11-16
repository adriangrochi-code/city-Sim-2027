# Motor de vóxeles (Rust)

> Documento de sistema. Complementa al [GDD](../GDD.md).
> Estado: **aplicación interactiva**: ventana con cámara de constructor de ciudades, mundo en
> *streaming* con hilos, sandbox de construir y excavar, y recorrido automático con capturas.
> El modo captura (`--shot`) se mantiene. Código en [`engine/`](../../engine).

![Ciudad colonial de vóxeles](../capturas/motor-voxel-ciudad.jpg)

## 1. Objetivos

- **Estética de vóxeles a la vista** (decisión del proyecto): cada bloque de 0,5 m se nota,
  con su variación de tono, sus bordes y la oclusión ambiental en las esquinas, pero con luz,
  sombras, cielo y niebla cuidados para mantener el "realismo estilizado" rioplatense.
- **Mapas del tamaño de Montevideo** (24 × 24 km) con detalle de 0,5 m cerca de la cámara.
- **Mundo editable**: calles, edificios, terraplenes y, más adelante, excavar, ganar tierra al
  río o dragar el puerto son simplemente cambios de vóxeles.
- **Motor propio en Rust**, sin otro motor de juego: control total del formato de datos, de la
  simulación de agentes y del render.

## 2. Arquitectura

Un solo crate binario, `motor`, organizado en módulos:

| Módulo | Responsabilidad |
|---|---|
| `palette` | Materiales (`u8`, 0 = aire) y paleta de colores rioplatense |
| `terrain` | Geografía del mapa: **puerto exacto** de `scripts/world/terrain_gen.gd` (mismas constantes y fórmulas; ruido FastNoiseLite configurado como el de Godot) |
| `world` / `world::chunk` | Almacén disperso de chunks por nivel, generación por columnas |
| `buildings` | Ediciones de vóxeles, generador aleatorio determinista y vegetación |
| `structures` | Kit modular de estructuras y desgaste por vóxel ([estética](estetica-voxel.md)); `colonial` (catálogo de la época) y `scenes` (escenas `ciudad` y `catalogo`) |
| `lod` | Anillos de nivel de detalle tipo *clipmap* |
| `mesher` | Caras visibles + *greedy meshing* + oclusión ambiental por vértice |
| `camera` | Cámara orbital de constructor de ciudades (Z invertida e infinita) |
| `render` | wgpu: sombras del sol, cielo, vóxeles y agua; captura a PNG/JPG |

Dependencias: `wgpu` (render), `glam` (matemática), `fastnoise-lite` (ruido),
`bytemuck`, `pollster`, `image`.

## 3. Datos

- **Vóxel**: 1 byte de material. Vóxel base de **0,5 m**.
- **Chunk**: 32 × 32 × 32 vóxeles (32 KB). A resolución base cubre 16 m de lado.
- **Disperso**: solo se guardan los chunks que tocan la superficie. Debajo se asume roca
  maciza y encima aire, a partir de la altura de cada columna (`Column.top`), así que un chunk
  enterrado no ocupa memoria.
- **Ediciones**: la ciudad y los árboles se escriben como un mapa de vóxeles a resolución
  base más un mapa de "material de superficie" (calles, plaza, huertas). Cada nivel de detalle
  las reduce a su resolución quedándose con el vóxel fino **más alto** de cada celda, para que
  desde arriba se vea el techo y no un moteado.

## 4. Niveles de detalle

Anillos concéntricos alrededor del foco de la cámara. Cada nivel duplica el tamaño del vóxel
y la distancia que cubre:

| Nivel | Vóxel | Chunk | Anillo (lado) |
|---|---|---|---|
| 0 | 0,5 m | 16 m | 384 m |
| 1 | 1 m | 32 m | 768 m |
| 2 | 2 m | 64 m | 1,5 km |
| 3 | 4 m | 128 m | 3 km |
| 4 | 8 m | 256 m | 6 km |
| 5 | 16 m | 512 m | 12 km |
| 6 | 32 m | 1 km | 24 km |
| 7 | 64 m | 2 km | 49 km |

- Cada anillo tiene 24 × 24 chunks y excluye el hueco que cubre el nivel más fino. Las
  regiones se alinean a la cuadrícula del nivel siguiente, así cada punto del mapa pertenece a
  **exactamente un nivel** (hay un test que lo verifica).
- **Juntas entre niveles**: el mallador trata como aire lo que queda fuera del anillo, y eso
  genera "faldones" verticales que tapan las grietas.
- **Mapa completo**: con 8 niveles ya se cubren los 24 km. Cuando la cámara se mueve, solo
  se regeneran las columnas de chunks que entran en cada anillo (ver §10, *streaming*).

## 5. Render

- **wgpu puro**, con Vulkan, Metal o DirectX 12 según la plataforma. Aquí se probó con el driver
  Vulkan por software (lavapipe).
- **Pase de sombras**: mapa de 4096² ortográfico desde el sol, centrado en la cámara, con
  PCF 3 × 3. El sol pasa por el **norte** (hemisferio sur).
- **Cielo**: degradado del horizonte al cenit, disco y halo del sol, colores según la hora
  (amanecer, mediodía, atardecer y noche).
- **Vóxeles**: color del material con variación por vóxel, bordes marcados que se
  desvanecen con la distancia, oclusión ambiental, luz del sol y del cielo. Las ventanas se
  iluminan de noche con luz de vela.
- **Agua**: plano translúcido al nivel del mar, color "león", con reflejo del cielo
  (Fresnel) y brillo del sol.
- **Niebla de distancia** hacia el color del horizonte, más tenue cuanto más se aleja la cámara.
- **Profundidad con Z invertida** y proyección infinita: precisión de 0,5 m a 40 km.
- MSAA 4×. Pocas llamadas de dibujo: las mallas se agrupan en lotes de hasta 6 millones de
  vértices por nivel.

## 6. Rendimiento medido

Máquina de prueba: 4 núcleos de CPU, **sin GPU** (render por software con lavapipe).
Ciudad de prueba de 7 × 7 manzanas, 1.400 ombúes y monte nativo, 8 niveles.

| Paso | Resultado |
|---|---|
| Ciudad y vegetación | 4,0 millones de vóxeles editados en 1,7 s |
| Generación del mundo | 4.792 chunks en 8 niveles, 3,0 s |
| Mallado (greedy + AO) | 692.000 triángulos en 1,4 s |
| Render 1600 × 900 (CPU) | 0,35–0,6 s por imagen |
| Memoria máxima | ~480 MB |

Triángulos por nivel: 160 k (0,5 m), 150 k (1 m), 99 k (2 m), 50 k (4 m), 49 k (8 m),
51 k (16 m), 29 k (32 m), 105 k (64 m). Con una GPU real el render es de tiempo real holgado;
el costo dominante pasa a ser la regeneración de chunks al mover la cámara.

## 7. Plan para llevar el prototipo de Godot al motor

El prototipo de Godot sigue siendo el banco de pruebas del diseño. Sus sistemas se portan en
este orden:

1. ~~**Ventana e interacción**~~ (hecho, ver §10): cámara con ratón y teclado, selección
   con *raycast* DDA, regeneración incremental de chunks al editar.
2. **Calles**: el grafo de `road_network.gd` (nodos, tramos como polilíneas, Bézier, cruces
   automáticos) se porta tal cual; el trazado se "pinta" en los vóxeles (empedrado o tierra) y
   nivela el terreno bajo la calzada.
3. **Solares y crecimiento**: `lots.gd` (solares a lo largo de la curva, zona mixta, tres
   niveles). Los edificios dejan de ser cajas: se estampan como **plantillas de vóxeles**
   (las de `buildings.rs` son el punto de partida), con variantes y rotación.
4. **Vecinos y conducción**: `agents.gd` (rutinas, A* por tiempo medido, IDM, cesión de
   paso, adelantamiento) se porta a Rust sin cambios de lógica; el render usa instancias
   (*instancing*) de pequeños modelos de vóxeles.
5. **Ciclo día/noche, parcelas, interfaz** (la interfaz probablemente con `egui`).
6. **Guardado**: los chunks editados se guardan comprimidos (RLE o LZ4); el resto se
   regenera desde la semilla.

## 8. Riesgos y decisiones abiertas

- **¿Por qué wgpu puro y no Bevy?** Bevy aporta ECS, ventana e interfaz, pero cambia su API
  en cada versión y su render genérico no está pensado para millones de vóxeles con
  niveles de detalle. Para un juego cuyo núcleo es el mundo de vóxeles y una simulación
  propia, conviene controlar el render y los datos. wgpu da el mismo acceso multiplataforma
  sin imponer arquitectura. Si más adelante hace falta ECS, se puede usar `hecs` o
  `bevy_ecs` sueltos.
- **Juntas entre niveles**: los faldones a veces se ven como líneas finas en la vista lejana.
  Alternativas: transiciones con vóxeles intermedios o un leve desplazamiento vertical.
- **Edificios en niveles gruesos**: a partir de 4 m por vóxel las casas se reducen a
  manchas. Para la vista de toda la ciudad conviene un impostor o un mapa de alturas de
  techos.
- **Memoria**: 4 millones de ediciones viven en un `HashMap`. Para ciudades grandes conviene
  guardarlas directamente en chunks dispersos de ediciones.
- **Generación incremental y en segundo plano**: hecha (§10). Queda pendiente quitar las
  columnas viejas antes, sin esperar a que el *streamer* quede ocioso.
- **Árboles y agua**: el agua es un plano. Los arroyos sobre el nivel del mar, las mareas y
  las sudestadas necesitarán un nivel de agua por zona.
- **Estética**: la variación de tono y los bordes de los vóxeles deben ajustarse con arte
  real (paleta definitiva, materiales con más textura: adoquín, ladrillo visto, paja).

## 9. Cómo ejecutarlo

```
cd engine
cargo run --release -- --shot ciudad.png
cargo run --release -- --shot cerca.png --cam 13470,16190,70,160,24 --hour 9.5
cargo run --release -- --shot lejos.png --cam 12600,15000,9000,190,42 --hour 16.5
cargo run --release                                  # ventana interactiva
cargo run --release -- --msaa 1                      # ventana sin MSAA (más rápida sin GPU)
cargo run --release -- --auto-test salida/           # recorrido automático sin ventana
cargo run --release -- --window --auto-test salida/  # el mismo recorrido en la ventana
cargo run --release -- --frames 300                  # ventana que se cierra sola
cargo test --release
```

Opciones nuevas: `--threads N` (hilos de generación; por defecto núcleos − 1),
`--smooth 0|1` (normal suave del terreno, activada por defecto), `--msaa 1|4`.
En Linux sin pantalla la ventana funciona con `xvfb-run` (probado con lavapipe; hace falta
`libxkbcommon-x11-0`).

`--cam x,z,distancia,giro,inclinación` (metros y grados), `--hour` (0–24), `--size 1600x900`,
`--levels 8`. Hace falta un driver Vulkan, Metal o DirectX 12. En Linux sin GPU sirve el
driver por software de Mesa (`mesa-vulkan-drivers`).

## Novedades: estética de estructuras

- **Kit modular** (`structures`): zócalo, cuerpo, coronación, aberturas y elementos sobre un
  marco de solar; variantes deterministas por semilla y desgaste por vóxel. Detalle en
  [estetica-voxel.md](estetica-voxel.md).
- **Catálogo colonial**: rancho, casa de azotea, casa de altos, pulpería, cabildo, iglesia
  matriz en 5 fases, muralla con portón y garita, muelle, barraca, saladero, estancia,
  atahona y molino.
- **Escenas**: `--scene ciudad` (por defecto) y `--scene catalogo`.
- **Detalle por material en el shader**: el número de material viaja en el alfa del color
  del vértice; el shader dibuja rejas, tejas, paja, sillería y azulejos por debajo del
  tamaño del vóxel.
- **Métricas** (4 núcleos, sin GPU):

| Escena | Vóxeles editados | Generación | Mallado | Triángulos | Memoria máxima |
|---|---|---|---|---|---|
| Ciudad | 4,3 M en 1,8 s | 3,4 s | 1,7 s | 1,24 M (antes 0,69 M) | 478 MB |
| Catálogo | 1,1 M en 0,3 s | 1,3 s | 1,2 s | 0,80 M | 218 MB |

  Los triángulos de la ciudad subieron un 79 %: el desgaste y los detalles cortan las caras
  que antes el *greedy meshing* unía.
- **Tests**: 13 en total (4 nuevos: determinismo por semilla, casas dentro del solar, la
  iglesia crece por fases y termina sin andamios, desgaste determinista).


## 10. Aplicación interactiva y *streaming*

![Ventana interactiva](../capturas/motor-ventana.jpg)

### Controles

| Entrada | Acción |
|---|---|
| WASD / flechas | Desplazar la cámara (velocidad proporcional a la distancia) |
| Q / E | Girar |
| Rueda | Zoom de 15 m a 15 km |
| Botón central (arrastrar) | Orbitar |
| Botón derecho (arrastrar) | Desplazar |
| Clic izquierdo | Colocar la estructura elegida (huella verde bajo el cursor) |
| Clic derecho sin arrastrar | Excavar (radio 3 vóxeles) |
| F | Rellenar terreno (radio 4) |
| 1–9 / Tab | Elegir estructura del catálogo (14: rancho, casas, pulpería, aljibe, ombú, cabildo, iglesia, muralla, atahona, molino, estancia, saladero…) |
| R | Rotar la estructura 90° |
| `[` / `]` | Cambiar la hora |
| P | Pausar el día (un día dura 8 minutos) |
| F12 | Guardar captura PNG |

El título de la ventana muestra FPS, triángulos, columnas cargadas, trabajos pendientes,
memoria y la estructura elegida. La cámara sigue la altura del terreno.

### Arquitectura

| Módulo | Responsabilidad |
|---|---|
| `store` | `EditStore`: ediciones ya reducidas a cada nivel, por chunk. El terreno no se guarda: es una función. A igual altura gana la edición más reciente. |
| `stream` | `Streamer`: hilos de trabajo con cola de prioridad (cercanía al foco; ediciones urgentes), trabajos versionados (se descartan resultados viejos), anillos que se recentran, presupuesto de vértices subidos por cuadro, retiro de columnas lejanas. Cada columna se malla sola: el borde de un vóxel se calcula directamente. |
| `raycast` | Marcha sobre el mapa de alturas y luego DDA (Amanatides–Woo) por los vóxeles editados. |
| `sandbox` | Catálogo de estructuras, colocar con rotación, excavar y rellenar como ediciones. |
| `session` | Estado de juego compartido entre ventana y recorrido: cámara, hora, escena cargada en segundo plano, *overlay* del cursor. |
| `app` | Ventana winit + superficie wgpu (vsync, cambio de tamaño). |
| `tour` | Recorrido automático `--auto-test` con capturas y métricas. |

Al editar, solo se re-mallan las columnas de chunks tocadas y sus vecinas de borde (1 a 6
columnas por edición); hay un test que lo comprueba.

### Líneas oscuras en los escalones del terreno

Cada escalón de 0,5 m dibujaba una cara lateral con borde y oclusión ambiental, y desde lejos
eso se veía como rayas oscuras. Solución: cada vértice de terreno lleva una **normal suave de
la ladera** (gradiente de las alturas vecinas) y el shader la usa para iluminar el terreno; de
cerca, las caras laterales se mezclan con su normal real para que los vóxeles sigan notándose.
Además se suavizó la oclusión y el borde en materiales de terreno.

![Escalones antes y después](../capturas/motor-escalones-antes-despues.jpg)

### Métricas (4 núcleos, sin GPU, lavapipe)

| Medida | Resultado |
|---|---|
| Renderer listo | 65 ms |
| Primer cuadro en la ventana | 0,2–0,7 s (el terreno aparece enseguida; la ciudad se carga en segundo plano) |
| Primeras columnas malladas | 4 ms tras empezar la sesión |
| Escena completa visible | ~3,6–4,3 s |
| Vista general ociosa | 4,9 s; 3.600 columnas; 1,24 M triángulos; mallas 71–102 MB |
| Viaje de 3,3 km | listo en ~1,9 s |
| Re-mallado al editar | 1–6 columnas |
| Memoria | 330–450 MB sin capturas; 540–760 MB con capturas (pico ~800 MB) |
| FPS por software 1280 × 720 | 2,6 con MSAA 4×; 3,6–4,7 con MSAA 1× |
| Recorrido `--auto-test` completo | ~18 s |

Se corrigió un crecimiento de memoria hasta 900 MB: los búferes creados con datos iniciales no
se liberaban sin un `queue.submit` periódico; ahora se envía uno vacío y se hace `poll` en cada
cuadro.

![Sandbox](../capturas/motor-sandbox.jpg)
![Cerro](../capturas/motor-cerro.jpg)
![Plaza en streaming](../capturas/motor-plaza-streaming.jpg)

### Problemas conocidos

- Bandas tenues en algunas laderas y líneas finas en las juntas entre niveles.
- FPS bajos por software: con una GPU real no debería ser un problema, pero no está medido.
- Las capturas suben la memoria unos 200 MB (MSAA y compilación JIT de lavapipe).
- Las estructuras se apoyan en una sola altura de base y pueden solaparse con edificios
  existentes (no hay comprobación de colisiones).
- Las columnas retiradas se quitan cuando el *streamer* queda ocioso (o pasado un límite), así
  que al moverse rápido hay mallas solapadas un momento.
- Solo 4 rotaciones (90°).

**Tests**: 18 en total (nuevos: orden de carga por cercanía, re-mallado solo de columnas
tocadas, DDA sobre terreno y edificios, normal suave, edición más reciente gana).
