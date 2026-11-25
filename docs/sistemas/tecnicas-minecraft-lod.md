# Técnicas del mod LOD de Minecraft aplicadas al motor de vóxeles

> Documento de plan. Complementa a [motor-voxel.md](motor-voxel.md) (sobre todo §4, §10–12, §16 y
> §17). Fecha: 2026-11-18. Es el inventario, la comparación y el plan por fases para llevar al
> motor (`engine/`) lo que el mod LOD de Minecraft (`/home/user/minecraft-lod`, Java/NeoForge,
> versión 0.26.38) ya resolvió y midió.
>
> **Estado al 2026-11-24 (segunda tanda, §7):** hechas las fases **0, 1, 2, 4 y 5**, la **3**
> (prefiltrado, oclusión fuera del *greedy*, cobertura de ediciones y relleno de la costa, con
> lo que se fue `far_drop`) y la **6** salvo el tinte de la flora en el suelo (§2.15 a): SSAO
> lejano y escala de render con la prueba "apagar si no gana", detrás del preset y del
> auto-ajuste. Además: sombra proyectada de personajes y animales y ramas colgantes del sauce.
> Ver §6 (primera tanda) y §7. Cambios marcados con `[render]`. Código nuevo: `engine/src/cache.rs`,
> `relieve.rs`, `autoajuste.rs`; cambios marcados con `[lod]` en `stream.rs`, `lod.rs`,
> `session.rs`, `bench.rs`, `save.rs`, `app.rs`, `menu.rs`, `mesher.rs`, `render/mod.rs`, `agua.rs`
> y `world/mod.rs` (`terrain.rs` no se tocó: las cotas del relieve se arman con `height` y
> `forest_density`, que ya existían).
>
> **Advertencia sobre las medidas.** Las del mod salen casi todas de Xvfb con llvmpipe (render por
> software) y de perfiles JFR; las del motor, de lavapipe. Los porcentajes de "ganancia esperada"
> de este documento son **estimaciones** razonadas a partir de esas medidas, no medidas en este
> motor. Cada fase define cómo medirlas con `motor --bench` antes de darlas por buenas.

## 0. Resumen

El motor ya tiene, y en varios casos mejor que el mod, la mitad "de GPU" de un LOD moderno: caras
compactas de 20 bytes con *vertex pulling*, grupos por dirección, megabúfer, dibujo indirecto con
descarte en GPU, Hi-Z en dos pasadas, cascadas en caché, curvatura y niebla. El mod es más fuerte
en la otra mitad: **cómo se reduce el mundo al bajar de nivel, cómo se reparte el trabajo de CPU
en el tiempo y cómo se adapta la calidad al equipo**. Las cinco técnicas de más impacto esperado:

| # | Técnica del mod | Dónde pega en el motor | Impacto esperado |
|---|---|---|---|
| 1 | Auto-ajuste por cuello de botella (CPU/GPU) con perillas en orden, consciente del vsync, + presets con recomendación por hardware y calibración (`BalanceadorCpuGpu`, `QualityPreset`, `CalibradorBenchmark`) | `app.rs`, `save.rs` (`Settings`), `render/mod.rs`, `menu.rs` | El juego **se adapta solo** del iGPU a la placa dedicada; hoy todo es fijo (11 niveles, 4×2048², 160 k vértices por cuadro). Es la técnica que más FPS mínimos asegura en equipos flojos. |
| 2 | Horizonte y oclusión por relieve **antes de generar** (`OclusionRelieve`, horizonte real `sqrt(2Rh)`, `DynamicHorizonRadius`) | `stream.rs` (cola), `lod.rs`, `terrain.rs` | A nivel de calle o rasante, **−25 a −50 % de columnas generadas** (estimado: el Hi-Z ya mide 62 % de caras tapadas en "Calle" pero se generan igual); 0 % desde arriba. |
| 3 | Reducción jerárquica con prefiltrado: superficie a la altura real (`SuperVoxel.relleno`), alturas promediadas, material por voto, cobertura en vez de "cualquier sólido gana" (`HierarchicalReducer`) | `stream.rs` (`build_column_inner`), `terrain.rs`, `store.rs` (`apply_coarse`), `mesher.rs`, `voxel.wgsl` | **Calidad lejana**: sin costas en acantilado (reemplaza el parche `far_drop`), relieve que no "salta" al recentrar los anillos, edificios lejanos sin engordar. Costo de GPU ~0, +5–15 % de caras en niveles gruesos. |
| 4 | Caché de columnas en RAM con límite real de bytes (`BoundedRegionCache`, 0.26.6 / 0.26.37) | `stream.rs` | Zoom de ida y vuelta (el gesto más común de un constructor de ciudades) **sin regenerar**: −70 a −90 % de columnas rehechas en un bucle de zoom; ~15 MB por vista completa en caché. |
| 5 | Fundido con tramado entre niveles en una pasada aparte, sin `discard` en la principal (`FundidoNiveles`, 0.25.0 + 0.26.23) + niebla en zonas sin datos (`NieblaSectores`) | `render/mod.rs`, `cull.wgsl`, `voxel.wgsl`, `water.wgsl` | **Sin saltos visibles** al cambiar de nivel (hoy el cambio es instantáneo), con costo de GPU acotado a las columnas que se están fundiendo (+1–3 % estimado). |

Le siguen: prioridad por lo que se ve + presupuesto de subida por bytes y tiempo + hilos con menos
prioridad (§2.6), el tamaño del anillo como perilla de error en pantalla dependiente de la
resolución (§2.4), SSAO de radio creciente para lo lejano (§2.13) y escalado de resolución (§2.16).

El plan (§3) empieza **después** de que entren los cuatro trabajos en curso (agua/terreno/calles
como vóxeles, época 2, mapa del país, personajes y modelos de detalle) y está partido en fases que
se pueden unir y probar por separado.

---

## 1. Inventario del mod: qué está hecho y qué solo diseñado

Rutas relativas a `src/main/java/com/example/minecraftlodmod/` salvo los shaders
(`src/main/resources/assets/minecraftlodmod/shaders/`). "Arq. §N" es la sección de
`arquitectura-minecraft-lod-mod.md`.

### 1.1 Datos, reducción y mallado

| Técnica | Clases / archivos | Versión / sección | Estado |
|---|---|---|---|
| Supervóxel de 8 bytes: color 3 B, `alturaLocal` (= **relleno**, fracción llena del alto) 1 B, material 1 B, flags 1 B (homogéneo, nevado, luz de bloque 2 bits, luz horneada 4 bits), estado de bloque 2 B | `core/SuperVoxel` (record inmutable) | Arq. §5, §30; luz de bloque 0.24.0 | Implementado |
| Octree por región con nodos vacío / homogéneo / mixto | `core/OctreeNode` | Arq. §2 | Implementado como contenedor; **el recorrido jerárquico del árbol para elegir nivel nunca se hizo** (el render elige un nivel por celda, `render/PlanCeldas`: "el selector jerárquico sobre el octree completo queda para Pista B") |
| Selector por error en pantalla `error_px = tamaño / distancia × alto / (2·tan(FOV/2))`, reacciona al FOV (zoom) | `core/LodSelector`, usado por `render/PlanCeldas.nivelPara` | Arq. §3 | Implementado (por celda) |
| Piso y techo de píxeles (`pixelesMinimos` = 2 px, `pixelesMaximos`) | `PlanCeldas.nivelPara` | 0.26.22 (−38 % vértices, GPU −31 % con umbral 1,0), Arq. §30, §36 | Implementado |
| "Detalle extra": la perilla de auto-ajuste multiplica umbral y piso (1 → 0,5) cuando sobra rendimiento | `core/BalanceadorCpuGpu.detalleExtra` | 0.26.25, Arq. §37 | Implementado |
| Reducción jerárquica 2×2×2 en O(n): visible con ≥ 4 hijos visibles, o relleno ≥ ¼, o ≥ 2 de 4 columnas vistas desde arriba; relleno = altura media de lo sólido por columna; material por voto; color, luz y estado de la **superficie vista desde arriba**; color promediado **solo entre vóxeles del mismo estado** (bloque representativo, idea de Voxy); el aire no aporta color | `generation/HierarchicalReducer` | Arq. §2, §25.2, §30; bug del aire negro en NOTES | Implementado |
| Colapso por homogeneidad desde cierto nivel según preset | `QualityPreset` (columna "colapso desde"), `SectionExtractor` | Arq. §2, §8 | Implementado |
| Greedy meshing (Lysenko) que une caras de mismo material, color y **misma AO en las 4 esquinas** | `generation/GreedyMesher` | Arq. §4, §25.5 | Implementado |
| AO por vértice **solo en caras de arriba** (+12 % guardados contra +42 % en todas); en costados, opción apagada (+18 % GPU) | `GreedyMesher`, opción `oclusionCostados` | Arq. §25.5, 0.26.25 | Implementado |
| Superficie a la altura real: la cara de arriba se dibuja a la altura del relleno, los costados llegan hasta ahí y contra un vecino más bajo se dibuja el tramo entre las dos alturas; solo se unen caras con el mismo recorte | `GreedyMesher`, `GeometriaLod` | Arq. §30 | Implementado |
| Luz suavizada por esquina (como el *smooth lighting* de vanilla) | `generation/VertexLightSampler` | Arq. §4 | Implementado |
| Culling de caras con los vecinos al mallar; bordes entre niveles con "firma de vecinas" para rearmar | `GreedyMesher.Vecinos`, `EstadoCelda` | Arq. §25.4, §34 | Implementado |
| Generación aproximada por funciones de densidad para lo nunca visitado (estilo FarPlaneTwo) | `generation/GeneradorAproximado`, `TerrenoAproximado`, `NivelesGrandes` (niveles 5–10) | Arq. §25.7, §30; 0.25.0 | Implementado |
| Cobertura vegetal: la decoración que no se malla tiñe el vóxel de abajo con su color × fracción opaca | `LectorSeccionMinecraft.voxel`, `PaletaTexturas` | 0.26.28 | Implementado |
| Vóxeles grandes con "franja": pasto solo en la fila de arriba del costado de un vóxel grande, tierra debajo | shader `lod_textura`, `ColorTextura.tieneFranja` | Arq. §29, 0.19.0 | Implementado |
| Búfer plano de supervóxeles fuera del montículo (8 B contiguos) | `core/PackedVoxelBuffer` | Arq. §16 | Implementado pero **no lo usa nadie** |

### 1.2 Almacenamiento

| Técnica | Clases | Versión | Estado |
|---|---|---|---|
| RLE por nodo (supervóxel + largo, 10 B por corrida) | `storage/RunLengthCodec` | Arq. §5 | Implementado |
| Nodos con paleta (≥ 512 vóxeles y ≥ 16 corridas): −24 a −31 % en disco, lectura 34 → 20 µs/nodo | `storage/OctreeNodeCodec` | 0.26.34, Arq. §41 | Implementado |
| Deflate por nodo (no por archivo) para conservar la lectura parcial | `storage/CompresionNodos` | NOTES §5 resuelto | Implementado |
| Archivo de región con tabla de offsets (lectura parcial: una lectura posicional por nodo), datos *append-only*, índice con diario + CRC, compactación por generación, *write-behind* cada 2–5 s | `storage/RegionFileStore`, `RegionHeader` | 0.25.5, Arq. §5 | Implementado |
| Caché LRU con **límite real de bytes**, contando 48 B de costo fijo por entrada (sin eso ocupaba ~3× su presupuesto) | `storage/BoundedRegionCache` | 0.26.6, Arq. §24 | Implementado |
| Caché partida en 16 segmentos con candado propio (esperas > 10 ms: 60 → 9, peor 0,47 → 0,02 s) | `RegionFileStore` | 0.26.37 | Implementado |
| Archivos de lectura abiertos una vez, lecturas sin el candado de escritura, menos basura | `RegionFileStore` | 0.25.4 | Implementado |
| Presupuesto de memoria derivado del preset (caché + tope de tareas) | `config/PresupuestoMemoria` | Arq. §24 | Implementado |

### 1.3 Hilos y planificación

| Técnica | Clases | Versión | Estado |
|---|---|---|---|
| *Work-stealing* (`ForkJoinPool`) + semáforo de concurrencia ajustable en caliente + *backpressure* (`maxTareasEnCola`) | `generation/GenerationTaskScheduler` | Arq. §23–24, 0.26.36 (`reducePermits`) | Implementado |
| Prioridad por vista: costo = distancia² × factor por ángulo con la mirada (vista, margen, detrás) para mallas, extracción y pregeneración | `generation/PrioridadVista` | 0.26.18 (vista completa 80 s+ → 37–39 s) | Implementado |
| Hilos de generación con menos prioridad que el render (llega al SO en Windows) | `GenerationTaskScheduler` | 0.25.4 | Implementado |
| Subidas a la GPU con tope **de bytes** por cuadro (4 MB), no solo de cantidad | `render/RenderLod` | 0.25.4 | Implementado |
| La malla vieja se dibuja hasta que las que la tapan están listas (tope 3 s) | `render/RenderLod`, `FundidoNiveles` | 0.25.0 | Implementado |
| Plan de celdas en un hilo aparte | — | NOTES, "mejoras notadas" | Solo anotado |

### 1.4 Render y shaders

| Técnica | Clases / shaders | Versión | Estado |
|---|---|---|---|
| Buffers por celda (no por nodo) y por dirección de cara; se omiten los grupos de espaldas | `render/GeometriaLod`, `RenderLod` | Arq. §6, §25.3 | Implementado |
| Vértice compacto de 12 B con sprite por tabla (`texelFetch`) | `GeometriaLod.escribirCompacto`, `PaletaTexturas` | Arq. §25.1 | Implementado |
| Descarte por campo de visión con 4 planos (sin near/far), caja con la **altura real de la malla** | `render/CampoVision` | 0.26.11 (2,5 → 0,86 M vértices), 0.26.38 (mirando al cielo −60 % vértices) | Implementado |
| Oclusión por relieve ("horizon culling"): horizonte por dirección con columnas de 32×32 bloques, conservador en las dos puntas; no se arma ni se dibuja lo tapado | `render/OclusionRelieve` | 0.26.37 (9,0 → 4,3 ms por plan) | Implementado |
| Fundido entre niveles: Bayer 4×4 complementario, 0,4 s, hasta 32 mallas salientes; **pasada aparte** para que la principal no tenga `discard` (early-Z) | `render/FundidoNiveles`, `lod_textura_fundido.fsh` | 0.25.0, 0.26.23 | Implementado |
| Niebla en zonas sin datos: 64 sectores, la niebla termina donde empieza lo que falta; recalculada cada 100 ms | `render/NieblaSectores`, `lod_acabado.fsh` | 0.25.2, 0.26.38 | Implementado |
| SSAO del LOD: 8 muestras, ángulo áureo, **radio = máx(1,5, 0,025 × distancia)**, a media resolución con desenfoque 4×4 que respeta bordes de profundidad | `render/AcabadoLod`, `lod_ssao.fsh`, `lod_acabado.fsh` | 0.23.0 | Implementado |
| Neblina atmosférica exp2 + niebla de borde en la misma pasada | `render/AtmosphericPerspective` (lógica), `lod_acabado.fsh` | Arq. §17, 0.23.0 | Implementado (en shader) |
| Curvatura `(d − d0)² / 2R` y radio de horizonte real `sqrt(2Rh) + sqrt(2R·32)` | `core/HorizonteCurvo` | 0.19.0, Arq. §29 | Implementado |
| Radio dinámico según la altura sobre el terreno (35 % abajo, curva de saturación hacia el techo) | `core/DynamicHorizonRadius` | Arq. §19 | Implementado pero **sin conectar** (nadie lo usa; lo reemplazó en la práctica el horizonte real de §29) |
| Texturas horneadas de modelos no cúbicos, siluetas en cruz con recorte | `render/AtlasLod`, `lod_textura_recorte.fsh` | 0.22.0, 0.26.29 | Implementado |
| Nubes lejanas con cobertura media cuando el texel es menor que un píxel | `render/NubesLejanas` | 0.19.0 | Implementado |
| Agua translúcida separada, al final, escribiendo profundidad | `GeometriaLod.separarAgua` | 0.26.26 | Implementado |
| Escalado FSR 1 (EASU + RCAS), temporal propio (Halton, YCoCg), XeSS y DLSS; "apagar solo si pierde más de 3 %" | `render/Escalado*`, `PruebaEscalado`, `fsr_easu.fsh`, `fsr_rcas.fsh`, `escalado_temporal.fsh` | 0.12.0, 0.21.0, 0.26.27 | FSR1 y temporal implementados; XeSS/DLSS implementados **sin probar en hardware** |
| Backend de render de alto rendimiento (estilo Distant Horizons) | `render/RenderBackend` (solo la interfaz) | Arq. §20 | **Solo diseñado** |
| Hi-Z, recorrido en GPU y *multidraw* indirecto (Voxy) | — | Arq. §25 "NO adoptado" | Descartado por compatibilidad |
| Transvoxel / factores de reducción no potencia de 2 | — | Arq. §6, §30 | Descartados |

### 1.5 Calidad, medición y ajuste

| Técnica | Clases | Versión | Estado |
|---|---|---|---|
| Presets Mínimo / Bajo / Medio / Alto / Ultra / Horizonte (radio, umbral px, hilos, caché, colapso, FPS objetivo) | `config/QualityPreset`, `ParametrosCalidad` | Arq. §8, §22 | Implementado |
| Recomendación por hardware (núcleos + RAM; no detecta GPU dedicada) | `QualityPreset.recomendarPorHardware` | Arq. §8, §14 | Implementado |
| Calibración: mundo de semilla fija, 6 puntos (llanura, bosque, montaña, cueva, vista alta, vuelo), espera hasta que el LOD está listo, mide 10 s; pasa si promedio ≤ 90 % del objetivo y 1 % peor ≤ 2,5 × objetivo; escalones = presets + 2 intermedios; **sin vsync ni tope de FPS** | `benchmark/CalibradorBenchmark`, `PuntosBenchmark`, `RecomendacionesBenchmark`, `InformeBenchmark` | 0.26.27, 0.26.38 | Implementado |
| Auto-ajuste de dos perillas (umbral, radio) | `core/PerformanceAutoTuner`, `generation/ControlDeRendimiento` | Arq. §7, §21, §23 | Implementado pero **nunca conectado** (Arq. §28) |
| Auto-ajuste por cuello de botella: GPU medida con *timer query*; orden GPU: agrupado ↓, oclusión ↑, escala ↓, detalle ↓, radio ↓; CPU: generación ↓, agrupado ↑, oclusión ↓, detalle, radio; tirones con buen promedio: generación ↓; recupera tras 2 ciclos < 80 % | `core/BalanceadorCpuGpu`, `render/BalanceCpuGpu` | 0.16.0, Arq. §28 | Implementado y conectado |
| Vsync / tope de FPS: el objetivo nunca es menor que el intervalo; en el tope cuenta como cumplido y se recupera por **carga** de a un paso, con espera exponencial ante fallas (8 → 30 → 240 ciclos); en el menú no ajusta | `BalanceCpuGpu.topeMs` | 0.26.38, Arq. §44 | Implementado |
| HUD FPS / MIN / AVG, CPU, RAM, GPU, VRAM; log de depuración por segundo con eventos y tirones | `render/MonitorRendimiento`, `ColoresHud`, `EstadisticaFrames` | 0.9.0, 0.26.12 | Implementado |

### 1.6 Específico de Minecraft (no se trae)

Red y rebanadas con huellas (`network/`, 0.26.33), VulkanMod integrado, contrato de shaders de Voxy
e Iris, *cubic chunks* (`cubico/`), chunks en RAM y pregeneración del servidor, exclusión exacta
del área vanilla, entidades tapadas por rayos (`OcultamientoEntidades`), `TablaEstados`, atajos del
`Beardifier` y del acuífero. Lo único reutilizable de ese grupo es la **idea** de probar en un hilo
aparte con rayos si una entidad está tapada, que el Hi-Z del motor ya cubre mejor para lo que dibuja
(y que queda pendiente para vecinos y flora, §17 de motor-voxel.md).

---

## 2. Técnica por técnica: qué hay en el motor, qué falta y cómo hacerlo

Cada ficha: **Mod** · **Motor hoy** · **Brecha** · **Ganancia esperada** · **Costo y riesgo** ·
**Pasos**. Las referencias de código del motor son de `engine/src/` en el commit `db47905`.

### 2.1 Superficie a la altura real en los niveles gruesos ("relleno")

- **Mod:** `SuperVoxel.relleno` + `GreedyMesher` (Arq. §30). El tamaño del vóxel solo se nota a lo
  ancho; a lo alto, la superficie queda a resolución de bloque en todos los niveles.
- **Motor hoy:** el vóxel es 1 byte de material. La altura se redondea a vóxeles enteros
  (`stream::top_from_height`); en los niveles ≥ 8 se usa el parche `lod::far_drop` (bajar la malla
  `vóxel − 3 m`) + `top_from_height_far` para que las costas no sean acantilados de 64–256 m. En
  niveles intermedios (4–32 m) una ladera suave sigue siendo una escalera de escalones de 4–32 m, que
  la normal suave del shader disimula pero el contorno no.
- **Brecha:** no hay fracción de vóxel. El salto de forma entre el nivel N y el N+1 es de hasta un
  vóxel entero de alto: es la principal fuente de "pop" al recentrar anillos.
- **Ganancia:** calidad. Siluetas de cerros y costas iguales en todos los niveles; desaparece la
  necesidad de `far_drop`. Rendimiento: neutro en GPU; el *greedy* une menos caras de arriba
  (solo iguales en recorte): **+5 a +15 % de caras en niveles ≥ 3** (estimado; en el mod el costo
  fue aceptable).
- **Costo y riesgo:** medio-alto. Toca el formato de `Quad` (hay 6 bits libres en `a` y 6 en `b`),
  `emit_quad`, `voxel.wgsl::corner` y el pase de sombras. Riesgo estético: la decisión del proyecto es
  "vóxeles a la vista"; por eso **solo en niveles cuyo vóxel mida < ~2 px** (desde el nivel 3 o 4
  con el anillo actual), nunca en 0–2. Riesgo de grietas en costados: hay que dibujar el tramo
  contra el vecino más bajo (como el mod).
- **Pasos:**
  1. `stream.rs::build_column_inner`: junto con `top[k]`, guardar `fill[k] = clamp(h/vs − top_idx, 0, 1)`
     cuantizado a 4 bits (1/16 de vóxel) en los niveles con relleno; en el resto, 15 (lleno).
  2. `mesher.rs`: `mesh_chunk_grad` recibe `fill: Option<&[u8]>` como hoy `grad`. La clave del
     *greedy* de caras de arriba de terreno incluye el relleno; los costados cuyo borde de arriba es
     la superficie llevan el relleno de su columna y los que dan a un vecino más bajo extienden el
     borde de abajo hasta el relleno del vecino.
  3. `Quad`: 4 bits de "recorte de arriba" en `a` (bits 26–29) y 4 de "extensión de abajo" en `b`
     (26–29). `voxel.wgsl::corner` resta `(15 − r)/16 × vóxel` a las esquinas de arriba (y suma a
     las de abajo); `vs_shadow` usa la misma función.
  4. Quitar `far_drop`/`top_from_height_far` cuando las capturas "lejos" y "Cerro rasante" queden
     iguales o mejores (mantener el test de la costa: la tierra 3 m sobre el agua).
  5. Tests: expansión de caras con relleno igual a una malla de referencia; ninguna grieta (rayo
     vertical por cada columna del borde da sólido).

### 2.2 Prefiltrado del terreno al bajar de nivel (alturas y material)

- **Mod:** `HierarchicalReducer`: el nivel N+1 sale del N (O(n)); relleno = **altura media** de las
  columnas; material **por voto**; color/estado de la superficie vista desde arriba, promediado solo
  entre iguales.
- **Motor hoy:** el terreno es una función y se **muestrea en un punto**: `t.height_with_coast(x, z)`
  en el centro de cada vóxel del nivel (`build_column_inner`, bucle de `hs`) y
  `t.surface_material(x, z, h, slope)` en el mismo punto. En el nivel 7 (32 m) un vóxel promedia
  1024 m² con una sola muestra, que incluye el ruido de detalle (`n_detail`, restingas, médanos,
  cauces de 10 m). Al moverse los anillos la muestra cae en otro punto: **aliasing temporal**
  (cerros que suben y bajan un vóxel, arroyos que aparecen y desaparecen, manchas de material).
- **Brecha:** falta filtrar antes de muestrear (o reducir desde lo fino como el mod).
- **Ganancia:** calidad y estabilidad lejana; el material por voto evita "salpicado" de arena/roca.
  Rendimiento: más muestras en niveles gruesos, que son pocas columnas (los niveles 7–10 suman
  ~1700 de 4896 columnas, pero con 1–2 chunks cada una); con el atajo de la grilla del mapa real,
  costo casi nulo.
- **Costo y riesgo:** medio. `terrain.rs` lo está tocando el trabajo del mapa del país: **hacerlo
  después** y en funciones nuevas (`height_filtered(x, z, ancho)`), sin cambiar las existentes.
- **Pasos:**
  1. `terrain.rs`: `height_filtered(x, z, w)`: para `w` ≥ paso de la grilla real (100 m), leer una
     **pirámide de promedios** de la grilla (una vez al cargar el mapa, como `amplio.bin`); para
     `w` menor, quitar del ruido las octavas con longitud de onda < 2·w (*band-limit*) y promediar
     2×2 sub-muestras en los detalles que no son ruido (cauces, costa).
  2. `surface_material`: en niveles ≥ 3, voto entre 4 sub-muestras (2×2), desempate determinista
     por posición (mismo criterio que `store::tiebreak`).
  3. `stream.rs`: usar `height_filtered(x, z, vs)` cuando `vs ≥ 2 m`; mantener el muestreo puntual
     en 0–2 (detalle de cerca y el empedrado alineado).
  4. Test: la altura filtrada de un vóxel grueso es el promedio (± tolerancia) de las finas que
     contiene; determinismo entre corridas.

### 2.3 Reducción de las ediciones por cobertura (no "cualquier sólido gana")

- **Mod:** `HierarchicalReducer`, regla de visibilidad paso a paso (2×2×2): visible si ≥ 4 de 8 hijos
  visibles, o relleno ≥ ¼, o cubre ≥ 2 de 4 columnas desde arriba. Así una pared fina sobrevive
  (cubre 2 de 4 en cada paso) y un poste suelto desaparece.
- **Motor hoy:** `store.rs::apply_coarse` reduce **directo** de lo fino a cada nivel y el ganador
  de cada vóxel grueso es el sólido más alto: **un solo vóxel fino sólido llena el vóxel grueso
  entero**. Una farola de 0,25 × 0,25 × 4,5 m es, en el nivel 4, un cubo de 4 × 4 × 4 m; rejas,
  pretiles y aleros engordan de la misma manera. Desde lejos la ciudad se ve más maciza ("manchas",
  §8 de motor-voxel.md) y hay más caras de las necesarias.
- **Brecha:** falta un criterio de cobertura y hacerlo por pasos.
- **Ganancia:** calidad (siluetas de la ciudad lejana) y **−5 a −15 % de caras en niveles 2–6 con
  ciudad** (estimado; depende de cuánta obra fina haya).
- **Costo y riesgo:** medio. Riesgo de huecos (techos sin paredes, puentes que se cortan) y de
  romper el determinismo ya logrado (§11.7). Conflicto con época 2 (estructuras nuevas): medir
  después.
- **Pasos:**
  1. `apply_coarse` → reducción encadenada: la carga se lleva al nivel 1 con la regla del mod
     (contando sólidos y columnas cubiertas por celda 2×2×2), el 2 desde el 1, etc. El techo sigue
     ganando el material (desde arriba se ve el techo).
  2. Materiales "finos" (rejas, postes, faroles; lista en `palette.rs`) cuentan medio a efectos de
     cobertura; paredes, techos, suelo, completos.
  3. Tests: una pared de 0,5 m se ve en todos los niveles hasta 4 m; un poste aislado desaparece
     desde el nivel 2; misma imagen en dos corridas; `bench` "Ciudad de cerca", "Península" y
     "Lejos" con diferencia de imagen solo en niveles ≥ 2.

### 2.4 El error en pantalla como perilla: tamaño del anillo según preset y resolución

- **Mod:** umbral px por preset (6 → 0,75), piso 2 px y techo (`pixelesMaximos`), perilla del
  auto-ajuste; con 1080p y umbral bajo, el piso ahorró 38 % de vértices sin cambio visible (0.26.22).
- **Motor hoy:** anillos fijos de `RING_CHUNKS = 24` (`lod.rs`). Cada anillo va de 192 a 384 vóxeles
  de su nivel: a 720p y FOV 50° un vóxel mide **~4 px en el borde de adentro y ~2 px en el de
  afuera**; a 1440p, 8 → 4 px; a 4K, 12 → 6 px. `choose_first_level` (0,45 / 0,55 px) solo decide el
  primer nivel, según la altura de la cámara.
- **Brecha:** la calidad depende de la resolución (a más resolución, vóxeles más grandes en pantalla)
  y no hay forma de cambiar detalle por rendimiento sin quitar niveles enteros.
- **Ganancia:** perilla continua para presets y auto-ajuste. `RING_CHUNKS` 16 → ~−45 % columnas
  (vóxeles de 3–6 px a 720p); 32 → +78 % columnas (1,5–3 px). En 1440p, 32 mantiene la imagen que
  hoy se ve a 720p.
- **Costo y riesgo:** medio. `RING_CHUNKS` es `const` y lo usan los tests de cobertura (el último
  anillo llega a 85 km con 24; con 16 llega a ~57 km: el preset bajo tiene que aceptarlo o sumar un
  nivel). Debe ser par con mitad par: 16, 20, 24, 28, 32.
- **Pasos:**
  1. `lod.rs`: `LevelSpec { lod, ring }` y `level_columns_from` con el anillo de cada spec; tests
     parametrizados por anillo (un punto en exactamente un nivel; alcance mínimo según preset).
  2. `stream.rs::Streamer::new` / `update_view`: anillo según preset y `view_h`
     (`ring = clamp(24 × (view_h/720)^0,5 × factor_preset)` redondeado a par con mitad par).
  3. Cambiar de anillo en caliente = recentrar con otro anillo (el retiro "solo cuando está
     cubierto" ya evita huecos).

### 2.5 Fundido entre niveles con tramado

- **Mod:** `FundidoNiveles` (0.25.0): la malla vieja se dibuja hasta que las que la tapan están
  listas, después se cruzan con Bayer 4×4 complementario en 0,4 s, hasta 32 a la vez. 0.26.23: el
  tramado va en **otra pasada** solo para las mallas que entran o salen; la principal sin `discard`
  (early-Z).
- **Motor hoy:** `Streamer::poll` + `covered()` ya hacen la primera mitad (la retirada se quita
  recién cuando las nuevas están subidas, §17 "huecos de terreno"). El cambio es instantáneo:
  la columna nueva aparece y la vieja se borra en el mismo cuadro.
- **Brecha:** el cruce.
- **Ganancia:** calidad (desaparece el salto al cambiar de nivel, que con el relleno §2.1 ya será
  chico). Costo: solo las columnas en transición (unas decenas) se dibujan dos veces en una pasada
  con `discard`: **+1 a +3 % de GPU** estimado.
- **Costo y riesgo:** medio. Hay que integrarlo con el descarte en GPU y el Hi-Z (las columnas que
  se funden no deben entrar a la pirámide de la primera pasada, porque tienen huecos), y con MSAA el
  tramado puede usar la máscara de muestras (*alpha-to-coverage*) en vez de `discard`.
- **Pasos:**
  1. `stream.rs`: `Op::Remove` pasa a `Op::FadeOut(key)` y `Op::Set` lleva `fade_in: bool` (columna
     que reemplaza a otra de otro nivel).
  2. `render/mod.rs`: `ColGpu` ya tiene `lo.w` (nivel) y `hi.w` (activa); usar `hi.w` como
     `0 = inactiva, 1 = estable, 2 = entra, 3 = sale` y un búfer de `t0` por *slot*. Las salientes se
     liberan a los 0,4 s (`free_later` ya difiere 2 cuadros).
  3. `cull.wgsl`: las columnas en transición escriben sus argumentos en un **segundo rango**
     (`args_fade`), no en el principal ni en la fase 1 del Hi-Z.
  4. Pipeline `voxel_fade` = mismo `vs_main` + `fs_fade` que hace `discard` con Bayer 4×4 (o
     `@builtin(sample_mask)` con MSAA) según `(t − t0)/0,4` y si entra o sale. Se dibuja después de
     la segunda pasada del Hi-Z. Sombras: sin fundido (se dibuja la estable).
  5. Opción "Fundido entre niveles" (prendida desde el preset Medio).

### 2.6 Prioridad por lo que se ve, presupuesto de subida por bytes y tiempo, hilos con menos prioridad

- **Mod:** `PrioridadVista` (0.26.18: distancia² × factor por ángulo); subida con tope de 4 MB por
  cuadro (0.25.4); generación con menos prioridad que el render (0.25.4).
- **Motor hoy:** `Streamer::prio` = distancia al centro + nivel (no mira hacia dónde apunta la
  cámara orbital); `poll(160_000)` limita por **vértices** (= 40 000 caras, ~0,8 MB) desde
  `app.rs::frame`, sin mirar el tiempo; los hilos de `worker` tienen la prioridad normal del SO y en
  4 núcleos compiten con el hilo principal (3 hilos de trabajo + render). Además
  `refresh_detail` rearma el búfer **entero** de la flora cuando cambia cualquier columna.
- **Brecha:** lo que está fuera del cuadro compite con lo que está adentro; el presupuesto no se
  adapta a cuánto tarda realmente subir.
- **Ganancia:** tiempo hasta "vista completa" tras teletransporte, giro o zoom **−30 a −50 %**
  (estimado por el 0.26.18 del mod); menos cuadros lentos por "subir mallas" (se ven en
  `perfil`); menos tirones en 4 núcleos con los hilos debajo del render.
- **Costo y riesgo:** bajo. La cola ya se rehace en `requeue_all`; solo cambia la clave.
- **Pasos:**
  1. `Streamer::update_view` recibe la dirección de la vista (de `OrbitCamera`): `prio` =
     `dist × (1 + k × (1 − cos θ))` con θ entre la mirada horizontal y el centro de la columna, más el
     nivel; lo de adentro del volumen de visión con k chico, lo de atrás con k grande. Las columnas
     urgentes (ediciones) siguen en −1.
  2. `requeue_all` solo si la mirada giró más de ~15° o el centro cambió (hoy solo por centro).
  3. `poll(budget)`: presupuesto doble — caras (como hoy) **y** milisegundos medidos de
     `set_column` en el cuadro anterior (por ejemplo 2 ms con vsync a 60 Hz); el auto-ajuste (§2.7)
     lo mueve.
  4. Hilos: bajar la prioridad de los `worker` (crate `thread-priority` o `libc::setpriority` en
     Linux y `SetThreadPriority` en Windows), detrás de una opción para medir.
  5. Flora: subir por columna en el megabúfer o en un búfer con asignador (coordinar con el trabajo
     de personajes y modelos de detalle, que crea un *pipeline* instanciado nuevo: puede que ya lo
     resuelva).

### 2.7 Auto-ajuste por cuello de botella, consciente del vsync

- **Mod:** `BalanceadorCpuGpu` (0.16.0, Arq. §28, §44): una perilla por segundo; GPU medida con
  *timer query*; orden distinto si manda la GPU o la CPU; recupera con margen; **con vsync el
  objetivo nunca es menor que el intervalo y en el tope se recupera por carga, de a un paso, con
  espera creciente**; no ajusta en el menú. Lección de 0.26.38: sin eso, con vsync a 60 Hz bajaba
  todo al piso o nunca recuperaba.
- **Motor hoy:** nada se ajusta solo. `app.rs` mide el cuadro por partes (simulación, subir mallas,
  selección, interfaz, dibujo) y lo imprime con `perfil`; `TIMESTAMP_QUERY` solo en el banco
  (§12 pendiente: "*timestamps* asíncronos en la ventana"). `Settings.sincronizacion_vertical` es
  `true` por omisión (la misma trampa del mod). Además la ventana **no dibuja** si `frame_key` no
  cambió: un medidor ingenuo vería "FPS bajos" con la cámara quieta.
- **Brecha:** todo.
- **Ganancia:** FPS mínimos estables en todo el rango de equipos; en iGPU, la diferencia entre
  jugable y no. Es la técnica de mayor impacto **si** los presets (§2.8) existen primero.
- **Costo y riesgo:** medio. El riesgo es oscilar: el mod lo resolvió con histéresis, una perilla por
  ciclo, bloqueo de `detalleExtra` 30 ciclos y espera exponencial.
- **Pasos:**
  1. `render/mod.rs`: *timestamps* del pase principal y de sombras en la ventana, leídos de forma
     asíncrona un cuadro (o dos) tarde con `map_async`; `FrameStats.gpu_ms`.
  2. Módulo nuevo `autoajuste.rs` (lógica pura, con tests como los del mod): entrada = muestras de
     (cuadro, CPU de trabajo, GPU, intervalo de vsync, ¿se dibujó?); ignora cuadros no dibujados
     (`frame_key`) y el menú; ciclo de 1 s.
  3. Perillas del motor, en orden — GPU al límite: escala de render (§2.16, si existe) → cascadas
     (4→3→2) y resolución de sombras → anillo (§2.4) → niveles (11→10→9). CPU al límite:
     concurrencia de los `worker` (permiso tipo semáforo, como `GenerationTaskScheduler`) →
     presupuesto de subida → anillo → niveles. Tirones con buen promedio: concurrencia ↓.
     Recuperar en orden inverso, con "detalle extra" (anillo +4) al final y bloqueado tras fallar.
  4. Vsync: `tope_ms = 1000 / refresco` (de `winit` `MonitorHandle::refresh_rate_millihertz`); en el
     tope, recuperar por carga (GPU y CPU de trabajo sin la espera de `present`).
  5. Opción "Auto-ajuste" (prendida por omisión, se congela durante la calibración).

### 2.8 Presets, recomendación por hardware y calibración

- **Mod:** `QualityPreset` (6 presets), `recomendarPorHardware` (núcleos + RAM; **no** podía saber si
  hay GPU dedicada), `CalibradorBenchmark` (0.26.27/0.26.38).
- **Motor hoy:** `Settings` tiene niveles (9/10/11), MSAA (0 = automático por adaptador CPU/GPU),
  sombras (cascadas × resolución), vsync. `DrawMode` y Hi-Z se eligen solos según el adaptador.
  `motor --bench` es una herramienta de desarrollo, no algo que el jugador corre.
- **Brecha:** no hay "calidad" de una sola perilla ni recomendación; el motor **sí** puede saber más
  que el mod: `wgpu::AdapterInfo.device_type` (`Cpu`, `IntegratedGpu`, `DiscreteGpu`), el
  fabricante y los límites.
- **Ganancia:** buena primera impresión en cualquier equipo; base para el auto-ajuste.
- **Costo y riesgo:** bajo-medio. Conflicto con el trabajo del mapa del país (menú) y con `save.rs`.
- **Pasos:**
  1. `save.rs`: `Settings.calidad: Preset { Minimo, Bajo, Medio, Alto, Ultra, Personalizado }` con
     `#[serde(default)]` (opciones viejas = Personalizado con sus valores, sin cambiar nada).
     Tabla sugerida (a ajustar con medidas en GPU real):

     | | Mínimo | Bajo | Medio | Alto | Ultra |
     |---|---|---|---|---|---|
     | Niveles | 9 | 9 | 10 | 11 | 11 |
     | Anillo (chunks) | 16 | 20 | 24 | 24 | 32 |
     | Sombras | 2 × 1024² | 2 × 2048² | 3 × 2048² | 4 × 2048² | 4 × 4096² |
     | MSAA | 1× | 1× | 4× | 4× | 4× |
     | Fundido / SSAO lejano | no / no | sí / no | sí / no | sí / sí | sí / sí |
     | Hilos de trabajo | 1 | núcleos − 2 | núcleos − 1 | núcleos − 1 | núcleos − 1 |
     | Subida por cuadro | 1 ms | 1,5 ms | 2 ms | 3 ms | 4 ms |
     | FPS objetivo | 30 | 30 | 45 | 60 | 60 |

  2. Recomendación: `Cpu` → Mínimo; `IntegratedGpu` → Bajo (Medio con ≥ 8 núcleos lógicos);
     `DiscreteGpu` → Alto (Ultra con ≥ 8 GB de límite de búfer / VRAM reportada y ≥ 6 núcleos).
     Primera vez que se abre el juego: se aplica y se avisa.
  3. "Medir y ajustar" en Opciones: corre 3 vistas del banco (ciudad de cerca, península, calle)
     **sin vsync**, por escalones (preset y dos intermedios), 5 s cada una después de quedar
     ocioso el *streamer*; pasa si promedio ≤ 90 % del objetivo y 1 % peor ≤ 2,5 × objetivo. Guarda
     "Personalizado". Informe en la carpeta de datos (como `InformeBenchmark`).

### 2.9 Horizonte y oclusión por relieve **antes de generar**

- **Mod:** `OclusionRelieve` (horizonte por dirección con columnas 32×32 de nivel 5, conservador en
  las dos puntas, nada cerca ni bajo tierra; no se arma ni se dibuja lo tapado) y el horizonte real
  `sqrt(2Rh) + sqrt(2R·H)` de `HorizonteCurvo`.
- **Motor hoy:** el Hi-Z (§17) descarta en GPU lo tapado **ya generado y subido**; en la vista
  "Calle" tapa el 62 % de las caras, en "Miguelete" el 10 %, desde arriba ~0. La generación y el
  mallado (lo más caro del *streaming*, §11) se hacen igual para todo. La curvatura se aplica en el
  shader (`curved`) y en la caja de descarte, pero un cerro detrás del horizonte se genera igual.
- **Brecha:** saber antes de generar qué columnas no se van a ver desde esta altura.
- **Ganancia:** con la cámara baja (calle, rasante, `Miguelete`): **−25 a −50 % de columnas
  generadas** en total y bastante más en los niveles 7–10 (a 30 m de altura el horizonte geométrico
  está a ~20 km: el agua y la llanura de más allá de ~28 km quedan bajo el horizonte salvo los
  cerros). Desde arriba (península, lejos): 0. Sin cambio de imagen si es conservador.
- **Costo y riesgo:** medio. Riesgo de huecos al bajar o subir rápido la cámara (la prueba
  `nunca_quedan_huecos_al_hacer_zoom_o_viajar_rapido` tiene que seguir pasando) y de **sombras
  perdidas**: una columna tapada para la cámara puede tirar sombra sobre algo visible. Por eso: en
  los niveles con sombra (`SHADOW_MAX_LOD`, ≤ 4 m, más los de cascadas lejanas) lo tapado **se
  posterga** (al final de la cola) en vez de saltearse; en los niveles sin sombra, se saltea.
- **Pasos:**
  1. `terrain.rs`: `max_height_in(x0, z0, x1, z1)` con una **pirámide de máximos** de la grilla del
     mapa real y del entorno amplio (+ margen del ruido de detalle y de la altura máxima de obras de
     `LevelStore::max_y`); se arma una vez al cargar el mapa.
  2. `stream.rs`: antes de encolar, prueba de horizonte con curvatura: oculta si
     `d_cerca > sqrt(2R·h_ojo) + sqrt(2R·H_max)`; después, oclusión por relieve como el mod
     (sectores angulares, de cerca hacia afuera, elevación como tangente con la bajada d²/2R, sin
     oclusores a menos de ~200 m y nunca con la cámara bajo el terreno).
  3. Re-evaluar al cambiar la altura de la cámara más de un 20 % o el centro (no por cuadro): es la
     misma ocasión en que hoy se llama a `update_with`.
  4. Las ocultas no se cuentan como faltantes en `covered()` (para que el retiro no espere algo que
     nunca se genera) pero sí como "no quiero" para el plano de niebla (§2.12).

### 2.10 Radio dinámico según la altura

- **Mod:** `DynamicHorizonRadius` (35 % abajo, saturación arriba) — diseñado pero desconectado; en la
  práctica lo reemplazó el horizonte real (§29 del mod).
- **Motor hoy:** siempre `Settings.niveles` (11 por omisión) sin importar la altura.
- **Brecha / decisión:** no hace falta otra clase: con §2.9, un nivel cuyas columnas están todas
  ocultas simplemente no se genera. Lo que sí conviene es **no subir ni recorrer** esos niveles
  (432 columnas cada uno de los niveles 9 y 10): se lo trata como "niveles activos" =
  `min(niveles, nivel donde empieza lo oculto + 1)`.
- **Ganancia:** pequeña en GPU, moderada en CPU de recentrado (`level_columns_from`,
  `requeue_all`) con la cámara baja. Va dentro de la fase de §2.9.

### 2.11 Caché de columnas en RAM con límite real de bytes

- **Mod:** `BoundedRegionCache` (LRU por bytes **con costo fijo por entrada**: sin él ocupaba 3× lo
  previsto), segmentada en 16 (0.26.37). Disco como fuente de verdad (`RegionFileStore`).
- **Motor hoy:** al retirar una columna (`Op::Remove`) su malla se descarta; si la cámara vuelve
  (zoom de ida y vuelta, que además cambia `first` por `choose_first_level`, o un paneo de ida y
  vuelta) se genera y se malla de nuevo. El terreno es una función y las ediciones están en
  `EditStore`, así que **no hace falta disco**: alcanza con RAM.
- **Brecha:** la caché.
- **Ganancia:** en un bucle de zoom 150 m ↔ 3 km o un paneo de ida y vuelta, **−70 a −90 % de
  columnas rehechas** y "listo" casi inmediato. Memoria: una columna promedia ~180 caras × 20 B ≈
  3,6 KB (1,45 M triángulos / 4032 columnas en "Ciudad de cerca"); una vista completa ≈ 15 MB, así que
  con 64–128 MB entran varias vistas.
- **Costo y riesgo:** bajo-medio. Lo delicado es la **validez**: una malla depende de (a) las
  ediciones de su columna y vecinas, (b) qué vecinas están en el anillo (los faldones del borde) y
  (c) la versión del terreno (mareas/agua como vóxeles: ver conflicto con ese trabajo).
- **Pasos:**
  1. `store.rs`: contador de versión por columna de chunks y nivel (`LevelStore::col_version`),
     que sube en `apply` para las columnas tocadas (ya se calculan en `touched`).
  2. `stream.rs`: clave de caché = `(nivel, columna, firma de las 8 vecinas en el anillo, versión
     de la columna y sus vecinas, versión del terreno)`; valor = `Mesh` (con `detail`).
     `Op::Remove` deposita la malla; antes de encolar un trabajo se busca en la caché.
  3. LRU por bytes **contando** `size_of::<Mesh>()` + capacidad de los 6 `Vec<Quad>` + `detail` +
     costo de la entrada del mapa (la lección del 0.26.6). Presupuesto por preset (32 → 256 MB).
  4. Una sola caché con `Mutex` alcanza (la usa el hilo principal); si la usan los `worker`,
     segmentarla (lección del 0.26.37).
  5. Nuevo test: zoom de ida y vuelta sin ediciones = 0 columnas generadas la segunda vez; con una
     edición en el medio, solo las tocadas.
- **Disco (archivo de región con tabla de offsets, RLE/paleta, *append-only*):** evaluado y **no
  recomendado por ahora**. Las partidas regeneran los vóxeles desde el estado de la ciudad
  (`save.rs`), el terreno es una función y la generación por columna (~0,2–1 ms) es más barata que
  leer y descomprimir. Tendría sentido solo si el mapa del país hace la generación del entorno mucho
  más cara (por ejemplo, datos reales con muchas capas): ahí se aplicaría a **alturas y materiales
  filtrados por nivel** (§2.2), no a mallas.

### 2.12 Niebla en las zonas sin datos

- **Mod:** `NieblaSectores` (64 sectores; la niebla termina donde empieza lo que falta; se recalcula
  cada 100 ms, no por cuadro — 0.26.38).
- **Motor hoy:** `reach_fog` funde el borde del último anillo con la niebla; al entrar a una partida,
  teletransportarse o hacer zoom rápido hacia afuera, lo que todavía no se generó se ve como **el
  plano del río** (los huecos del §17 eran eso).
- **Brecha:** la niebla no sabe qué falta.
- **Ganancia:** calidad percibida durante el *streaming* (nada de "río donde debería haber tierra");
  costo: una textura de 64×1 y una lectura por fragmento.
- **Costo y riesgo:** bajo-medio. Toca `voxel.wgsl` y `water.wgsl` (otros dos trabajos en curso).
- **Pasos:**
  1. `stream.rs`: cada 100 ms o al recentrar, para 64 sectores alrededor del centro, la distancia a
     la primera columna querida que no está subida (`versions` vs `ready`), recorriendo por nivel.
  2. `render/mod.rs`: textura `R16Float` 64×1 en el grupo de `Globals`.
  3. `voxel.wgsl`/`water.wgsl`: `fog = max(fog, smoothstep(alcance − margen, alcance, dist_xz))`
     por sector, como `lod_acabado.fsh`, junto a `reach_fog`.

### 2.13 SSAO de radio creciente con la distancia

- **Mod:** `lod_ssao.fsh` + `lod_acabado.fsh` (0.23.0): 8 muestras, radio `máx(1,5, 0,025·d)`, a
  media resolución, desenfoque 4×4 que respeta la profundidad. Oscurece valles y pies de cerro de
  lejos y rincones de cerca; el AO por vértice queda para el detalle.
- **Motor hoy:** AO por vértice (`face_ao`) que el shader **apaga a lo lejos en el terreno** a
  propósito (`ao_min` con `far_t`) para no rayar laderas. Lejos no hay ninguna oclusión: los valles,
  la base de la ciudad contra el suelo y los montes contra la llanura se ven planos.
- **Ganancia:** calidad lejana y de media distancia (lectura del relieve). Costo GPU: una pasada a
  media resolución (~5–8 % a 1080p en iGPU, estimado; en placa dedicada, poco).
- **Costo y riesgo:** medio. Con MSAA la profundidad hoy se descarta (`StoreOp::Discard`) cuando no
  hay Hi-Z; hay que guardarla o reusar el nivel 0 de la pirámide del Hi-Z (media resolución, pero es
  el **mínimo** de 2×2: sirve para AO lejano, no para el cercano). Va en el preset Alto/Ultra.
- **Pasos:**
  1. Pasada de cómputo `ssao.wgsl` sobre la profundidad (o el nivel 0 del Hi-Z) a media resolución,
     solo para distancias > ~150 m (de cerca manda el AO por vértice); radio en metros
     `máx(2, 0,025·d)`.
  2. Desenfoque bilateral 4×4 y aplicación en `voxel.wgsl` como multiplicador de la luz del cielo
     (no de la del sol), igual que hoy `ao`.

### 2.14 Menos caras en niveles gruesos: AO fuera de la clave del *greedy*

- **Mod:** AO solo en caras de arriba (+12 % contra +42 %); oclusión en costados apagada por
  omisión porque "corta fusiones de caras" (+18 % GPU).
- **Motor hoy:** `face_ao` en las 6 direcciones y la clave del *greedy* incluye la AO
  (`masks[...] = m | ao << 8`), en todos los niveles. En el terreno lejano el shader ya **ignora** la
  AO (`ao_min = mix(0.9, 1.0, far_t)`): se pagan caras que no se ven.
- **Ganancia:** **−10 a −25 % de caras en los niveles ≥ 4** (estimado por la diferencia del mod
  entre AO en todas las caras y solo arriba) con imagen casi igual.
- **Costo y riesgo:** bajo. Un parámetro `ao: bool` en `mesh_chunk_grad`, falso para terreno desde
  el nivel cuyo vóxel mide ≤ ~1,5 px en el borde de adentro del anillo (con el anillo actual,
  nivel ≥ 4; para obra, ≥ 5). Medir diferencia de imagen en "Península" y "Lejos".

### 2.15 Cobertura vegetal y franja de pasto en vóxeles grandes

- **Mod:** la decoración no mallada tiñe el suelo (0.26.28); el costado de un vóxel grande de pasto
  lleva el pasto solo arriba (Arq. §29).
- **Motor hoy:** la flora baja existe hasta `PLANT_MAX_VOXEL` (0,5 m) y después "la pradera se ve por
  el color del suelo" (`detail.wgsl`), pero el suelo no toma el color de la flora que dejó de
  dibujarse; con `soil = ceil(2/vs)` = 1 vóxel desde 2 m, el costado de un escalón de pasto de 4–32 m
  es verde de arriba abajo.
- **Ganancia:** calidad chica pero barata: sin cambio de tono en el borde del nivel de la flora;
  barrancas que se leen como tierra.
- **Pasos:** en `voxel.wgsl`, (a) para pastos en niveles sin flora, mezclar hacia el color medio de
  la flora de ese material con un peso de cobertura por material (tabla en la paleta); (b) en
  costados de terreno con vóxel ≥ 2 m, pasto solo en el último ~0,3 m (`fract(world.y / vs)`), tierra
  debajo. Conflicto: el trabajo de terreno/agua y el de época 2 tocan `voxel.wgsl`.

### 2.16 Escalado de resolución (FSR 1) con "apagar solo si pierde"

- **Mod:** EASU + RCAS (0.21.0), "apagar solo si pierde más de 3 %" (`PruebaEscalado`, 0.26.27),
  temporal propio; recomendado por la calibración cuando la GPU está al ≥ 85 % del cuadro.
- **Motor hoy:** no hay; MSAA 1×/4× es la única perilla de resolución.
- **Ganancia:** en iGPU limitada por fragmentos, 0,67× de escala ≈ **−40 a −55 % de trabajo de
  fragmentos**; perilla continua para el auto-ajuste. La interfaz (egui) queda a resolución
  completa.
- **Costo y riesgo:** medio. Reescribir EASU/RCAS en WGSL (FSR 1 es MIT). Interacción con MSAA
  (resolver antes de escalar) y con el Hi-Z (la pirámide sale del tamaño de render). El temporal
  (jitter + vectores de movimiento) queda fuera de este plan: es el 80 % del trabajo según el propio
  mod.
- **Pasos:** destino de render del tamaño escalado, pasada EASU a tamaño completo, RCAS, después
  la interfaz; opción "Escala de render" (100/85/75/67 %) y perilla del auto-ajuste.

### 2.17 Lo que el motor ya tiene (equivalente o mejor)

| Mod | Motor | Comentario |
|---|---|---|
| Vértice compacto 12 B | `mesher::Quad` de 20 B **por cara** con *vertex pulling* (≈ 5 B por vértice) | Mejor |
| Buffers por celda y dirección | Megabúfer con *buddy allocator* (`render/alloc.rs`) y grupos por dirección | Mejor |
| Descarte por campo de visión con la altura real | `cull.wgsl` + `box_visible` con la caja de la malla y la curvatura | Igual |
| — (descartado por compatibilidad) | Hi-Z en dos pasadas, dibujo indirecto múltiple | Mejor |
| Greedy + culling con vecinos | Máscaras de bits por fila + *greedy*; bordes leídos del anillo | Igual o mejor |
| *Backpressure* del scheduler | `sync_channel(MAX_READY = 96)` + versiones por trabajo | Igual |
| Malla vieja hasta que la nueva está | `covered()` en `Streamer::poll` | Igual (falta el cruce, §2.5) |
| Curvatura + perspectiva atmosférica | `curved()`, `fog_color` hacia el sol, `reach_fog` | Igual |
| Clave de sombra | Cascadas en caché por caja de luz, a lo sumo 2 por cuadro | Mejor (el mod no tiene sombras propias) |
| "Rough generation" | El terreno **es** una función | No aplica |
| Error en pantalla para el primer nivel | `choose_first_level` (0,45/0,55 px con histéresis) | Igual (falta la perilla por anillo, §2.4) |

### 2.18 Banco: vuelo, percentiles y "salto al recentrar"

- **Mod:** calibración con 6 puntos incluido un **vuelo** a 20 b/s; métricas promedio, 1 % peor,
  peor; espera a que el LOD esté listo (2 muestras sin mallas en cola); informes comparables.
- **Motor hoy:** `bench.rs`: vistas quietas (5 con ciudad + 3 del mapa real), N cuadros fuera de
  pantalla, barrido de 10 cuadros con y sin Hi-Z, cascadas en movimiento. No mide el *streaming*
  **mientras** la cámara se mueve con el presupuesto de la ventana, ni percentiles.
- **Brecha:** sin esto no se pueden medir las fases 1, 2 y 4 de §3.
- **Pasos (fase 0):**
  1. Vista "Vuelo": la cámara recorre una trayectoria fija (ciudad → península → cerro, 3 km a
     40 m/s y un zoom 150 m → 3 km → 150 m) con `apply_ops` y el presupuesto de la ventana por
     cuadro. Métricas: cuadro p50/p95/p99 y máximo (trabajo de CPU), máximo de "subir mallas",
     columnas generadas, pendientes al final y segundos hasta quedar ocioso.
  2. Métrica "salto al recentrar": con la cámara fija, generar con el centro de los anillos en A y
     en A + un chunk del nivel 6 (512 m), esperar ocioso y comparar imágenes (% de píxeles > 8).
     Mide el *pop* que atacan §2.1, §2.2 y §2.5.
  3. Columna "Columnas generadas" por vista (ya está `CHUNKS`; sumar columnas) y "ocultas por
     horizonte" (§2.9).
  4. Salida en `bench.md` **y** `bench.json` para comparar con un script (`tools/`).

---

## 3. Plan de implementación por fases

### 3.1 Antes de empezar: los cuatro trabajos en curso

> **Actualización 2026-11-23: los cuatro ya entraron** (v0.10.0 y v0.10.1: `engine/src/agua.rs`,
> la época 2, `engine/src/pais/`, `engine/src/detalle/`). Lo que eso cambió del plan:
> - **Agua en vóxeles:** las columnas llevan el agua (`agua::mallar`); el nivel del río lo pone el
>   shader y los arroyos tienen nivel fijo por mapa, así que la marea **no** entra en la firma de
>   la caché (§2.11 paso 2: la "versión del terreno" es constante por `Streamer`). Para la oclusión,
>   lo dibujado de una columna con agua llega hasta 3 m (la marea).
> - **Mapa del país:** vive en `pais/` y su propio modo de pantalla; no tocó el menú de Opciones,
>   así que los presets (Fase 5) entraron en `menu.rs` sin conflicto.
> - **Personajes:** el pipeline instanciado nuevo (`detalle/`, `set_agents`) **no** reemplazó el
>   búfer de la flora: `Renderer::refresh_detail` sigue rearmando la flora entera cuando cambia una
>   columna (§2.6 paso 5, pendiente).
> - **Otro agente sigue trabajando en flora y clasificación del terreno** (`terrain.rs`,
>   `trees.rs`, `pais/`, `tools/`): por eso la Fase 2 no agregó la pirámide en `terrain.rs`, sino
>   en `relieve.rs`, muestreando las funciones públicas que ya había.

El plan arrancaba **después** de que se unan a `ccr-889e1313-dztkuu` estos trabajos, porque tocan los
mismos archivos:

| Trabajo en curso | Archivos que toca (según su encargo) | Fases de este plan que chocan |
|---|---|---|
| Agua, terreno y calles como vóxeles | `stream.rs` (`build_column_inner`, capas, superficie), `water.wgsl`, `render/mod.rs` (agua), `terrain.rs` | 1 (cola y caché en `stream.rs`), 2 (`stream.rs`), 3 (todo: relleno y prefiltrado en `build_column_inner`, `mesher.rs`), 4 (`water.wgsl` para la niebla por sectores) |
| Época 2 (estructuras, luces de noche en `voxel.wgsl`) | `structures/`, `buildings.rs`, `voxel.wgsl` | 3 (`store::apply_coarse` reduce sus estructuras; `voxel.wgsl::corner` para el relleno), 4 (`voxel.wgsl`), 6 (SSAO y franja en `voxel.wgsl`) |
| Mapa del país | `terrain.rs`, `session.rs`, `menu.rs` | 2 (pirámide de máximos en `terrain.rs`), 3 (`height_filtered`), 5 (opciones y presets en `menu.rs`/`save.rs`, `session.rs::apply_ops`) |
| Personajes y modelos de detalle (pipeline instanciado nuevo) | `city/agentes/`, `render/mod.rs` (`encode`, *pipelines*, búfer de flora), `detail.wgsl`, `agentes.wgsl` | 1 (subida de la flora), 4 (`render/mod.rs::encode` para el pase de fundido), 5 (perillas que tocan sus distancias) |

Regla para todas las fases: **rebasar sobre lo unido, volver a correr `motor --bench` como base y
recién ahí cambiar**. Cada fase es una rama propia, con tests, una tabla antes/después en
`motor-voxel.md` (nueva sección §18 en adelante) y capturas, como §11–§17.

Medidas: `cargo run --release -- --bench salida/ --frames 5` (y `--draw gpu` donde haga falta el
Hi-Z). Con lavapipe, alternar antes/después y correr dos veces (varía ±15–20 %). Lo que se pueda
medir en placa real con `medir-fps.bat`, medirlo.

### Fase 0 — Banco ampliado (base para medir todo lo demás)

- **Qué:** §2.18 (vuelo, percentiles, salto al recentrar, `bench.json`).
- **Archivos:** `bench.rs` (y un script en `tools/`). Conflicto mínimo (solo agrega vistas).
- **Criterio de éxito:** el banco corre en lavapipe en < 3 min más que hoy; dos corridas seguidas dan
  las mismas columnas generadas y la misma imagen (diferencia 0); el informe trae p50/p95/p99 del
  vuelo y el "salto al recentrar" de las vistas Península y Lejos.

### Fase 1 — *Streaming* sin tirones y sin trabajo repetido (sin cambio de imagen)

- **Qué:** §2.6 (prioridad por vista, presupuesto por caras y milisegundos, prioridad de hilos) y
  §2.11 (caché de columnas en RAM).
- **Archivos:** `stream.rs`, `store.rs` (versión por columna), `session.rs`/`app.rs` (presupuesto),
  `render/mod.rs` (medir `set_column`).
- **Conflictos:** agua/terreno como vóxeles (`stream.rs`); la versión del terreno de la clave de caché
  tiene que incluir lo que ese trabajo haga variar (marea, nivel del agua).
- **Criterio de éxito (`--bench`):**
  - Vuelo: cuadro p99 **−30 %** o mejor y máximo de "subir mallas" ≤ 4 ms (lavapipe).
  - Zoom de ida y vuelta: columnas generadas la segunda vez **≤ 20 %** de la primera.
  - Tiempo hasta "vista completa" tras teletransporte en la vista Calle: **−30 %**.
  - Imagen idéntica en todas las vistas quietas (diferencia 0) y test nuevo de la caché (con
    edición en el medio solo se rehacen las tocadas). `nunca_quedan_huecos_al_hacer_zoom_o_viajar_rapido`
    sigue en verde.

### Fase 2 — Horizonte y relieve antes de generar

- **Qué:** §2.9 y §2.10.
- **Archivos:** `terrain.rs` (pirámide de máximos, función nueva), `stream.rs` (filtro antes de
  encolar), `lod.rs` (niveles activos).
- **Conflictos:** mapa del país (`terrain.rs`): solo funciones nuevas; agua como vóxeles (`stream.rs`).
- **Criterio de éxito:**
  - Columnas generadas: **−25 % o más** en "Calle", "Cerro rasante" y "Miguelete"; 0 % de cambio en
    "Península" y "Lejos".
  - Diferencia de imagen ≤ 0,05 % en todas las vistas y 0 en el barrido de cámara del Hi-Z (bajando
    y subiendo la cámara además de girarla).
  - Sombras: las cascadas de "Calle" y "Ciudad de cerca" iguales (las columnas tapadas con sombra se
    postergan, no se saltean).

### Fase 3 — Calidad lejana: reducción con prefiltrado, relleno y cobertura

- **Qué:** §2.2 (prefiltrado), §2.14 (AO fuera del *greedy* en lo grueso), §2.3 (cobertura de
  ediciones) y §2.1 (relleno), **en ese orden y en sub-ramas separadas** (cada una se puede unir
  sola).
- **Archivos:** `terrain.rs`, `stream.rs`, `mesher.rs`, `store.rs`, `voxel.wgsl` (`corner`),
  `render/mod.rs` (pase de sombras).
- **Conflictos:** los cuatro trabajos (es la fase más expuesta); empezar cuando estén todos unidos.
- **Criterio de éxito:**
  - "Salto al recentrar" (Fase 0): **−50 %** de píxeles cambiados en Península y Lejos.
  - Caras en niveles ≥ 4: **−10 %** o más (§2.14, §2.3) compensando el +5–15 % del relleno; total de
    caras de la vista Lejos ≤ hoy.
  - Costas: el test de la costa de `far_drop` reescrito para el relleno (tierra 0–3 m sobre el agua,
    fondo debajo) y `far_drop` eliminado.
  - Tests nuevos: altura filtrada = promedio de las finas; pared fina visible en todos los niveles
    hasta 4 m; poste aislado desaparece desde el nivel 2; sin grietas en el borde del relleno;
    determinismo (dos corridas, diferencia 0).
  - Revisión visual con capturas antes/después de Península, Lejos y Cerro (la estética de vóxeles
    de cerca, niveles 0–2, **igual**).

### Fase 4 — Transiciones: fundido y niebla de lo que falta

- **Qué:** §2.5 y §2.12.
- **Archivos:** `stream.rs` (`Op::FadeOut`, sectores), `render/mod.rs` (estado de la columna,
  *pipeline* de fundido, textura de sectores), `cull.wgsl`, `voxel.wgsl`, `water.wgsl`.
- **Conflictos:** personajes (orden de pases en `encode`), época 2 y agua (`voxel.wgsl`,
  `water.wgsl`).
- **Criterio de éxito:**
  - El *pipeline* principal sigue sin `discard` (revisión del WGSL).
  - GPU pase principal (`TIMESTAMP_QUERY` del banco): ≤ +3 % en el vuelo; quieto, 0 (nada se funde).
  - Vuelo: en ningún cuadro se ve el plano del río donde hay tierra (prueba: en cada cuadro del vuelo,
    los píxeles con color de agua por debajo de la cota 0,5 m del mapa = 0, contando el agua real).
  - Diferencia de imagen con la cámara quieta y ociosa: 0.

### Fase 5 — Perillas, presets, recomendación y auto-ajuste

- **Qué:** §2.4 (anillo como perilla), §2.8 (presets, recomendación y "Medir y ajustar") y §2.7
  (auto-ajuste con vsync).
- **Archivos:** `lod.rs`, `stream.rs`, `save.rs` (`Settings`), `menu.rs`, `app.rs`, `render/mod.rs`
  (*timestamps* asíncronos), módulo nuevo `autoajuste.rs`.
- **Conflictos:** mapa del país (`menu.rs`, `session.rs`); personajes (sus distancias de dibujo
  pueden sumarse como perilla).
- **Criterio de éxito:**
  - `motor --bench --calidad minimo|bajo|medio|alto|ultra` (opción nueva): las columnas y caras bajan
    de forma monótona de Ultra a Mínimo; Mínimo **≤ 50 %** de las caras de Alto en Península.
  - Tests de `autoajuste.rs` con series sintéticas: con vsync a 60 Hz y objetivo 60, no baja nada si la
    carga es < 80 %; con GPU al 100 % baja primero sombras y después anillo; recupera de a un paso y
    no oscila (≤ 1 cambio cada 8 ciclos en régimen); ignora cuadros no dibujados y el menú.
  - En lavapipe (adaptador CPU) la recomendación da Mínimo; con placa real, `medir-fps.bat` registra
    el preset recomendado y su FPS.
  - Opciones viejas (`opciones.json` sin `calidad`) cargan como Personalizado sin cambios.

### Fase 6 — Acabado: SSAO lejano, flora y franja, escalado

- **Qué:** §2.13, §2.15 y §2.16 (este último opcional, detrás del preset y del auto-ajuste).
- **Archivos:** `render/mod.rs`, shaders nuevos (`ssao.wgsl`, `escalado.wgsl`), `voxel.wgsl`.
- **Conflictos:** época 2 (`voxel.wgsl`), personajes (orden de pases), interfaz (egui a resolución
  completa).
- **Criterio de éxito:**
  - SSAO: GPU pase principal + SSAO ≤ +8 % a 1280×720 en lavapipe (medir también en placa); capturas
    de Cerro y Península con los valles marcados; de cerca (vista Calle) diferencia < 1 % (manda el AO
    por vértice).
  - Escalado al 67 %: GPU pase principal **−35 %** o mejor; la prueba "apagar solo si pierde" lo
    desactiva en el adaptador CPU si no gana más de 3 %.

### 3.2 Orden y dependencias

```
Fase 0 ──► Fase 1 ──► Fase 2
   │          │
   │          └──────► Fase 5 (usa el presupuesto de Fase 1 como perilla)
   └──► Fase 3 ──► Fase 4 ──► Fase 6
```

Fases 1 y 3 pueden ir en paralelo (archivos distintos salvo `stream.rs`: 1 toca la cola, 3 toca
`build_column_inner`). Fase 5 no depende de 3 ni de 4, pero sus presets incluyen las opciones de
fundido y SSAO: si llegan antes, esas filas quedan en "no".

### 3.3 Riesgos generales

- **Lavapipe no es una placa.** Las ganancias de GPU (fundido, SSAO, escalado, Hi-Z) solo se pueden
  confirmar en hardware real; las de CPU (fases 1–3) sí se ven acá. Mantener `medir-fps.bat` al día
  con las métricas nuevas.
- **Estética de vóxeles a la vista.** Todo lo que suaviza (relleno, prefiltrado, SSAO) se aplica solo
  donde el vóxel mide pocos píxeles. Los niveles 0–2 no cambian.
- **Determinismo.** El motor logró imagen idéntica entre corridas (§11.7); la caché (fase 1), la
  cobertura (fase 3) y el horizonte (fase 2) tienen que conservarlo: test de dos corridas en cada
  fase.
- **Auto-ajuste que oscila o castiga de más.** Las dos lecciones del mod (vsync en 0.26.38, cuadros
  que no se dibujan) entran como tests desde el principio.

---

## 4. Referencias

- Mod: `README.md`, `CLAUDE.md`, `arquitectura-minecraft-lod-mod.md` (§2–9, §16–19, §21–25, §28–30,
  §34–37, §41–44), `NOTES.md` ("Mejoras notadas"), `CHANGELOG.md` (0.19.0, 0.21.0, 0.22.0–0.25.6,
  0.26.6, 0.26.11, 0.26.18, 0.26.22–0.26.29, 0.26.34, 0.26.37, 0.26.38).
- Motor: `engine/src/lod.rs`, `stream.rs`, `store.rs`, `mesher.rs`, `terrain.rs`, `render/mod.rs`,
  `render/hiz.rs`, `render/shaders/{voxel,cull,hiz,sombras,water,detail}.wgsl`, `app.rs`,
  `session.rs`, `bench.rs`, `save.rs`; [motor-voxel.md](motor-voxel.md) §4, §10–12, §16, §17.
- Licencias: del mod (del mismo autor) se toman ideas y, si conviene, código propio. Lo que el mod
  tomó de **Voxy** (todos los derechos reservados) es solo idea; **FarPlaneTwo** y **FSR 1** son MIT
  (crédito en el archivo que los adapte).

---

## 5. Resultados de las fases 0, 1, 2 y 5 (2026-11-23)

> **Medido con render por software** (lavapipe/llvmpipe, Vulkan, 4 núcleos, 3 hilos de trabajo,
> 1280×720, MSAA 4×). Valen las proporciones y los conteos (columnas, caras, diferencias de
> imagen), **no** los milisegundos absolutos: con lavapipe la "GPU" es la CPU. Durante las
> corridas había otro agente compilando en la misma máquina: los tiempos de pared varían
> ±15–50 % entre corridas; por eso las comparaciones de tiempo que siguen son de corridas
> **seguidas** (`motor --bench` del commit de la Fase 0 sin cambios del streamer, y enseguida
> el del final) o del **mismo binario** con las funciones apagadas por variables de entorno
> (`MOTOR_SIN_CACHE=1 MOTOR_SIN_OCLUSION=1 MOTOR_HILOS_NORMALES=1`).

### 5.1 Qué quedó

| Fase | Técnica del mod | En el motor |
|---|---|---|
| 0 | Calibración con vuelo, percentiles e informes comparables | `bench.rs`: tabla de **streaming** (vuelo de 84 s con zoom 150 m ↔ 3 km y 3 km a 40 m/s con el presupuesto de la ventana a 60 Hz; p50/p95/p99, 1 % peor, peor cuadro, peor subida, columnas generadas, de la caché, ocultas, bytes subidos, pendientes, hasta ocioso), **zoom de ida y vuelta** (dos vueltas), **teletransporte** Lejos → Calle (hasta lo visible listo y hasta todo listo), **salto al recentrar** (Península y Lejos), columna "Generadas (ocultas)" por vista y `bench.json`. `MOTOR_BANCO=streaming` corre solo la parte de streaming y `MOTOR_BANCO_VISTAS=peninsula,calle` solo esas vistas. |
| 1 | `BoundedRegionCache` (0.26.6) | `cache.rs`: LRU por bytes **contando el costo fijo por entrada** (96 B), clave (columna, firma); la firma es (vecinas en el anillo, generación de ediciones de la columna y de las vecinas a ≤ 2 columnas). Las mallas no se copian: la ventana devuelve al `Streamer` la malla que acaba de subir (`give_back`) y la caché la saca al reusarla. Presupuesto por preset (32 a 256 MB; 128 por omisión). |
| 1 | `PrioridadVista` (0.26.18) | `Streamer::prio`: distancia × (1 + 1,83·(cos 50° − cos θ)) fuera del cono de la mirada (hasta 4× detrás); la cola se reordena si la mirada gira más de 15°. |
| 1 | Subida con tope de bytes (0.25.4) | `RitmoSubida`: bytes por cuadro = presupuesto en ms / costo por byte medido (media móvil); se suma al tope de 160 000 vértices de antes. |
| 1 | Hilos de generación con menos prioridad (0.25.4) | `setpriority` (+5) por hilo en Linux, `SetThreadPriority(BELOW_NORMAL)` en Windows; `MOTOR_HILOS_NORMALES=1` lo apaga. Límite de concurrencia ajustable en caliente (el semáforo de `GenerationTaskScheduler`). |
| 2 | `OclusionRelieve` + horizonte real | `relieve.rs`: pirámide de cotas (mínimo y máximo) del terreno muestreada una vez por mapa (16 m sobre el mapa y 3 km alrededor, 512 m en el entorno amplio, ±180 km; margen de medio desnivel de la celda + 5/9 m; ~1 s en 4 núcleos, compartida entre partidas) y densidad del monte (64 / 512 m) para las copas de los niveles gruesos. Barrido por 2048 sectores, de cerca hacia afuera, con la tangente y la bajada `d²/2R` de `voxel.wgsl::curved` acotadas en las dos puntas; lo dibujado de una columna se acota con la misma función de la cara de arriba que usa el mallado (`top_from_height`, `top_from_height_far`), árboles (+25 m), copas (+11 m + un vóxel), agua (3 m) y obras (`max_y`). Ojo un 15 % más alto (≥ 3 m), nada a menos de 300 m, solo tapan columnas sin obras a ≥ 150 m, ninguna con el ojo bajo el terreno. Se reevalúa al recentrar o cuando el ojo se mueve un 10 % de su altura; el barrido corre en un hilo aparte. Lo tapado que puede tirar sombra (niveles ≤ 6, a menos de 6 km) se posterga en vez de saltearse. |
| 2 | `DynamicHorizonRadius` | Sin clase aparte (como preveía §2.10): un nivel con todas sus columnas tapadas no se genera. Además, `Streamer::set_active_levels` apaga niveles gruesos en caliente (perilla de la Fase 5). |
| 5 | `QualityPreset` + `recomendarPorHardware` | `autoajuste.rs`: Mínimo…Ultra (tabla de §2.8 con caché por preset); recomendación por `wgpu::DeviceType`, núcleos y RAM (`/proc/meminfo`); en Opciones, fila "Calidad" (★ = recomendada) + "Auto-ajuste" + "FPS objetivo"; la primera vez se aplica la recomendada y se avisa; las opciones viejas quedan en Personalizado ("A mano"). `Settings`: `calidad`, `anillo`, `cache_mb`, `subida_ms`, `hilos`, `fps_objetivo`, `autoajuste`. `motor --bench --calidad minimo…ultra`. |
| 5 | Anillo como perilla (§2.4) | `LevelSpec { lod, ring }` con `lod::RINGS = [16, 20, 24, 28, 32]` y `Streamer::set_ring` (recentra; lo viejo se quita cuando lo nuevo lo cubre). |
| 5 | `BalanceadorCpuGpu` + vsync (0.26.38) | `Autoajuste`: ciclos de 1 s, una perilla por vez; GPU: sombras → anillo → niveles; CPU: hilos → subida → anillo → niveles; tirones: hilos; recupera en orden inverso tras 2 ciclos bajo el 80 %. Objetivo ≥ intervalo del vsync (refresco del monitor de winit); en el tope (≤ 110 % del intervalo) no baja nada y recupera por **carga** (máx. de GPU y trabajo del hilo principal sin la espera de la superficie), de a un paso, prueba de 8 ciclos y espera de 30 → 240 ciclos si pierde el tope. Ignora cuadros no dibujados (`frame_key`), el menú y los recorridos. GPU: marcas de tiempo del pase principal leídas sin esperar (`map_async`, uno o dos cuadros tarde); sin ellas, "manda la GPU" si el hilo principal ocupa menos del 60 % del cuadro. Diagnóstico: segunda línea del contador de FPS ("auto-ajuste: manda la GPU · en el tope, carga 6,1 ms de 16,7 · sombras ↓ (GPU al límite)") y el perfil (`CITYSIM_PERFIL`). |

**No se hizo** (queda anotado): "Medir y ajustar" (calibración desde Opciones, §2.8 paso 3); flora por columna en vez de rearmar el búfer entero (§2.6 paso 5); la pirámide de máximos con la grilla del mapa real en `terrain.rs` (se muestreó `height` en `relieve.rs` para no tocar `terrain.rs`, que está trabajando otro agente).

### 5.2 Vistas quietas: columnas generadas e imagen

Corridas seguidas (`antes`: el banco de la Fase 0 sin los cambios del streamer; `después`: todo).
"Generadas (ocultas)": columnas que salieron de los hilos de trabajo y, entre paréntesis, las que
se dejaron sin generar porque el relieve las tapa. La diferencia de imagen compara las capturas
de cada vista entre las dos corridas (píxeles con diferencia > 8 en algún canal).

| Vista | Columnas generadas antes → después | Ocultas | Generar+mallar (pared) antes → después | CPU de generación antes → después | Caras | Diferencia de imagen |
|---|---|---|---|---|---|---|
| Ciudad de cerca | 4896 → 4737 (−3 %) | 168 | 1,39 → 1,43 s | 3,23 → 3,33 s | 0,80 → 0,78 M | 0,000 % |
| Península | 4032 → 4032 | 0 | 0,92 → 0,99 s | 2,11 → 2,25 s | 0,54 → 0,54 M | 0,000 % |
| Lejos (9 km) | 3168 → 3168 | 0 | 0,73 → 0,74 s | 1,69 → 1,70 s | 0,26 → 0,26 M | 0,000 % |
| Ciudad de noche | 4896 → 4904 | 1 | 1,45 → 1,39 s | 3,37 → 3,26 s | 0,80 → 0,80 M | 0,000 % |
| Calle (nivel del suelo) | 4896 → 4167 (**−15 %**) | 736 | 1,43 → 1,18 s (−17 %) | 3,33 → 2,75 s (−17 %) | 0,81 → 0,73 M | 0,000 % |
| Cerro, rasante | 4896 → 4896 | 0 | 3,43 → 3,36 s | 9,03 → 8,90 s | 2,08 → 2,08 M | 0,000 % (barrido 0,000 %) |
| Miguelete de cerca | 4896 → 3823 (**−22 %**) | 1079 | 4,20 → 3,66 s (−13 %) | 11,26 → 9,87 s (−12 %) | 3,22 → 3,06 M | 0,000 % |
| Cerro | 4464 → 4464 | 0 | 3,11 → 3,18 s | 8,25 → 8,45 s | 1,74 → 1,74 M | 0,000 % |

- **Imagen idéntica** en las 14 capturas del banco (vistas, barridos de cámara del Hi-Z, multitudes
  y las dos del salto al recentrar): las fases 0–2 no cambian el aspecto.
- **Oclusión:** −15 % de columnas en "Calle" y −22 % en "Miguelete"; 0 desde arriba (Península,
  Lejos, Cerro) como se esperaba. En "Cerro, rasante" el ojo está a ~390 m (la cámara orbita a
  2,5 km de la cima): ve por encima de todo, nada queda tapado. Quedó por debajo del −25 % del
  criterio: el margen de los niveles gruesos es grande (la cara de arriba de un vóxel de 256 m
  se redondea, y las copas del monte suman un vóxel entero), y conservador a propósito.
- Las caras dibujadas en "Calle" bajan de 0,81 a 0,73 M en la GPU (lo tapado ya no se sube; el
  Hi-Z igual descartaba el 62 %).
- Sombras: las cascadas de "Calle" y "Ciudad de cerca" quedaron iguales (misma imagen; lo tapado a
  menos de 6 km en niveles con sombra se posterga, no se saltea).

### 5.3 Streaming (Fase 0 antes → después)

| Prueba | Antes | Después |
|---|---|---|
| Zoom 150 m ↔ 3 km, columnas generadas en la 1.ª / 2.ª vuelta | 1256 / 1256 (100 %) | 409 / **0 (0 %)**; de la caché 1060 / 1256 |
| Zoom, tiempo de cada vuelta hasta ocioso | 0,6 s / 0,6 s | 0,3 s / 0,2 s |
| Vuelo: columnas generadas | 72 371 | 64 312 (−11 %; 9847 de la caché, 233 tapadas) |
| Vuelo: cuadro p50 / p95 / p99 | 0,07 / 1,04 / 1,71 ms | 0,10 / 1,61 / 2,62 ms |
| Vuelo: 1 % peor / peor cuadro / peor subida | 2,48 / 8,3 / 8,3 ms | 4,12 / 24,0 / 24,0 ms |
| Vuelo: subido | 168,4 MB | 173,7 MB |
| Teletransporte Lejos → Calle: lo visible / todo listo | 1,06 / 1,06 s | 0,86 / 0,91 s |
| Teletransporte: columnas generadas | 2911 | 2873 |
| Salto al recentrar (Península / Lejos) | 4,36 % / 0,46 % | 4,36 % / 0,46 % (no cambia: es la Fase 3) |
| Oclusión en el vuelo | — | 570 evaluaciones; armar las cajas, peor 3,3 ms en el hilo principal; el barrido, peor 15 ms en su hilo |

A/B del vuelo con el mismo binario (dos pares de corridas alternadas, carga de la máquina ~1–1,5;
las tres últimas columnas apagan una sola cosa, con el barrido de la oclusión todavía en el hilo
principal):

| Vuelo | Todo apagado | Todo prendido | Sin caché | Sin oclusión | Hilos con prioridad normal |
|---|---|---|---|---|---|
| p99 | 1,68 / 1,65 ms | 3,81 / 3,45 ms (barrido en el hilo principal) → 2,33 ms (en su hilo) | 2,88 ms | 2,09 ms | 3,17 ms |
| peor subida | 9,1 / 9,7 ms | 19,9 / 24,3 ms | 8,8 ms | 18,2 ms | 22,6 ms |
| columnas generadas | 72 410 / 72 468 | 62 469 / 62 426 | 72 694 | 62 217 | 62 527 |

Lectura honesta: **el criterio de la Fase 1 para el vuelo no se cumplió** (se pedía p99 −30 % y
peor subida ≤ 4 ms). El zoom de ida y vuelta sí (0 % la segunda vez, criterio ≤ 20 %), y el
teletransporte bajó un 19 % hasta lo visible (criterio −30 %). Lo que subió el p99 del vuelo:
(1) la caché entrega columnas sin esperar a los hilos, así que más cuadros suben el presupuesto
entero (es trabajo que antes esperaba; las columnas llegan antes); (2) la oclusión costaba ~1 ms
del p99 mientras el barrido corría en el hilo principal: se pasó a un hilo aparte. Los picos de
"peor subida" (un solo cuadro de 18–24 ms con ~70 órdenes y ~100 KB) aparecen en `set_column` /
`remove_column` del renderer (asignador del megabúfer y escrituras de lavapipe), no en el
streamer; con un solo cuadro por corrida, el máximo es ruido de lavapipe hasta medirlo en placa.

### 5.4 Presets (`motor --bench --calidad …`, vistas Península y Calle, 3 cuadros)

| Preset | Niveles · anillo · sombras · MSAA | Columnas Península / Calle | Caras Península / Calle | Cuadro Península (sombra en caché) |
|---|---|---|---|---|
| Mínimo | 9 · 16 · 2×1024² · 1× | 1408 / 1757 | 0,33 / 0,42 M | 115 ms |
| Bajo | 9 · 20 · 2×2048² · 1× | 2200 / 2710 | 0,41 / 0,57 M | 142 ms |
| Medio | 10 · 24 · 3×2048² · 4× | 3600 / 4105 | 0,50 / 0,73 M | 239 ms |
| Alto | 11 · 24 · 4×2048² · 4× | 4032 / 4160 | 0,54 / 0,73 M | 262 ms |
| Ultra | 11 · 32 · 4×4096² · 4× | 7168 / 6887 | 0,76 / 1,04 M | 321 ms |

Columnas y caras bajan de forma monótona de Ultra a Mínimo. Mínimo tiene el **61 %** de las caras
de Alto en Península (el criterio pedía ≤ 50 %: con anillo de 16 el nivel 0 sigue teniendo la
misma resolución; bajar más exige vóxeles más grandes de cerca o la Fase 3). En lavapipe la
recomendación da **Mínimo** (adaptador `Cpu`).

### 5.5 Pruebas nuevas

`cache::` (límite de bytes con costo fijo, firmas, LRU), `stream::` (zoom de ida y vuelta sale de
la caché y una edición rehace solo lo tocado; la malla de la caché es igual a la generada; **la
oclusión nunca tapa una columna visible** con el relieve real en cuatro vistas rasantes y una
desde arriba, comprobado con rayos contra el terreno; con oclusión el streamer termina y al subir
genera lo que aparece), `relieve::` (loma, curvatura, cotas conservadoras contra el terreno
muestreado cada 1–32 m en 900 zonas), `lod::` (cualquier anillo cubre todo una sola vez),
`autoajuste::` (con vsync no baja nada; con objetivo mayor que el refresco nunca llega al piso y
recupera de a un paso; espera creciente si pierde el tope; orden GPU y CPU; ignora cuadros no
dibujados y el menú; no oscila; presets y recomendación) y `save::` (opciones viejas quedan en
Personalizado sin cambios; preset aplicado ida y vuelta).

### 5.6 Riesgos

- Nada de esto se midió en una placa real: los presets, el auto-ajuste y el presupuesto de
  subida en ms están calibrados a ciegas. `medir-fps.bat` debería registrar el preset
  recomendado y la línea del auto-ajuste.
- La oclusión es conservadora por construcción y probada con rayos, pero se reevalúa por pasos
  (10 % de la altura del ojo): al orbitar rápido con la cámara rasante, una columna que deja de
  estar tapada tarda lo que tarda en generarse (los niveles gruesos, que son los que se tapan,
  se generan en milisegundos).
- El auto-ajuste no se probó en una ventana real (acá no hay pantalla): sus decisiones están
  cubiertas con series sintéticas.

---

## 6. Tirones del vuelo, fases 3 y 4 (en parte) y calidad de imagen (2026-11-24)

> Medido con lavapipe (4 núcleos, 1280×720, MSAA 4×). La máquina la compartían otros agentes
> compilando: las tablas de tiempo son de corridas con la máquina quieta (carga ~1) o, donde se
> dice, con `chrt -f 1` (el proceso medido por encima de los demás). "Antes" es la v0.11.0
> (commit `68a51e7`) compilada igual. El banco ahora también da la **CPU del hilo principal** por
> cuadro (`CLOCK_THREAD_CPUTIME_ID`), que no cuenta lo que el sistema le quita al hilo.

### 6.1 Por qué tironeaba el vuelo (regresión de v0.11.0)

Medido por partes con la CPU del hilo (`MOTOR_BANCO_DETALLE=1`), no eran el megabúfer ni las
tablas del descarte, sino cuatro cosas:

| Causa | Costo medido | Arreglo |
|---|---|---|
| `covered()` de cada retirada en **cada** cuadro, recorriendo en cada nivel fino todo lo que ese nivel quiere | 4–5 ms por cuadro con ~740 retiradas esperando (zoom y vuelo), 100+ cuadros | Se miran solo si cambió algo de lo que decide la cobertura (versiones, subidas, tapadas) y con un índice de lo que falta proyectado a los niveles gruesos (`missing_index`/`covered_by`; el test compara con `covered`) |
| El mapa de la caché de columnas (entradas de ~250 B) se duplicaba al crecer | 4–17 ms en un solo cuadro, 6 veces por vuelo | Malla en caja (`Box<Mesh>`) y lugar reservado según el presupuesto |
| ~370 000 `queue.write_buffer` en el vuelo (caras, origen, visibilidad y fila de la tabla por columna, más una por retirada), cada uno con su búfer intermedio | 40 % del trabajo del hilo principal (0,54 s de escritura + 0,51 s de envío) | `Subidas`: las caras van directo a un `StagingBelt`; las tablas de columnas y orígenes tienen espejo en la CPU y se suben por tramos al final del cuadro (la visibilidad, que también escribe la GPU, solo en los lugares tocados) |
| Recentrar rearmaba y comparaba los conjuntos de **todos** los niveles, y el cuadro que recentraba subía además el presupuesto entero | 1,5–3 ms en los cuadros del p99 | Los niveles cuyo anillo y hueco no se movieron no se tocan; lo gastado en recentrar y recibir sale del presupuesto de subida del cuadro, y el envío a la GPU del cuadro siguiente entra en el costo por byte (`RitmoSubida::medir_envio`) |

Lo que queda de los cuadros > 1 ms es casi todo el `queue.submit` de las copias (~0,4 ms): con
lavapipe las copias se ejecutan en el mismo hilo al enviar (en una placa son de la GPU).

| Vuelo (84 s), máquina quieta, dos corridas | Fase 0 (§5.3) | v0.11.0 | Ahora |
|---|---|---|---|
| Cuadro p50 / p95 / p99 | 0,07 / 1,04 / 1,71 ms | 0,08 / 1,58–1,66 / **2,40–2,46** ms | 0,11 / 1,25 / **1,77–1,81** ms |
| 1 % peor | 2,48 ms | 3,54–3,75 ms | 2,15–2,51 ms |
| Peor cuadro / peor subida | 8,3 / 8,3 ms | **20,0–20,5** ms | **5,4–5,7 / 4,9–5,4** ms |
| CPU del hilo: p99 / peor / total | — | — | 1,65 / 2,5–5,7 ms / 2,04 s |
| Teletransporte Lejos → Calle: visible / todo / peor cuadro | 1,06 / 1,06 s / — | 0,91–0,96 / 0,96–1,01 s / 8,8–9,4 ms | 0,91–0,94 / 0,94–0,98 s / 4,0–5,7 ms |
| Zoom de ida y vuelta: generadas en la 2.ª vuelta | 100 % | 0 % | 0 % |

Con la máquina cargada (otros agentes compilando, carga 9–14) el tiempo de pared no sirve
(p99 5–6 ms en las dos versiones); con `chrt -f 1`: v0.11.0 p99 2,15 ms y peor 20,2 ms, ahora
p99 1,66–1,73 ms y peor 6,5–10,9 ms.

**Criterios:** peor cuadro ≤ 8 ms y por debajo del de antes: **sí** (20 → 5,4–5,7 ms, y 8,3 en la
Fase 0). p99: −26 % contra la v0.11.0, pero **igual** que la Fase 0 (1,71 ms): el criterio de −30 %
contra ella **no se cumple**. Peor subida ≤ 4 ms: **no** (4,9–5,4 ms). Lo que falta es el envío de
las copias en lavapipe (ver arriba) y el recentrado de los niveles finos; bajar el presupuesto de
subida a 1 ms casi no movió el p99 (1,66 → 1,49 ms con `chrt`), así que no es el presupuesto.

### 6.2 Fase 3 (en parte): prefiltrado y oclusión fuera del *greedy*

- **Altura prefiltrada** (`stream::prefiltra`, `altura_prefiltrada`): desde el vóxel de 2 m (nivel 3)
  la altura de un vóxel es el promedio de sus 4 esquinas; las esquinas se comparten entre vecinos,
  así que cuesta lo mismo que antes (CPU de generación igual: Península 2,35 → 2,54 s, Cerro
  9,5 → 10,2 s, dentro del ruido). Primero se probó 2×2 y 3×3 muestras por vóxel más un **voto del
  material**: la generación costaba 2,5–4 veces más y el voto se quedaba siempre con el pasto más
  común (el campo lejano del Cerro perdía sus manchas verdes, 22 % de píxeles cambiados): se
  descartó. El material sigue saliendo del centro del vóxel. `MOTOR_SIN_PREFILTRO=1` lo apaga
  para medir.
- **Oclusión por vértice fuera de la clave del *greedy*** (`mesher::AoModo`): terreno sin oclusión
  desde el nivel 6 (16 m) y nada desde el 8 (64 m). Se probó desde el nivel 4 como decía §2.14: el
  shader todavía muestra la oclusión en el borde de adentro de cada anillo (vóxeles de ~4 px) y el
  Cerro se veía más claro y plano; desde el 6 la imagen cambia 0,005 %.

| Vista | Caras antes → ahora | Diferencia de imagen |
|---|---|---|
| Ciudad de cerca | 0,78 → 0,75 M (−4 %) | 0,001 % |
| Calle | 0,73 → 0,71 M (−3 %) | 0,000 % |
| Península | 0,54 → 0,51 M (−6 %) | 0,18 % |
| Lejos (9 km) | 0,26 → 0,23 M (−12 %) | 0,17 % |
| Cerro | 1,74 → 1,61 M (−7 %) | 4,5 % (relieve un poco más suave) |
| Cerro, rasante | 2,08 → 1,95 M (−6 %) | 2,2 % |

Los niveles 0–2 quedan iguales (Calle y Ciudad de cerca ≤ 0,001 %). **Criterios:** caras en
niveles ≥ 4 −10 %: en Lejos sí (−12 %), en el resto −6/−7 %. **Salto al recentrar: no bajó**
(Península 4,36 → 4,39 %, Lejos 0,46 → 0,47 %): esa medida la dominan los cambios de los niveles
finos (flora, aspecto vóxel de cerca), que el prefiltrado no toca. Determinismo: dos corridas,
diferencia 0 (los PNG son idénticos byte a byte).

**No se hizo:** cobertura de ediciones (§2.3: hay que releer el nivel 0 de cada celda porque las
cargas se unen de a una, con riesgo de huecos y de romper el determinismo) y relleno (§2.1: toca el
formato de `Quad`, el *greedy*, `corner` y las sombras); por eso `far_drop` sigue.

### 6.3 Fase 4 (en parte): niebla de lo que falta

`Streamer::sectores_faltan` (64 sectores alrededor del centro de los anillos, cada 100 ms o cuando
cambia algo): la distancia a la primera columna querida que no está en la GPU y cuya zona no tapa
ninguna otra dibujada (de otro nivel, aunque esté retirada); las tapadas por el relieve no cuentan.
`Renderer::set_faltan` la sube a un búfer (binding 8) y `voxel.wgsl`/`water.wgsl` funden con la
niebla desde un 20 % antes (`hole_fog`, entre dos sectores manda el más cercano). El plano de
respaldo del río usa `fs_plano` (está más allá del último anillo). Quieto y ocioso no hay nada que
falte: la imagen no cambia (el banco no la usa). Test: al empezar todos los sectores en 0, al
terminar ninguno, al moverse > 0. **No se hizo** el fundido con tramado (§2.5) ni se midió el
criterio "ningún píxel de río donde hay tierra" en el vuelo (el banco no dibuja en el vuelo).

### 6.4 Calidad de imagen y franja de pasto

- **Variación de color a gran escala** del terreno natural (`voxel.wgsl`): zonas de 400 m a 1,3 km
  algo más húmedas o más secas (±6 %).
- **Niebla por altura** (`niebla_altura`, en vóxeles, agua, flora y agentes): densidad ∝ exp(−y/450 m)
  promediada en el rayo, entre ×0,8 y ×1,2 de la de antes; **dispersión ancha del sol** además del
  halo (más con el sol bajo).
- **Curva tonal**: un contraste leve en los medios (sombras ~5 % más hondas, medios ~3 % más claros),
  ahora también en el cielo y el agua para que el horizonte siga empalmando.
- **Franja de pasto** (§2.15 b): desde el vóxel de 2 m el costado del vóxel de pasto es tierra salvo
  sus 30 cm de arriba, **solo en las barrancas** (normal suave empinada): en las pendientes suaves
  dibujaba curvas de nivel marrones (se vio en la vista rasante y se corrigió).

Capturas (arriba antes, abajo después): `docs/capturas/render-rasante-antes-despues.jpg`,
`render-cerro-antes-despues.jpg`, `render-peninsula-antes-despues.jpg`,
`render-noche-antes-despues.jpg`, `render-cerca-antes-despues.jpg`.

### 6.5 Pendiente y riesgos

- ~~Fase 3: cobertura (§2.3) y relleno (§2.1, quitar `far_drop`). Fase 4: fundido (§2.5). Fase 6:
  SSAO lejano (§2.13) y escalado (§2.16).~~ Hechos en la segunda tanda (§7). Queda el tinte de
  la flora en el suelo (§2.15 a).
- Todo medido en lavapipe: el envío de copias que queda en el p99 en una placa es de la GPU.
- La niebla de lo que falta se ve solo en la ventana (el banco no la dibuja): revisarla en una
  placa al teletransportarse y al alejar rápido.

---

## 7. Segunda tanda: fundido, cobertura, relleno, SSAO, escala y sombras de personajes (2026-11-24)

> Medido con lavapipe (4 núcleos, 1280×720, MSAA 4×). Las vistas quietas se midieron con otros
> agentes compilando en la máquina (carga 3–28): sus tiempos de GPU varían ±20–40 % entre
> corridas y **solo valen los conteos y las diferencias de imagen**. El vuelo se midió después,
> con la máquina quieta y `chrt -f 1`, en corridas alternadas (antes = v0.12.0 compilada igual).

### 7.1 Qué quedó

| Técnica (§) | En el motor |
|---|---|
| Fundido entre niveles (§2.5, `FundidoNiveles` + `lod_textura_fundido` del mod) | `Streamer::fundido`: una columna nueva que se superpone con una retirada de otro nivel sale como `Op::Entra`; una retirada ya cubierta, como `Op::Sale` (índice de retiradas proyectadas a los niveles gruesos, llevado al día al retirar y quitar). El renderer las marca en la tabla de columnas (`hi.w`: 1 estable, 2 entra, 3 sale): `cull.wgsl` deja afuera de la vista principal **y del Hi-Z** a las que se funden (tienen huecos); las sombras toman las estables y las que entran. Se dibujan después de la segunda pasada del Hi-Z con `fade_pipe` = mismo vértice + `fs_fade` (Bayer 4×4 complementario, 0,4 s), **el único fragmento con `discard`**: `fs_main` sigue sin él. El avance sale del reloj de las globales (`cam_pos.w`) y del comienzo de cada columna (`fundido[slot]`, escrito una vez, por tramos con las demás subidas); las colas de entrantes y salientes están en orden y solo se mira el frente. La que sale se libera a los 0,4 s (su agua se sigue dibujando hasta entonces). Opción "Fundido entre niveles" (desde el preset Medio). Fuera de la ventana está apagado (capturas, banco de vistas, demo). |
| Cobertura de ediciones (§2.3, `HierarchicalReducer`) | `store::reducir_por_cobertura`: desde el nivel 3 (`COBERTURA_DESDE`), reducción encadenada desde el nivel 2: sólido si ≥ 4 de 8 hijos sólidos (rejas, cercos, andamios, faroles cuentan medio: `es_fino`, en `store.rs` para no tocar `palette.rs`) o ≥ 2 de 4 columnas cubiertas; gana el sólido más alto con el desempate de siempre. Un vóxel que no queda sólido es aire solo si la carga lo vació (aire ≥ sólidos); si no, no se escribe (una carga nunca borra lo de otra con aire que no puso). Niveles 0–2 sin cambios. |
| Relleno (§2.1) y fin de `far_drop` | `mesher::mesh_chunk_relleno` + `Relleno { top, fill, y0 }`: recorte de la cara de arriba en 1/16 de vóxel (`Quad.a` bits 26–29), costados con el borde de arriba recortado y el de abajo corrido hasta la superficie del vecino (`Quad.b` bits 26–30, con signo) y **escalones** entre vecinos con el vóxel de superficie a la misma altura; solo en superficies con aire encima. `voxel.wgsl::corner` (y así `vs_shadow`) aplica el corrimiento; el agua usa la cara recortada como fondo (`agua::mallar` recibe `fill` en vez de `drop`). Se aplica desde el nivel 3 **solo a la tierra baja de la costa**: la que antes quedaba un vóxel entero (4 a 256 m) sobre el agua se dibuja a 3 m (`stream::superficie`, redondeado hacia arriba a 1/16 de vóxel: 16 m en el de 256 m). `lod::far_drop`, `DROP_LOD` y `top_from_height_far` se eliminaron (la obra de los niveles 8–10 ya no baja `vóxel − 3 m`). `MOTOR_SIN_RELLENO=1` y `MOTOR_RELLENO_DESDE=n` para medir. |
| SSAO lejano (§2.13, `lod_ssao` + `lod_acabado`) | `post.wgsl`: 8 muestras en espiral áurea, radio `máx(2 m, 0,025·d)` en metros (pasado a píxeles con el foco), solo lo que está a más de un 20 % del radio y por encima de ~20° (los escalones de un vóxel dibujaban curvas de nivel), a media resolución (Rg16Float: oclusión y distancia) y aplicada a resolución completa con un realce bilateral y mezcla multiplicativa, desde 100–200 m y menos con la niebla. La profundidad se guarda (`Store`) solo con SSAO. Opción "Oclusión lejana" (Alto y Ultra) y primera perilla del auto-ajuste con la GPU al límite. |
| Escala de render (§2.16, `EscaladoFsr` + `PruebaEscalado`) | `render/escala.rs` + `post.wgsl`: la escena (con su Hi-Z y MSAA) se dibuja al 100/85/75/67 %, se realza a la salida con Catmull-Rom de 9 lecturas bilineales y se afila con RCAS de FSR 1 (MIT); la interfaz va después, nítida. **EASU no se portó** (queda pendiente). `autoajuste::PruebaEscalado`: 30 cuadros con escala y 30 sin, dos veces; sigue si la mediana gana más de un 3 %, si no se apaga por la sesión (aviso en el menú). Perilla del auto-ajuste solo si el jugador eligió escalar (piso 67 %). Preset Mínimo al 75 %. |
| Auto-ajuste | Perillas nuevas `Ssao` y `Escala` (`Perillas::con_efectos`); orden GPU: SSAO → sombras → escala → anillo → niveles; recupera en orden inverso. |
| Sombra de personajes y animales | Mapa propio de 2048² con la matriz de la cascada 0, rehecho en cada cuadro solo con las cajas de los agentes (`agentes.wgsl::vs_proyecta`, `agent_cast_pipe`); las cascadas siguen en caché (mover los agentes las habría rehecho siempre). `sombras.wgsl::shadow` = estática × `shadow_agentes` (PCF 4 lecturas, `misc.w` = hay mapa): lo reciben el terreno, la flora y los mismos agentes. `Renderer::sombra_agentes` para medir. |
| Ramas colgantes del sauce | `stream::leaf_fuzz` (nivel 0, micro-vóxeles): de la cara de abajo de la copa del sauce cuelgan hebras de 0,5–2 m (60 % de los vóxeles de abajo, sin llegar al suelo), que tiemblan como las hojas. Sin captura propia (no hay sauce en las vistas del banco). |

**No se hizo:** tinte de la flora en el suelo (§2.15 a: cambia el color de todo el pasto lejano,
también en el mapa de prueba sin flora; queda para hacerlo con el mapa real a la vista), EASU.

### 7.2 Vistas quietas (conteos e imagen)

| Vista | Caras antes → después | Diferencia de imagen con la v0.12.0 |
|---|---|---|
| Ciudad de cerca | 0,75 → 0,76 M | 0,06 % |
| Península | 0,51 → 0,52 M | 0,27 % |
| Lejos (9 km) | 0,23 → 0,24 M | 1,08 % (costa a 3 m) |
| Calle | 0,71 → 0,73 M | 0,000 % |
| Cerro, rasante | 1,95 → 1,99 M | 0,61 % |
| Cerro | 1,61 → 1,65 M | 1,10 % |

- **Relleno con la altura real en todas las columnas** (lo que pedía §2.1): Península 0,51 →
  1,17 M caras (desde el nivel 3), 0,89 M (desde el 6), 0,71 M (solo 8–10): cada columna con su
  recorte casi no se une con la vecina. Se dejó solo para la costa (+2–4 % de caras, por los
  escalones de la orilla). **Criterio "caras de Lejos ≤ hoy": no** (+4 %); **"−10 % en niveles ≥ 4"
  : no** (la cobertura apenas toca la ciudad de prueba, que es chica frente al terreno).
- **Niveles 0–2 iguales:** Calle 0,000 %, Ciudad de cerca 0,06 % (lo de la orilla lejana).
- **Determinismo:** dos corridas del banco (Península y Cerro rasante con su barrido): PNG
  idénticos byte a byte. La demo (`--demo-ciudad`) imprime las mismas 129 líneas que la v0.12.0
  salvo la versión, tiempos y los incendios, que ya variaban entre dos corridas del mismo binario
  (la simulación depende del reloj), igual que la hora de cada captura.
- **Salto al recentrar:** Península 4,63 → 4,50 %, Lejos 0,56 → 0,49 %: lejos del −50 % (lo
  dominan los niveles finos, §6.2).

### 7.3 Fundido, SSAO, escala y sombras (GPU, lavapipe, cargada)

| Vista | Fundido 32 / 128 columnas | SSAO (dif. de imagen) | Escala 67 % (dif.) | Prueba del escalado |
|---|---|---|---|---|
| Península | −13 / −12 % (ruido) | −16 % (2,5 %) | −26 % (13,6 %) | sigue |
| Calle | +4,6 / +5,1 % | +11 % (0,00 %) | −18 % (3,9 %) | sigue |
| Cerro, rasante | +13,5 / −4,3 % | −17 % (1,1 %) | −15 % (10 %) | sigue |
| Cerro | +19 / −7 % | −23 % (1,8 %) | −45 % (32 %, textura fina) | sigue |

Con la máquina cargada las cifras de GPU se contradicen (SSAO "ahorra"): **no sirven para
validar los criterios de +3 % (fundido) y +8 % (SSAO)**; hay que medirlos en una placa. Lo que sí
vale: **quieto, el fundido no cambia nada** (0,000 % después de terminar en las cuatro vistas, y
sin fundidos en curso la pasada no se dibuja); el pipeline principal no tiene `discard`
(revisado); el SSAO de cerca no cambia la imagen (Calle 0,00 %, criterio < 1 %); la escala al
67 % gana un 15–45 % aun en lavapipe (vértices incluidos). En el vuelo hay hasta 720 columnas
fundiéndose a la vez (~275 por cuadro): con un zoom de 3 km se retiran cientos.

**Sombra de personajes** (multitud de 2500 agentes, mínimo de los cuadros): con cajas, Calle
868 → 900 ms (+3,7 %), Ciudad de cerca 882 → 948 ms (+7,5 %); con los modelos de micro-vóxeles
cerca, +11–16 % (88 mil cajas en el mapa). En el juego hay muchos menos agentes a la vista.

Capturas: `docs/capturas/render-fundido-antes-despues.jpg` (abajo, 32 columnas a mitad del
fundido), `render-ssao-cerro-antes-despues.jpg`, `render-escala67-antes-despues.jpg`,
`render-relleno-rasante-antes-despues.jpg` (arriba v0.12.0), `render-sombras-personajes-antes-despues.jpg`.

### 7.4 Vuelo: costo del fundido y el criterio de p99 −30 %

Máquina quieta (recién reiniciada, carga ~1,5), `chrt -f 1`, `MOTOR_BANCO=streaming`, corridas
alternadas; "sin fundido" es el mismo binario con `MOTOR_SIN_FUNDIDO=1`.

| Vuelo (84 s) | v0.12.0 (2 corridas) | Ahora, sin fundido (3) | Ahora, con fundido (4) |
|---|---|---|---|
| Cuadro p50 | 0,16–0,17 ms | 0,15–0,20 ms | 0,32–0,41 ms |
| p99 | 1,87–2,03 ms | 1,92–2,06 ms | 2,23–2,43 ms |
| CPU del hilo, total | 2,25–2,34 s | 2,26–2,47 s | 2,69–2,94 s |
| Peor subida | 5,7–12,9 ms | 5,7–36,8 ms (un pico) | 8,6–11,0 ms |

- Sin fundido, la tanda no cambia el vuelo (relleno, cobertura y lo demás son de generación).
- El fundido cuesta **~0,2 ms por cuadro de CPU** en el vuelo (p99 +15 %), con ~280 columnas
  fundiéndose por cuadro. Se bajó de +0,9 ms (primera versión: el avance se escribía por cuadro
  y por columna) a +0,4 (avance en el shader con el reloj de las globales), +0,25 (índice de
  retiradas incremental: rearmarlo costaba ~0,1 ms por cuadro) y lo que queda; lo que sigue
  sospechado son los bloques del megabúfer y los lugares de la tabla retenidos 0,4 s más (más
  bloques vivos en el asignador). No es GPU (el banco del vuelo no dibuja): el criterio de GPU
  (+3 %) queda para una placa.
- **Criterio del p99 −30 % del vuelo (Fase 1), documentado:** se mide contra el binario de la
  Fase 0 (o la versión anterior) **en la misma sesión**, con la máquina quieta, `chrt -f 1` y al
  menos dos pares de corridas alternadas; vale la mediana de los p99 de cada binario. Las cifras
  absolutas no se comparan entre sesiones: el mismo binario de la v0.12.0 dio p99 1,44 ms una
  hora antes (con otros procesos tomando núcleos) y 1,87–2,03 ms en esta máquina recién
  reiniciada. Con lavapipe lo que queda del p99 es el envío de las copias en el hilo principal
  (§6.1), que en una placa hace la GPU: el criterio **no se cumple acá** (la v0.12.0 ya estaba en
  el mismo nivel que la Fase 0) y hay que volver a medirlo con `medir-fps.bat` en placa. No se
  encontró una mejora barata más allá de las del fundido.

## 8. "Se lagea más que antes": v0.12.0 → v0.13.0 en el juego (2026-11-25)

Reporte del usuario (Windows, placa real): la v0.13.0 se siente más lenta y con los FPS más
inestables que la v0.12.0. Los bancos de vistas quietas y del vuelo no lo mostraban porque no
corren la ciudad: faltaba medir **como el juego**.

### 8.1 Cómo se mide

- **Banco del juego** (`MOTOR_BANCO_JUEGO=1 motor --demo-ciudad carpeta --size 1280x720`;
  `MOTOR_BANCO_JUEGO=3` también arma la época 3; `MOTOR_BANCO_JUEGO_CUADROS=n`, 360 por omisión):
  funda, traza, zonifica, crece 60 meses y larga los vecinos (época 3: Ciudad Nueva e Inmigración
  con tranvías y fábricas), pone la cámara sobre la calle más concurrida y hace dos recorridos de
  `n` cuadros con el reloj del juego a 60 cuadros por segundo: **calle** (30 m, 12°) y **media
  altura** (260 m, 36°), paneando como con W/A/S/D, girando y con un zoom de ida y vuelta (±45 %).
  Cada cuadro hace lo mismo que la ventana (`Session::update`, `apply_ops` con el tope de subida de
  la ventana, selección bajo el cursor, dibujo) y escribe `juego-época-N.md` con mediana, p95,
  p99, peor y media de cada sección. Al final de cada recorrido mide la sombra de los personajes
  (mínimo de 10 cuadros, con y sin el mapa).
- **Secciones** (`perfil.rs`, acumuladores por hilo, dos lecturas del reloj por sección):
  simulación, cajas (vecinos, escenas, rampas, fauna y detalles, fachadas), subir agentes, luces,
  streaming, selección, interfaz, dibujo (CPU), fundido (parte del dibujo), espera (vsync/GPU) y
  auto-ajuste; "trabajo del hilo principal" es la suma sin la espera.
- **En la placa del jugador**: `medir-tirones` (`CITYSIM_PERFIL=1`) escribe en `tirones.txt`
  la misma tabla cada 15 s de juego (con el preset, si está en el menú, cuántas veces se
  rehicieron los destinos de la escena y el estado del auto-ajuste), cada cambio de perilla, y en
  cada cuadro lento las cuatro secciones más largas.
- Lavapipe (render por CPU, 4 núcleos): el "cuadro" es 0,5–2,5 s de rasterizado y no dice nada;
  valen el **trabajo del hilo principal** (que en la placa es lo que tiene que entrar en 16,7 ms)
  y las proporciones de la GPU. v0.12.0 se midió con el mismo banco portado a su código
  (`d3ef56b` + `perfil.rs`, sin escenas ni rampas, que no existían).

### 8.2 Qué se encontró (trabajo del hilo principal, ms: mediana / p99 / peor)

| Escenario | v0.12.0 | v0.13.0 | Arreglado |
|---|---|---|---|
| Época 1, calle (360 cuadros) | 1,50 / 5,80 / 7,1 | 1,81 / 7,02 / **15,2** | 1,77 / 5,37 / 7,5 |
| Época 1, media altura | 10,6 / 20,4 / 26,2 | 11,3 / 25,7 / **30,1** | 10,5 / 20,8 / 35,6 ¹ |
| Época 3, calle (240 cuadros) | — | 3,40 / 15,0 / **109** | 3,43 / 9,6 / 9,8 |
| Época 3, media altura | — | 9,1 / 35,6 / **352** | 8,4 / 16,3 / 19,9 |

¹ Un solo cuadro con el dibujo en 31,8 ms (lavapipe; la sección de rampas, 0,13 ms).

1. **Rampas de los escalones (la causa de los tirones).** `armar_rampas` armaba hasta 3 tramos
   nuevos por cuadro en el hilo principal. Un tramo no cuesta 5 ms: cuesta lo que mide (alturas
   de la calzada columna por columna); en la época 3 (avenidas y tranvías) un cuadro llegó a
   **105 y 345 ms**, y a media altura, donde la vista recorre barrios nuevos todo el tiempo, el
   p95 de la sección era 12,4 ms y el p99 24 ms. En la época 1, peor 13–18 ms. Ahora
   (`oficios::Rampas`): en la ventana se arman en un **hilo aparte** con una copia de la red
   (`RoadNetwork: Clone`, rehecha solo si cambia la firma de la red), de a 8 tramos por tanda, y
   el cuadro junta las que ya están; la lista de tramos cercanos se recalcula solo si la vista se
   movió más de 20 m o cambió la red (en cada cuadro se mide la distancia exacta a esos
   candidatos: lo dibujado es lo mismo que recorriendo toda la red). Sección rampas: peor
   13,1 / 18,4 / 105 / 345 → 0,25 / 0,13 / 0,46 / 0,32 ms. Las capturas del recorrido (y la ventana
   con `--auto-test`) las siguen armando en el cuadro, como antes (`Session::rampas_en_hilo`).
2. **Más agentes en la época 3.** Con la ciudad de la inmigración hay 20 767 cajas a la vista de
   la calle (época 1: 6 439; v0.12.0: 5 009): cajas de vecinos 0,68 ms, subir agentes 0,51,
   luces 0,32 (recorren todos los faroles de gas, sebo y eléctricos), escenas 0,25, fachadas 0,17
   de mediana: ~2 ms por cuadro contra ~0,5 de la v0.12.0. No da tirones, pero en un procesador
   lento (A12 de la A275, unas 3–4× más lento que esta máquina por núcleo) son 6–8 ms por cuadro.
   No se tocó (sin tirones; cachear las luces o el hash de las cajas daría décimas de ms): queda
   para cuando haya datos de la placa.
3. **Sombra de los personajes.** Con el mapa rehecho en cada cuadro: época 1 calle (6 439 cajas)
   −1,3 % (ruido); época 3 calle (20 767) **+6,7 %** del cuadro; media altura (1 948) +6,6 %: casi
   todo es fijo (limpiar el mapa de 2048² y leerlo en cada píxel), no por caja. Ahora el mapa no
   se redibuja si no cambiaron las cajas ni la matriz de la cascada 0 (partida en pausa, cámara
   quieta sin agentes moviéndose), y va con la perilla de sombras: **en el escalón más bajo
   (preset Mínimo, o el auto-ajuste en el piso) no se dibuja** (única diferencia de imagen, y solo
   ahí).
4. **Auto-ajuste: el SSAO y la escala oscilaban.** Fuera del tope (sin vsync), con el SSAO en el
   borde (prendido pasa el objetivo, apagado sobra el 20 %) se prendía y apagaba cada 3–4 s:
   **115 cambios en 400 s** en la simulación de la prueba
   `el_ssao_en_el_borde_no_se_prende_y_apaga_en_bucle`, y cada cambio rehace los destinos de la
   escena (MSAA, profundidad, Hi-Z, SSAO). Lo mismo puede pasar con cualquier perilla, pero las dos
   nuevas son las caras de mover y las primeras del orden de la GPU. Ahora: si hay que bajar dentro
   de los 6 ciclos de una subida, **se deshace esa subida** y la perilla queda bloqueada 30 ciclos
   (60, 120, hasta 240 si vuelve a fallar enseguida); el SSAO y la escala no se vuelven a subir
   antes de 10 ciclos de haberse movido. Resultado: ≤ 10 cambios en 400 s (pruebas
   `el_ssao_en_el_borde…` y `la_escala_en_el_borde_no_oscila`). La prueba del escalado que no gana
   ya no vuelve el techo entero a sus valores (antes `set_techo` subía de golpe todo lo que el
   auto-ajuste había bajado): solo deja la escala al 100 % (`Autoajuste::apagar_escala`).
5. **Fundido entre niveles**: en el juego, 0,00–0,01 ms de mediana y 0,04 ms de p99 de CPU por
   cuadro (pocas columnas por cuadro al caminar o hacer zoom, a diferencia del vuelo de §7.4). Se
   deja prendido desde Medio; su costo de GPU queda para la placa.
6. **GPU del pase principal** (lavapipe, época 1, calle): v0.12.0 464 ms, v0.13.0 500 ms (+8 %),
   arreglado 483 ms (las cifras de lavapipe varían ±5 % entre corridas).

### 8.3 Qué no se pudo medir sin la placa

El costo de GPU real del fundido, del SSAO, de la escala y de la sombra de los personajes (en
lavapipe la GPU es la CPU y comparte los núcleos con los hilos de generación); si el vsync y el
tope de FPS del jugador dejan el auto-ajuste en el tope o fuera de él; y cuánto pesan los ~2 ms de
cajas y luces de la época 3 en su procesador. Para eso está el desglose de `tirones.txt`.
