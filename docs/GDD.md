# City Sim 2027: documento de diseño (v0.1)

> Estado: **borrador de visión (v0.2: progresión por eras)**. Este documento define la intención, la estética y las
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

### El giro: una ciudad que atraviesa la historia
En lugar de una ciudad contemporánea genérica, **la partida empieza en la época victoriana y la
ciudad evoluciona por eras hasta un futuro lejano**. Calles de adoquín, farolas de gas y
tranvías de caballos acaban convertidos, tras décadas de decisiones del jugador, en avenidas con
vehículos autónomos y torres verdes. Lo que construiste antes sigue ahí: **la ciudad final
muestra en sus capas toda su historia**, como las ciudades reales (ver sección 4).

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
6. **Con memoria:** el paso del tiempo deja huella. Nada se "reemplaza" sin más: se conserva,
   se reforma o se derriba, y cada opción tiene consecuencias.

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
- **Variedad con coherencia:** el estilo arquitectónico lo marca **la era** en la que se
  construyó cada edificio (ver 3.5). Los edificios se generan por módulos (planta baja +
  cuerpo + coronación) para evitar repeticiones evidentes.
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
pájaros y el murmullo comercial. La música cambia según la hora del día, el estado de ánimo de
la ciudad y **la era**: cuerdas y piano en la época victoriana, jazz en los años 20,
sintetizadores en la era contemporánea y texturas ambientales en el futuro.

### 3.5 Estética por era
Cada era tiene su propio "kit" visual: arquitectura, vehículos, mobiliario urbano, color de
la luz nocturna e incluso el marco de la interfaz.

| Era | Periodo aprox. | Arquitectura | Calle y transporte | Luz nocturna | Paleta |
|---|---|---|---|---|---|
| **1. Victoriana / industrial** | 1850–1900 | Ladrillo, hierro fundido, cubiertas de pizarra, estaciones con bóveda de vidrio | Adoquín, carruajes, tranvía de caballos, tren de vapor | Gas: ámbar tenue y parpadeante | Ocres, rojo ladrillo, verde botella, humo gris |
| **2. Belle Époque / Art Déco** | 1900–1940 | Fachadas ornamentadas, primeros rascacielos escalonados, cines | Tranvía eléctrico, primeros coches, metro temprano | Incandescente cálida, neones tempranos | Crema, dorado, verde azulado |
| **3. Modernismo de posguerra** | 1945–1975 | Hormigón, muro cortina, brutalismo, barrios residenciales de casas unifamiliares | Autopistas, coche dominante, autobús | Vapor de sodio: naranja intenso | Beige, turquesa, naranja |
| **4. Contemporánea** | 1980–2030 | Vidrio y acero, rehabilitación de naves industriales | Metro, carril bici, coche eléctrico | LED blanco | Grises neutros, acentos de color |
| **5. Futuro cercano** | 2030–2080 | Madera en altura, fachadas verdes, cubiertas solares | Vehículos autónomos, drones de reparto, calles peatonales | LED adaptativo, cálido | Verdes, madera, blanco |
| **6. Futurista** | 2080+ | Arcologías, megaestructuras, pasarelas aéreas | Maglev, vehículos aéreos | Paneles de luz ambiental, acentos cian | Blanco nacarado, cian, vegetación integrada |

Notas:
- La atmósfera también evoluciona: niebla y humo de carbón en la era victoriana, aire limpio
  (o no, según tus decisiones) en las eras siguientes.
- Un filtro de color *opcional* por era (ligero sepia victoriano, Kodachrome a mediados de
  siglo…) para reforzar la sensación de época sin estropear la legibilidad.

---

## 4. Mecánicas base

### 4.1 Bucle principal
```
Construir infraestructura → Zonificar → Llegan agentes (ciudadanos, empresas)
      ↑                                              ↓
Ajustar (impuestos, servicios, políticas) ← Observar problemas y oportunidades
```
Por encima de este bucle hay uno largo: **prepararse para la siguiente era** (sección 4.2).

Ritmo objetivo: cada 5–10 minutos, el jugador debería encontrar un problema nuevo que
*entender* (no solo un número en rojo).

### 4.2 Eras: la mecánica central de progresión
Las eras sustituyen al típico sistema de "desbloqueo por población".

**Cómo se avanza**
- El calendario avanza solo, pero **el paso de era lo decide el jugador** cuando cumple unos
  hitos: población, nivel educativo (universidades e institutos de investigación) y un
  **gran proyecto de transición** (p. ej. la *Exposición Universal* para entrar en la Belle
  Époque o la *primera red eléctrica municipal*).
- Quedarse atrás es una opción válida pero con costes: tus ciudadanos empiezan a mudarse a
  ciudades vecinas más avanzadas y el comercio regional pierde valor.

**Qué cambia con cada era**
- **Nuevos edificios, servicios y transporte**, y algunos se vuelven obsoletos (los establos
  dejan de tener sentido cuando llegan los coches).
- **Las expectativas de los ciudadanos**: en 1870 nadie exige electricidad; en 1950 es
  imprescindible y en 2050 piden fibra y zonas verdes.
- **Los problemas característicos**:

| Era | Problema central | Ejemplo de mecánica |
|---|---|---|
| Victoriana | Higiene y epidemias, humo de carbón, incendios | El agua contaminada propaga enfermedades por la red; hay que construir alcantarillado |
| Belle Époque | Electrificación, convivencia caballos/tranvías/coches | Calles mixtas lentas; reconvertir líneas de tranvía |
| Posguerra | Expansión suburbana y dependencia del coche | Las autopistas cortan barrios y bajan el valor del suelo |
| Contemporánea | Precio de la vivienda, gentrificación, congestión | Los barrios históricos atraen turismo y encarecen el alquiler |
| Futuro cercano | Clima y transición energética | Olas de calor, inundaciones y la reconversión de infraestructuras fósiles |
| Futurista | Automatización y sentido de comunidad | El empleo cambia; los ciudadanos valoran la cultura y el espacio público |

**La ciudad con memoria (el diferencial)**
Al cambiar de era, los edificios existentes **no desaparecen**. Para cada barrio el jugador elige:
- **Conservar** → *distrito histórico*: más turismo y valor cultural, pero limita la densidad.
- **Rehabilitar** → la estructura se mantiene con usos nuevos (un almacén victoriano convertido
  en lofts o en un centro tecnológico).
- **Reurbanizar** → derribar y construir según la era actual: más densidad, pero pierdes
  patrimonio y puede haber protestas vecinales.

Resultado: dos ciudades nunca son iguales, y en la era futurista se puede leer la historia
de tu partida paseando por la calle (el núcleo victoriano junto a la estación, el ensanche
art déco, la autopista de los 60 convertida en parque elevado…).

**Infraestructura que evoluciona**
Las redes se heredan y se mejoran, no se reconstruyen desde cero: el adoquín se asfalta, las
líneas de tranvía de caballos se electrifican, las centrales de carbón se reconvierten. Las
decisiones antiguas pesan; por ejemplo, el trazado de calles victoriano estrecho limita la
densidad que se podrá alcanzar después.

### 4.3 Carreteras y transporte
- Trazado libre: rectas, curvas, rotondas e intersecciones que se generan automáticamente.
- El catálogo depende de la era: camino de tierra y adoquín → calle asfaltada → autopista →
  calle compartida → carril autónomo.
- **El tipo de vía define la densidad** que pueden alcanzar las zonas a su lado:
  calle → baja, calle ancha → media, avenida → alta.
- Transporte público por agentes según la era: tranvía de caballos, tren de vapor, tranvía
  eléctrico, metro, autobús, maglev. Bicicleta y
  peatones como modos reales, no decorativos.
- Mejora frente al referente: **rutas inteligentes**. Los agentes eligen la ruta según el
  tiempo estimado, la congestión y el modo de transporte, y cada ciudadano tiene un trabajo y
  una casa *asignados* (no "el más cercano libre").

### 4.4 Zonas y crecimiento
- Tres zonas clásicas: **residencial, comercial e industrial**, más **oficinas/tecnología** como
  evolución de la industria limpia.
- Las parcelas se generan siguiendo la carretera (también en curvas) y pueden fusionarse
  para formar edificios grandes.
- Cada edificio tiene **riqueza** (baja/media/alta) y **nivel**. La mejora depende de cosas
  visibles: valor del suelo, servicios, educación y felicidad.
- **Panel "¿Por qué no crece?"**: al seleccionar una parcela se ve qué le falta. Es la regla
  de oro de la legibilidad.

### 4.5 Servicios por red
- **Electricidad, agua y alcantarillado viajan por la red de carreteras** (o tuberías y líneas
  dedicadas en fases avanzadas). Si una zona no conecta con la fuente, no recibe servicio.
- **Servicios con agentes**: los bomberos, la policía, las ambulancias, la basura y el
  reciclaje salen físicamente de su edificio y recorren la ciudad. Con el tráfico mal
  resuelto, llegan tarde.
- **Edificios modulares**: una estación de bomberos se amplía con garajes; un hospital, con alas;
  una central eléctrica, con generadores. Se crece por piezas en lugar de reconstruir.

### 4.6 Ciudadanos (simulación por agentes)
- Cada ciudadano tiene casa, trabajo o escuela, nivel educativo, riqueza y estado de ánimo.
- Se puede **seguir a cualquier ciudadano**: ver su rutina, su trayecto y sus quejas.
- La simulación funciona por niveles de detalle: los agentes cercanos a la cámara se simulan
  completos y los lejanos de forma agregada, con resultados equivalentes. Así los mapas
  pueden ser mucho más grandes que en el referente.

### 4.7 Economía y política
- Presupuesto con impuestos por zona y riqueza, mantenimiento y préstamos.
- **Ordenanzas/políticas** (p. ej. "prohibido fumar", "fomento de la bicicleta", "industria
  limpia") con efectos y costes claros.
- **Especialización de la ciudad**: minería y petróleo, manufactura, comercio y turismo,
  tecnología y educación, cultura y ocio. Cada una desbloquea edificios y cadenas propias y
  cambia el aspecto de la ciudad.

### 4.8 Terreno y medio ambiente
- Herramientas de modelado del terreno antes y durante la partida (con coste).
- Ríos, costas, recursos naturales (agua subterránea, minerales, petróleo, viento, sol) que se
  agotan o se regeneran.
- La contaminación del aire y del suelo se propaga con el viento y el agua de forma visible.

### 4.9 Región
- Varias ciudades en un mapa regional compartido: **un jugador o cooperativo asíncrono
  opcional**, siempre jugable sin conexión.
- Comercio de servicios (vender electricidad o agua), trabajadores que se desplazan entre
  ciudades y **grandes proyectos regionales** construidos entre todos (aeropuerto
  internacional, estación espacial, megapresa…).

### 4.10 Eventos y desastres
Incendios, inundaciones, terremotos, tormentas y apagones: son opcionales y configurables.
Sirven para poner a prueba la red de servicios, no para castigar sin aviso.

### 4.11 Modos de juego
- **Carrera** con objetivos y desbloqueos progresivos (tutorial integrado).
- **Sandbox** con todo desbloqueado y dinero ilimitado opcional.
- **Escenarios**: retos con condiciones concretas (recuperar una ciudad en quiebra, etc.).

---

## 5. Qué hace a este juego "más moderno"

1. **Progresión por eras** de la época victoriana al futuro, con una ciudad que conserva su
   historia.
2. **Mapas grandes** gracias a la simulación con niveles de detalle.
3. **Rutas inteligentes** y transporte multimodal real.
4. **Legibilidad**: el panel "¿por qué?" está en todo, y se puede seguir a cualquier agente.
5. **Clima, estaciones y ciclo día/noche** con efecto en la simulación.
6. **Sin conexión obligatoria**; la región es local con cooperativo opcional.
7. **Modding desde el primer día**: edificios, estilos arquitectónicos y políticas definidos
   en datos, no en código.
8. **Modo foto** y cámara a ras de calle.

---

## 6. Alcance del primer hito jugable (cuando toque desarrollar)

Solo como referencia para el futuro. Todavía **no** se ha empezado.

- **Solo la era victoriana**: una era bien hecha vale más que seis a medias.
- Terreno plano, calles de adoquín y avenidas con densidad según la vía.
- Zonas R/C/I con crecimiento por demanda.
- Agua y alcantarillado que viajan por las calles (la electricidad llega con la era 2).
- Agentes (peatones, carruajes, tranvía de caballos) casa ↔ trabajo con rutas que evitan la congestión.
- Capa de datos de tráfico, panel "¿por qué no crece?" y guardado local.
- Iluminación de día y noche con farolas de gas y una primera pasada de "realismo estilizado".
- Hito siguiente: la transición a la era 2, para validar la mecánica de conservar, rehabilitar o
  reurbanizar.

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
| Coste de arte 3D realista con pocas personas | Edificios modulares + generación procedimental de fachadas |
| Las eras multiplican el arte necesario (×6) | Lanzar con 3–4 eras y añadir el resto después; compartir volúmenes entre eras y cambiar solo fachadas, cubiertas y detalles |
| Equilibrio entre eras (que una sea mucho más fácil o aburrida) | Cada era con un problema central propio (tabla 4.2) y pruebas de juego tempranas |
| Rendimiento de miles de agentes | Simulación por niveles de detalle; mover el núcleo a C#/C++ pronto |
| Que los agentes se perciban "tontos" | Asignación casa/trabajo estable y rutas por tiempo, con congestión |
| Alcance excesivo (región, multijugador) | La región y el cooperativo quedan para después del primer juego completo de una sola ciudad |

---

## 9. Preguntas abiertas

1. **Nombre definitivo:** "City Sim 2027" ya no encaja con un juego que recorre 250 años.
   Habría que buscar algo que hable del tiempo (p. ej. *"Eras"*, *"Crónica"*, *"Legado"*,
   *"Century"*, todos por comprobar).
2. **Escala de los mapas:** ¿cuántos km² por ciudad?
3. **Cuadrícula o trazado libre** para las parcelas: el trazado libre es más bonito, pero
   mucho más costoso de implementar.
4. **¿Victoriana británica o algo más amplio?** "Victoriano" es muy inglés; una primera era
   "industrial del siglo XIX" podría mezclar Londres, París, Buenos Aires o Barcelona.
   ¿Ciudad de inspiración concreta o ecléctica?
5. **Profundidad del ciudadano:** ¿nombres e historias individuales o anonimato con datos?
6. **Monetización y plataforma:** ¿PC primero (Steam)? ¿precio único y DLC de estilos?
7. **¿Multijugador regional?** ¿Imprescindible o un extra a largo plazo?
8. **¿Cuántas eras en el lanzamiento?** Propuesta: 1–3 (victoriana → posguerra) y el resto
   como expansión.
9. **¿Tiempo continuo o saltos?** ¿Una partida recorre los 250 años, o cada era se juega como
   capítulo con un salto de tiempo y un resumen de "lo que pasó"?
10. **¿Las ciudades de una región pueden estar en eras distintas?** (Interesante para el
    comercio, pero difícil de equilibrar.)
