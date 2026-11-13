# City Sim 2027: documento de diseño (v0.1)

> Estado: **borrador de visión (v0.3: de la Baja Edad Media a la era contemporánea)**. Este documento define la intención, la estética y las
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
En lugar de una ciudad contemporánea genérica, **la partida empieza en la Baja Edad Media, con
una villa amurallada, y evoluciona por eras hasta la ciudad moderna**. El mercado junto a la
catedral, las murallas y las calles tortuosas acaban rodeados, tras siglos de decisiones del
jugador, por ensanches, ferrocarril, metro y torres de vidrio. Lo construido antes sigue ahí:
**la ciudad final muestra en sus capas toda su historia**, como las ciudades reales (ver
sección 4).

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

Cada idea nueva se compara con estos seis pilares. Si no refuerza al menos uno, se descarta.

1. **Legible:** cualquier problema de la ciudad se puede *ver* y rastrear hasta su causa con
   un par de clics (un atasco → de dónde vienen esas carretas o coches → a dónde van).
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
Paisaje sonoro que depende de la cámara y de la simulación: campanas, mercados, carretas,
talleres, fábricas, tráfico y parques con pájaros. La música cambia según la hora del día, el
estado de ánimo de la ciudad y **la era**: laúd y flautas medievales, polifonía renacentista,
cuerdas barrocas, piano romántico, jazz y orquesta del siglo XX, y electrónica y acústica suave
en la era contemporánea.

### 3.5 Estética por era
Cada era tiene su propio "kit" visual: arquitectura, vehículos, mobiliario urbano, color de
la luz nocturna e incluso el marco de la interfaz (de pergamino e iluminación de manuscrito a
paneles translúcidos modernos).

| Era | Periodo aprox. | Arquitectura | Calle y transporte | Luz nocturna | Paleta |
|---|---|---|---|---|---|
| **1. Baja Edad Media** | 1300–1500 | Muralla, castillo, catedral gótica, casas de entramado de madera, molinos | Tierra y empedrado irregular, a pie, carretas de bueyes, barcas fluviales | Antorchas, velas y braseros: luz escasa y cálida | Madera, piedra caliza, ocres, verde musgo |
| **2. Renacimiento** | 1500–1650 | Palacios, plazas porticadas, universidades, puertos | Calles empedradas rectas, carruajes, comercio marítimo | Faroles de aceite en plazas principales | Terracota, piedra dorada, blanco cal |
| **3. Barroco e Ilustración** | 1650–1800 | Grandes avenidas y plazas reales, jardines formales, fachadas ornamentadas | Paseos, carruajes de caballos, canales | Faroles de aceite en las calles | Piedra clara, azul pizarra, dorados |
| **4. Revolución industrial** | 1800–1900 | Ladrillo, hierro fundido, fábricas, estaciones con bóveda de vidrio, ensanches | Adoquín, tren de vapor, tranvía de caballos | Gas: ámbar tenue y parpadeante | Rojo ladrillo, hollín, verde botella |
| **5. Siglo XX** | 1900–1970 | Art Déco, primeros rascacielos, hormigón, barrios de casas unifamiliares | Tranvía eléctrico, coche, metro, autopistas | Incandescente y vapor de sodio (naranja) | Crema, turquesa, naranja, gris hormigón |
| **6. Contemporánea** | 1970–hoy | Vidrio y acero, rehabilitación, fachadas verdes | Metro, carril bici, coche eléctrico, calles peatonales | LED blanco | Grises neutros con acentos de color, mucha vegetación |

Notas:
- La atmósfera también evoluciona: humo de hogares y fraguas en la Edad Media, hollín y niebla
  industrial en el siglo XIX, aire más limpio (o no, según tus decisiones) después.
- La noche cambia mucho entre eras: en la Edad Media la ciudad está casi a oscuras y solo
  brillan ventanas, antorchas y la catedral; en la era contemporánea es un mar de luz. Ver
  cómo la noche se ilumina era a era es parte del atractivo.
- Un filtro de color *opcional* por era (tono de manuscrito iluminado en la Edad Media, sepia en
  el siglo XIX, Kodachrome a mediados del XX…) refuerza la sensación de época sin estropear
  la legibilidad.

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
  hitos: población, conocimiento (monasterios → universidades → institutos técnicos) y un
  **gran proyecto de transición**. Ejemplos: terminar la catedral, fundar la universidad,
  trazar la primera gran avenida, la llegada del ferrocarril, la primera red eléctrica.
- Quedarse atrás es una opción válida pero con costes: tus ciudadanos empiezan a mudarse a
  ciudades vecinas más avanzadas y el comercio regional pierde valor.

**Qué cambia con cada era**
- **Nuevos edificios, servicios y transporte**, y algunos se vuelven obsoletos (la muralla deja
  de ser útil con la artillería; los establos, con el coche).
- **Las expectativas de los ciudadanos**: en 1400 basta con un pozo cercano y un mercado; en
  1900 se exige agua corriente y en 1960, electricidad, coche y escuela secundaria.
- **Cómo se organiza la ciudad**: en la Edad Media, vivienda, taller y tienda comparten
  edificio (zona **mixta**, el artesano vive encima de su taller). Con la revolución industrial
  aparecen la fábrica y la separación clásica en residencial, comercial e industrial. En la era
  contemporánea vuelve el uso mixto. **El propio sistema de zonas evoluciona con la historia.**
- **Los problemas característicos**:

| Era | Problema central | Ejemplo de mecánica |
|---|---|---|
| Baja Edad Media | Defensa, abastecimiento, incendios y peste | La ciudad crece apretada dentro de la muralla; las casas de madera propagan el fuego; la peste viaja con el comercio y el agua sucia |
| Renacimiento | Crecer más allá de la muralla, agua | Arrabales sin protección fuera de los muros; acueductos y fuentes públicas |
| Barroco e Ilustración | Orden urbano y finanzas | Abrir avenidas a través del tejido medieval (expropiar) para mejorar el tráfico y el prestigio |
| Revolución industrial | Insalubridad y vivienda obrera | Las fábricas contaminan ríos y aire; el agua contaminada propaga epidemias; hay que construir alcantarillado |
| Siglo XX | Tráfico y expansión suburbana | Las autopistas cortan barrios y bajan el valor del suelo |
| Contemporánea | Vivienda, gentrificación, sostenibilidad | El casco histórico atrae turismo y encarece el alquiler; transición energética |

**La ciudad con memoria (el diferencial)**
Al cambiar de era, los edificios existentes **no desaparecen**. Para cada barrio el jugador elige:
- **Conservar** → *distrito histórico*: más turismo y valor cultural, pero limita la densidad.
- **Rehabilitar** → la estructura se mantiene con usos nuevos (un convento convertido en
  universidad, una fábrica de ladrillo convertida en lofts o en centro cultural).
- **Reurbanizar** → derribar y construir según la era actual: más densidad, pero pierdes
  patrimonio y puede haber protestas vecinales.

**El momento estrella: la muralla.** Durante siglos la muralla limita y protege la ciudad. En la
revolución industrial el jugador puede **derribarla y convertir su trazado en un bulevar de
ronda** que da paso a un ensanche planificado, como pasó en muchas ciudades europeas reales.
O conservar un tramo como monumento. Esa decisión marca la forma de la ciudad para siempre.

Resultado: dos ciudades nunca son iguales, y en la era contemporánea se puede leer la
historia de tu partida paseando por la calle: el casco medieval de calles tortuosas en torno
a la catedral, la plaza renacentista, la avenida barroca, la ronda que sustituyó a la muralla,
el ensanche industrial y los rascacielos.

**Infraestructura que evoluciona**
Las redes se heredan y se mejoran, no se reconstruyen desde cero: los caminos se empiedran,
luego se asfaltan; los pozos dan paso a fuentes, acueductos y red de tuberías; las brigadas de
cubos, a bomberos profesionales. Las decisiones antiguas pesan: las calles medievales
estrechas limitan el tráfico y la densidad que se podrá alcanzar después.

### 4.3 Carreteras y transporte
- **Trazado libre** (imprescindible para que la ciudad medieval tenga calles orgánicas):
  rectas, curvas, plazas e intersecciones que se generan automáticamente.
- El catálogo depende de la era: sendero y camino de tierra → calle empedrada → avenida
  barroca → calle adoquinada con vía de tranvía → calle asfaltada y autopista → calle
  compartida y peatonal.
- **El ancho de la vía define la densidad** que pueden alcanzar las zonas a su lado:
  callejón → baja, calle → media, avenida → alta.
- Transporte por agentes según la era: a pie, carreta, barca fluvial, carruaje, canal, tren de
  vapor, tranvía, metro, autobús, coche, bicicleta. Los peatones son un modo real, no decorativo.
- Mejora frente al referente: **rutas inteligentes**. Los agentes eligen la ruta según el
  tiempo estimado, la congestión y el modo de transporte, y cada ciudadano tiene un trabajo y
  una casa *asignados* (no "el más cercano libre").

### 4.4 Zonas y crecimiento
- Las zonas disponibles dependen de la era (ver 4.2): **mixta** (vivienda + taller + tienda) en
  las primeras eras; **residencial, comercial e industrial** a partir de la revolución
  industrial; **oficinas** y vuelta al **uso mixto** en la era contemporánea.
- Las parcelas se generan siguiendo la carretera (también en curvas) y pueden fusionarse
  para formar edificios grandes.
- Cada edificio tiene **riqueza** (baja/media/alta) y **nivel**. La mejora depende de cosas
  visibles: valor del suelo, servicios, educación y felicidad.
- **Panel "¿Por qué no crece?"**: al seleccionar una parcela se ve qué le falta. Es la regla
  de oro de la legibilidad.

### 4.5 Servicios por red
- **Los servicios viajan por la red de calles** (y más adelante por tuberías y cables). Si un
  barrio no conecta con la fuente, no recibe servicio.
- **Los servicios evolucionan con la era**: agua (pozos → fuentes y acueductos → red de
  tuberías), saneamiento (fosas → alcantarillado), energía (leña y carbón → gas → electricidad),
  seguridad (guardia de la ciudad → policía), salud (hospital de órdenes religiosas → hospital
  moderno), fuego (brigadas de cubos → bomberos).
- **Servicios con agentes**: aguadores, guardias, bomberos, médicos y basureros salen
  físicamente de su edificio y recorren la ciudad. Si las calles están colapsadas, llegan tarde.
- **Edificios modulares**: un monasterio se amplía con biblioteca y hospital; una estación de
  bomberos, con garajes; una central eléctrica, con generadores. Se crece por piezas en lugar
  de reconstruir.

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

1. **Progresión por eras** de la Baja Edad Media a la era contemporánea, con una ciudad que
   conserva su historia.
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

- **Solo la Baja Edad Media**: una era bien hecha vale más que seis a medias.
- Terreno con río, calles de trazado libre (sendero, calle empedrada) y una muralla
  ampliable.
- Zona mixta (vivienda + taller) que crece por demanda, mercado y catedral como gran proyecto.
- Agua por pozos y fuentes; incendios que se propagan entre casas de madera.
- Agentes (peatones, carretas) casa ↔ taller ↔ mercado con rutas que evitan la congestión.
- Capa de datos de tráfico, panel "¿por qué no crece?" y guardado local.
- Iluminación de día y noche con antorchas y ventanas, y una primera pasada de "realismo
  estilizado".
- Hito siguiente: la transición al Renacimiento, para validar la mecánica de conservar,
  rehabilitar o reurbanizar.

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
| Las primeras eras son lentas (poca tecnología, poco que hacer) | Ritmo más rápido en las eras tempranas, con eventos (ferias, asedios, epidemias) y la muralla como reto de planificación |
| El trazado libre y orgánico es caro de programar | Investigarlo pronto: es imprescindible para la ciudad medieval |
| Equilibrio entre eras (que una sea mucho más fácil o aburrida) | Cada era con un problema central propio (tabla 4.2) y pruebas de juego tempranas |
| Rendimiento de miles de agentes | Simulación por niveles de detalle; mover el núcleo a C#/C++ pronto |
| Que los agentes se perciban "tontos" | Asignación casa/trabajo estable y rutas por tiempo, con congestión |
| Alcance excesivo (región, multijugador) | La región y el cooperativo quedan para después del primer juego completo de una sola ciudad |

---

## 9. Preguntas abiertas

1. **Nombre definitivo:** pendiente. "City Sim 2027" queda como nombre provisional; conviene que
   el definitivo hable del tiempo o de la historia.
2. **Escala de los mapas:** ¿cuántos km² por ciudad?
3. **Parcelas con trazado libre:** la ciudad medieval lo hace casi obligatorio. ¿Aceptamos
   el coste extra o simplificamos (cuadrícula con calles curvas superpuestas)?
4. **¿Qué tradición arquitectónica?** ¿Europa occidental genérica, mediterránea (ibérica e
   italiana), centroeuropea…? ¿O el jugador elige una "cultura" al empezar?
5. **Profundidad del ciudadano:** ¿nombres e historias individuales o anonimato con datos?
6. **Monetización y plataforma:** ¿PC primero (Steam)? ¿precio único y DLC de estilos?
7. **¿Multijugador regional?** ¿Imprescindible o un extra a largo plazo?
8. **¿Cuántas eras en el lanzamiento?** ¿Las seis o, por ejemplo, de la Edad Media a la
   revolución industrial y el resto como expansión?
9. **¿Tiempo continuo o saltos?** ¿Una partida recorre los ~700 años, o cada era se juega como
   capítulo con un salto de tiempo y un resumen de "lo que pasó"?
10. **¿Las ciudades de una región pueden estar en eras distintas?** (Interesante para el
    comercio, pero difícil de equilibrar.)
11. **¿Guerra y asedios?** ¿La defensa medieval es una mecánica real (asedios) o solo
    ambientación? Propuesta: amenazas abstractas sin combate, para mantener el foco en
    la ciudad.
