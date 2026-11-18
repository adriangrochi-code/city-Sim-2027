# Técnicas del mod LOD de Minecraft aplicadas al motor de vóxeles

> Documento de plan. Complementa a [motor-voxel.md](motor-voxel.md) (sobre todo §4, §10–12, §16 y
> §17). Fecha: 2026-11-18. **No cambia código**: es el inventario, la comparación y el plan por
> fases para llevar al motor (`engine/`) lo que el mod LOD de Minecraft (`/home/user/minecraft-lod`,
> Java/NeoForge, versión 0.26.38) ya resolvió y midió.
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

El plan arranca **después** de que se unan a `ccr-889e1313-dztkuu` estos trabajos, porque tocan los
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
