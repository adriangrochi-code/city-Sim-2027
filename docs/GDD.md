# City Sim 2027: documento de diseño (v0.1)

> Estado: **borrador de visión**. Este documento define la intención, la estética y las
> mecánicas base *antes* de escribir código. Todo lo que aparece aquí está abierto a discusión.
> Las preguntas pendientes se reúnen en la sección 9.

---

## 1. Intención

**Un constructor de ciudades que se sienta vivo.** La idea es hacer un sucesor espiritual de
los constructores de ciudades basados en agentes de principios de los 2010: la ciudad se
entiende *viéndola funcionar*. Cada coche, cada camión de basura y cada ciudadano es un
agente con un origen y un destino reales, no un número en una hoja de cálculo.

Queremos recuperar lo que hizo especial a ese estilo de juego y corregir lo que lo frustró:

| Lo que conservamos | Lo que corregimos |
|---|---|
| Simulación por agentes legible: ves por qué pasan las cosas | Agentes "tontos" (todos al trabajo más cercano, rutas idénticas, atascos absurdos) |
| Servicios que viajan por la red de carreteras | Parcelas diminutas que obligan a jugar en un espacio pequeño |
| Carreteras curvas y zonas que siguen la carretera | Conexión obligatoria a internet; nada de jugar sin conexión |
| Densidad determinada por el tipo de vía | Sin control del terreno (no se podía modelar el paisaje) |
| Especialización de ciudades y región compartida | Región en la que las ciudades vecinas apenas importaban de verdad |
| Edificios de servicio modulares (ampliables por piezas) | Progresión opaca: no sabías qué faltaba para que algo creciera |
| Capas de datos visuales (tráfico, contaminación, crimen…) | Interfaz recargada en pantallas grandes y poca accesibilidad |

**En una frase:** *"La ciudad que ves es la simulación que ocurre."*

### Público
- Jugadores de constructores de ciudades que echan de menos la sensación "de juguete vivo" y
  agente a agente, pero quieren mapas más grandes y más profundidad.
- Jugadores nuevos que buscan algo bonito y relajante, con curva de aprendizaje suave.

### Propiedad intelectual
El proyecto es **original**: nombre, edificios, interfaz, música y marca propios. Tomamos el
*género* y el *tipo de mecánicas* como inspiración, no recursos, nombres ni el aspecto
reconocible de ningún juego existente. "City Sim 2027" es un nombre provisional.

---

## 2. Pilares de diseño

Cada idea nueva se compara con estos cinco pilares. Si no refuerza al menos uno, se descarta.

1. **Legible:** cualquier problema de la ciudad se puede *ver* y rastrear hasta su causa con
   un par de clics (un atasco → de dónde vienen esos coches → a dónde van).
2. **Vivo:** la ciudad se mueve, cambia con la hora y la estación y reacciona a tus decisiones
   en segundos, no en meses de juego.
3. **Bonito a cualquier distancia:** tanto a ras de calle como en la vista de toda la región,
   cada encuadre debe poder servir de captura de pantalla.
4. **Interconectado:** las ciudades de una región se necesitan entre sí (comercio, trabajadores,
   servicios compartidos, turismo).
5. **Respetuoso con el jugador:** sin conexión obligatoria, guardado libre, deshacer, pausa
   siempre disponible y herramientas de modding desde el principio.

---

## 3. Dirección estética

### 3.1 Concepto: *"realismo estilizado"*
Base **3D realista** (proporciones, materiales y luz creíbles) con un toque cuidado de
**maqueta o diorama**: la ciudad debe parecer una miniatura perfecta que cobra vida.
Más cálida y limpia que el realismo fotográfico; más sofisticada que el low-poly.

Referencias de *sensación* (no de copia): fotografía tilt-shift de ciudades reales,
maquetas de arquitectura, ilustración urbana contemporánea y la luz de la "hora dorada".

### 3.2 Reglas visuales
- **Luz como protagonista:** ciclo día/noche completo. Al amanecer y al atardecer, tonos
  cálidos y sombras largas. De noche, las ventanas se iluminan según la ocupación real del
  edificio (la luz *es* un dato de la simulación).
- **Paleta:** colores naturales y algo desaturados en el terreno y los edificios. Los colores
  saturados se reservan para la información (zonas, alertas, capas de datos), así la interfaz
  nunca compite con la ciudad.
- **Profundidad de campo dinámica:** sutil desenfoque tilt-shift al alejar la cámara (activable
  o desactivable); nítido a ras de calle.
- **Estaciones y clima:** lluvia (calles mojadas con reflejos), niebla matinal, nieve en
  invierno y árboles que cambian de color en otoño. Tienen efecto en la simulación (consumo de
  energía, accidentes) además del visual.
- **Detalle por capas (LOD):** a distancia leemos la forma y el color de los barrios; de cerca
  aparecen terrazas, toldos, bicicletas, peatones y farolas.
- **Variedad con coherencia:** cada zona tiene "estilos arquitectónicos" seleccionables
  (p. ej. *europeo*, *norteamericano*, *latinoamericano contemporáneo*). Los edificios se
  generan por módulos (planta baja + cuerpo + coronación) para evitar repeticiones evidentes.
- **Animación de crecimiento:** los edificios no aparecen de golpe: andamios, grúas y obra.
  Al mejorar de nivel, se ve la reforma.

### 3.3 Interfaz
- **Minimalista y contextual:** una barra inferior con herramientas y paneles que solo
  aparecen cuando hacen falta.
- **Estilo:** paneles translúcidos con desenfoque de fondo, tipografía sans-serif limpia e
  iconografía de línea propia.
- **Capas de datos como "modo lente":** al activar una capa, la ciudad se desatura y solo el
  dato se colorea (calor de tráfico, flujo de agua, contaminación que se dispersa con el viento).
- **Accesibilidad:** paletas aptas para daltonismo, escala de interfaz, subtítulos y
  controles reasignables.

### 3.4 Sonido
Paisaje sonoro que depende de la cámara y de la simulación: tráfico, obras, parques con
pájaros y el murmullo comercial. Música ambiental (electrónica y acústica suave) que cambia
según la hora del día y el estado de ánimo de la ciudad.

---

## 4. Mecánicas base

### 4.1 Bucle principal
```
Construir infraestructura → Zonificar → Llegan agentes (ciudadanos, empresas)
      ↑                                              ↓
Ajustar (impuestos, servicios, políticas) ← Observar problemas y oportunidades
```
Ritmo objetivo: cada 5–10 minutos, el jugador debería encontrar un problema nuevo que
*entender* (no solo un número en rojo).

### 4.2 Carreteras y transporte
- Trazado libre: rectas, curvas, rotondas e intersecciones que se generan automáticamente.
- **El tipo de vía define la densidad** que pueden alcanzar las zonas a su lado:
  calle → baja, calle ancha → media, avenida → alta.
- Transporte público por agentes: autobús, tranvía, metro y tren regional. Bicicleta y
  peatones como modos reales, no decorativos.
- Mejora frente al referente: **rutas inteligentes**. Los agentes eligen la ruta según el
  tiempo estimado, la congestión y el modo de transporte, y cada ciudadano tiene un trabajo y
  una casa *asignados* (no "el más cercano libre").

### 4.3 Zonas y crecimiento
- Tres zonas clásicas: **residencial, comercial e industrial**, más **oficinas/tecnología** como
  evolución de la industria limpia.
- Las parcelas se generan siguiendo la carretera (también en curvas) y pueden fusionarse
  para formar edificios grandes.
- Cada edificio tiene **riqueza** (baja/media/alta) y **nivel**. La mejora depende de cosas
  visibles: valor del suelo, servicios, educación y felicidad.
- **Panel "¿Por qué no crece?"**: al seleccionar una parcela se ve qué le falta. Es la regla
  de oro de la legibilidad.

### 4.4 Servicios por red
- **Electricidad, agua y alcantarillado viajan por la red de carreteras** (o tuberías y líneas
  dedicadas en fases avanzadas). Si una zona no conecta con la fuente, no recibe servicio.
- **Servicios con agentes**: los bomberos, la policía, las ambulancias, la basura y el
  reciclaje salen físicamente de su edificio y recorren la ciudad. Con el tráfico mal
  resuelto, llegan tarde.
- **Edificios modulares**: una estación de bomberos se amplía con garajes; un hospital, con alas;
  una central eléctrica, con generadores. Se crece por piezas en lugar de reconstruir.

### 4.5 Ciudadanos (simulación por agentes)
- Cada ciudadano tiene casa, trabajo o escuela, nivel educativo, riqueza y estado de ánimo.
- Se puede **seguir a cualquier ciudadano**: ver su rutina, su trayecto y sus quejas.
- La simulación funciona por niveles de detalle: los agentes cercanos a la cámara se simulan
  completos y los lejanos de forma agregada, con resultados equivalentes. Así los mapas
  pueden ser mucho más grandes que en el referente.

### 4.6 Economía y política
- Presupuesto con impuestos por zona y riqueza, mantenimiento y préstamos.
- **Ordenanzas/políticas** (p. ej. "prohibido fumar", "fomento de la bicicleta", "industria
  limpia") con efectos y costes claros.
- **Especialización de la ciudad**: minería y petróleo, manufactura, comercio y turismo,
  tecnología y educación, cultura y ocio. Cada una desbloquea edificios y cadenas propias y
  cambia el aspecto de la ciudad.

### 4.7 Terreno y medio ambiente
- Herramientas de modelado del terreno antes y durante la partida (con coste).
- Ríos, costas, recursos naturales (agua subterránea, minerales, petróleo, viento, sol) que se
  agotan o se regeneran.
- La contaminación del aire y del suelo se propaga con el viento y el agua de forma visible.

### 4.8 Región
- Varias ciudades en un mapa regional compartido: **un jugador o cooperativo asíncrono
  opcional**, siempre jugable sin conexión.
- Comercio de servicios (vender electricidad o agua), trabajadores que se desplazan entre
  ciudades y **grandes proyectos regionales** construidos entre todos (aeropuerto
  internacional, estación espacial, megapresa…).

### 4.9 Eventos y desastres
Incendios, inundaciones, terremotos, tormentas y apagones: son opcionales y configurables.
Sirven para poner a prueba la red de servicios, no para castigar sin aviso.

### 4.10 Modos de juego
- **Carrera** con objetivos y desbloqueos progresivos (tutorial integrado).
- **Sandbox** con todo desbloqueado y dinero ilimitado opcional.
- **Escenarios**: retos con condiciones concretas (recuperar una ciudad en quiebra, etc.).

---

## 5. Qué hace a este juego "más moderno"

1. **Mapas grandes** gracias a la simulación con niveles de detalle.
2. **Rutas inteligentes** y transporte multimodal real.
3. **Legibilidad**: el panel "¿por qué?" está en todo, y se puede seguir a cualquier agente.
4. **Clima, estaciones y ciclo día/noche** con efecto en la simulación.
5. **Sin conexión obligatoria**; la región es local con cooperativo opcional.
6. **Modding desde el primer día**: edificios, estilos arquitectónicos y políticas definidos
   en datos, no en código.
7. **Modo foto** y cámara a ras de calle.

---

## 6. Alcance del primer hito jugable (cuando toque desarrollar)

Solo como referencia para el futuro. Todavía **no** se ha empezado.

- Terreno plano, carreteras (calle y avenida) con densidad según vía.
- Zonas R/C/I con crecimiento por demanda.
- Electricidad y agua que viajan por las carreteras.
- Agentes de tráfico casa ↔ trabajo con rutas que evitan la congestión.
- Capa de datos de tráfico, panel "¿por qué no crece?" y guardado local.
- Iluminación diurna con sombras y una primera pasada de "realismo estilizado".

---

## 7. Tecnología (decidida)

- **Motor:** Godot 4 (renderizador Forward+), GDScript para prototipar y C# o GDExtension (C++)
  para la simulación de agentes cuando el rendimiento lo exija.
- **Arte 3D:** Blender → glTF (.glb). Escala 1 unidad = 1 metro. Edificios modulares por piezas.
- **Datos:** edificios, zonas, políticas y estilos en archivos de recursos o JSON editables (base
  para el modding).

---

## 8. Riesgos principales

| Riesgo | Mitigación |
|---|---|
| Coste de arte 3D realista con pocas personas | Edificios modulares + generación procedimental de fachadas; empezar con un solo estilo arquitectónico |
| Rendimiento de miles de agentes | Simulación por niveles de detalle; mover el núcleo a C#/C++ pronto |
| Que los agentes se perciban "tontos" | Asignación casa/trabajo estable y rutas por tiempo, con congestión |
| Alcance excesivo (región, multijugador) | La región y el cooperativo quedan para después del primer juego completo de una sola ciudad |

---

## 9. Preguntas abiertas

1. **Nombre definitivo** y tono de la marca (¿serio y elegante o más desenfadado?).
2. **Escala de los mapas:** ¿cuántos km² por ciudad?
3. **Cuadrícula o trazado libre** para las parcelas: el trazado libre es más bonito, pero
   mucho más costoso de implementar.
4. **Primer estilo arquitectónico:** ¿europeo, norteamericano o latinoamericano?
5. **Profundidad del ciudadano:** ¿nombres e historias individuales o anonimato con datos?
6. **Monetización y plataforma:** ¿PC primero (Steam)? ¿precio único y DLC de estilos?
7. **¿Multijugador regional?** ¿Imprescindible o un extra a largo plazo?
