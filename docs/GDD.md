# City Sim 2027: documento de diseño (v0.5)

> Estado: **borrador de visión (v0.6: estética rioplatense, amenazas naturales y flujos del puerto)**. Este
> documento define la intención, la estética y las mecánicas base *antes* de escribir código.
> Todo está abierto a discusión; las preguntas pendientes están en la sección 9.

---

## 1. Intención

**Un constructor de ciudades que se sienta vivo.** Es un sucesor espiritual de los
constructores de ciudades basados en agentes de principios de los 2010: la ciudad se entiende
*viéndola funcionar*. Cada carreta, tranvía, ómnibus o vecino es un agente con un origen y un
destino reales, no un número en una hoja de cálculo.

Recuperamos lo que hizo especial a ese estilo de juego y corregimos lo que lo frustró:

| Lo que conservamos | Lo que corregimos |
|---|---|
| Simulación por agentes legible: ves por qué pasan las cosas | Agentes "tontos" (todos al trabajo más cercano, rutas idénticas, atascos absurdos) |
| Servicios que viajan por la red de calles | Parcelas diminutas que obligan a jugar en un espacio pequeño |
| Calles curvas y zonas que siguen la calle | Conexión obligatoria a internet; nada de jugar sin conexión |
| Densidad determinada por el tipo de vía | Sin control del terreno (no se podía modelar el paisaje) |
| Especialización de ciudades y región compartida | Región en la que las ciudades vecinas apenas importaban de verdad |
| Edificios de servicio modulares (ampliables por piezas) | Progresión opaca: no sabías qué faltaba para que algo creciera |
| Capas de datos visuales (tráfico, contaminación, crimen…) | Interfaz recargada y poca accesibilidad |

**En una frase:** *"La ciudad que ves es la simulación que ocurre."*

### El giro: una ciudad rioplatense que atraviesa la historia
La partida empieza en el **siglo XVIII con una plaza fuerte colonial amurallada** en una
península junto a un gran río, y evoluciona por épocas hasta la **ciudad contemporánea**.
La identidad visual es **rioplatense en todas las épocas**: la luz del Río de la Plata, las
azoteas, los adoquines, los plátanos en las veredas, la rambla frente al agua. Lo construido
antes sigue ahí: **la ciudad final muestra en sus capas toda su historia**, como Montevideo,
Colonia del Sacramento o Buenos Aires (ver sección 4.2).

**Sin guerras ni combates.** Las únicas amenazas son **naturales** (temporales, sudestadas,
crecidas, sequías) y **los cambios del mundo exterior que llegan por el puerto**: guerras
lejanas, crisis y olas migratorias que alteran lo que entra y sale, y a los que la ciudad
tiene que adaptarse (sección 4.10).

Que el escenario sea rioplatense es también un diferencial: casi todos los constructores de
ciudades se ambientan en Norteamérica o Europa.

### Público
- Jugadores de constructores de ciudades que echan de menos la sensación "de juguete vivo",
  agente a agente, pero quieren mapas más grandes y más profundidad.
- Jugadores nuevos que buscan algo bonito y relajante, con curva de aprendizaje suave.
- Público rioplatense y latinoamericano que rara vez ve su propia arquitectura en un juego.

### Propiedad intelectual y lugares reales
El proyecto es **original**: nombre, edificios, interfaz, música y marca propios. Tomamos el
*género* y el *tipo de mecánicas* como inspiración, no recursos, nombres ni el aspecto
reconocible de ningún juego existente. La ciudad es **ficticia pero inspirada en el Río de la
Plata** (Uruguay como referencia principal, con guiños de toda la región): los edificios se
diseñan "a la manera de" la arquitectura rioplatense, sin copiar edificios reales concretos. "City Sim 2027" es un nombre provisional.

---

## 2. Pilares de diseño

Cada idea nueva se compara con estos seis pilares. Si no refuerza al menos uno, se descarta.

1. **Legible:** cualquier problema de la ciudad se puede *ver* y rastrear hasta su causa con
   un par de clics (un atasco → de dónde vienen esas carretas u ómnibus → a dónde van).
2. **Vivo:** la ciudad se mueve, cambia con la hora y la estación y reacciona a tus decisiones
   en segundos, no en meses de juego.
3. **Bonito a cualquier distancia:** tanto a ras de vereda como en la vista de toda la región,
   cada encuadre debe poder servir de captura de pantalla.
4. **Interconectado:** las ciudades de una región se necesitan entre sí (puerto, campo,
   trabajadores, servicios compartidos, turismo).
5. **Respetuoso con el jugador:** sin conexión obligatoria, guardado libre, deshacer, pausa
   siempre disponible y herramientas de modding desde el principio.
6. **Con memoria:** el paso del tiempo deja huella. Nada se "reemplaza" sin más: se conserva,
   se reforma o se derriba, y cada opción tiene consecuencias.

---

## 3. Dirección estética

### 3.1 Concepto: *"realismo estilizado" rioplatense*
Base **3D realista** (proporciones, materiales y luz creíbles) con un toque cuidado de
**maqueta o diorama**: la ciudad debe parecer una miniatura perfecta que cobra vida.
Más cálida y limpia que el realismo fotográfico; más sofisticada que el low-poly.

Referencias de *sensación* (no de copia): fotografía tilt-shift de ciudades reales,
maquetas de arquitectura, fotografía antigua de Montevideo, Colonia y Buenos Aires, y los atardeceres sobre
el Río de la Plata.

### 3.2 La identidad rioplatense (constante en todas las épocas)
Lo que hace que la ciudad se reconozca como rioplatense, sea 1750 o 2020:

- **El río como horizonte.** Un estuario enorme, de agua color *león* (marrón dorado), que
  parece mar. La costa y la futura **rambla** son el gran paisaje del juego.
- **La luz rioplatense:** cielos amplios, nubes bajas que corren con el viento, atardeceres
  rosados y anaranjados sobre el agua, inviernos grises y húmedos, niebla sobre el río.
- **Azoteas planas:** cubiertas transitables, con tanques de agua, ropa tendida y antenas
  según la época. Vistas desde arriba, son la "textura" de la ciudad.
- **Fachadas a la calle, sin retiro**, con **balcones de hierro**, **persianas de madera**
  (verdes, marrones, grises) y **puertas con zaguán**.
- **Adoquines** de granito y **veredas de baldosa**.
- **Árboles:** **plátanos** que forman túneles verdes en las calles, **jacarandás** que tiñen
  todo de violeta en primavera, **palmeras** en plazas, el **ombú** solitario en el campo.
- **Colores de fachada:** pasteles desgastados por la humedad (rosa viejo, ocre, celeste,
  amarillo pálido), piedra y revoque gris; nunca saturados.
- **Vida en la calle:** ferias vecinales, mate en la plaza, el humo de la parrilla en el
  mercado del puerto, tambores en las llamadas de carnaval.
- **El clima tiene carácter:** temporales con viento **pampero**, días de calor húmedo en
  verano y frío gris en invierno.

### 3.3 Reglas visuales
- **Luz como protagonista:** ciclo día/noche completo. De noche, las ventanas se iluminan
  según la ocupación real del edificio (la luz *es* un dato de la simulación).
- **Paleta:** colores naturales y algo desaturados en el terreno y los edificios. Los colores
  saturados se reservan para la información (zonas, alertas, capas de datos), así la interfaz
  nunca compite con la ciudad.
- **Profundidad de campo dinámica:** sutil desenfoque tilt-shift al alejar la cámara
  (opcional); nítido a ras de vereda.
- **Estaciones y clima** con efecto en la simulación: temporales que afectan al puerto,
  calor que aumenta el consumo, jacarandás florecidos en primavera, plátanos sin hojas en
  invierno.
- **Detalle por capas (LOD):** a distancia leemos la forma y el color de los barrios; de cerca
  aparecen rejas, macetas, bicicletas, vecinos y faroles.
- **Variedad con coherencia:** el estilo de cada edificio lo marca **la época en que se
  construyó** (ver 3.6). Los edificios se generan por módulos (planta baja + cuerpo +
  coronación) para evitar repeticiones evidentes.
- **Animación de crecimiento:** los edificios no aparecen de golpe: andamios, grúas y obra.
  Al mejorar de nivel, se ve la reforma.

### 3.4 Interfaz
- **Minimalista y contextual:** una barra inferior con herramientas y paneles que solo
  aparecen cuando hacen falta.
- **El marco evoluciona con la época:** papel y tinta de plano colonial al principio, tipografía
  art déco en los años 30, paneles translúcidos modernos al final.
- **Capas de datos como "modo lente":** al activar una capa, la ciudad se desatura y solo el
  dato se colorea (tráfico, agua, contaminación que se dispersa con el viento).
- **Accesibilidad:** paletas aptas para daltonismo, escala de interfaz, subtítulos y
  controles reasignables.

### 3.5 Sonido
Paisaje sonoro que depende de la cámara y de la simulación: gaviotas y barcos en el puerto,
carretas sobre el adoquín, campanas, tranvías, ómnibus, ferias y tambores de candombe a lo
lejos. La música cambia con la hora, el ánimo de la ciudad y **la época**: guitarra criolla y
música colonial al principio, milonga, **tango** en la época de la inmigración, candombe y murga
en carnaval, y una mezcla acústica y electrónica suave en la época contemporánea.

### 3.6 Estética por época

| Época | Periodo aprox. | Arquitectura | Calle y transporte | Luz nocturna | Paleta |
|---|---|---|---|---|---|
| **1. Plaza fuerte colonial** | 1720–1830 | Murallas y ciudadela de piedra, casas bajas de azotea encaladas con rejas y patio, iglesia matriz, cabildo, pulperías | Tierra y empedrado, carretas de bueyes, a caballo, barcos de vela | Velas y faroles de sebo: casi a oscuras | Cal blanca, piedra, teja, verde de persianas |
| **2. Ciudad Nueva** | 1830–1870 | Derribo de la muralla, ensanche en damero, neoclásico, teatro, casas quinta con jardín en las afueras | Adoquín, diligencias, carruajes, primeros vapores | Faroles a gas: ámbar | Ocres, blanco, gris piedra |
| **3. Inmigración y Belle Époque** | 1870–1930 | Casa de patio con zaguán y fachada italianizante, balcones de hierro, art nouveau, conventillos, palacios eclécticos, puerto moderno | Tranvía de caballos y luego eléctrico, ferrocarril, puerto con grúas | Gas e incandescencia | Rosa viejo, amarillo pálido, celeste, hierro negro |
| **4. Art Déco y la Rambla** | 1930–1960 | Edificios art déco y primeros "rascacielos" de la ciudad, chalets de balneario, la **rambla** costera, estadio | Ómnibus, autos de época, tranvía en retirada | Incandescente cálida, primeros neones | Crema, ocre, verde agua, gris granito |
| **5. Modernismo y propiedad horizontal** | 1960–1990 | Torres de apartamentos frente a la costa, hormigón, **cooperativas de vivienda**, ladrillo visto | Ómnibus, coches, avenidas anchas | Vapor de sodio: naranja | Ladrillo, hormigón, beige |
| **6. Contemporánea** | 1990–hoy | Torres de vidrio, rehabilitación del casco histórico, parques lineales, aerogeneradores en el horizonte | Ómnibus eléctricos, bicisendas, peatonales | LED blanco | Neutros con acentos de color, mucho verde |

Notas:
- La atmósfera evoluciona: el humo de fogones y saladeros en la época colonial, hollín de
  vapores y frigoríficos en la industrial, aire más limpio (o no, según tus decisiones) después.
- La noche cambia muchísimo: en la época colonial la ciudad está casi a oscuras; en la
  contemporánea, la rambla es una línea de luz que dibuja la costa.
- Filtro de color *opcional* por época (tono de grabado colonial, sepia de fotografía antigua,
  color de postal de los 50…) que refuerza la sensación de época sin estropear la legibilidad.

---

## 4. Mecánicas base

### 4.1 Bucle principal
```
Construir infraestructura → Zonificar → Llegan agentes (vecinos, comercios, industrias)
      ↑                                              ↓
Ajustar (impuestos, servicios, políticas) ← Observar problemas y oportunidades
```
Por encima de este bucle hay uno largo: **prepararse para la siguiente época** (sección 4.2).

Ritmo objetivo: cada 5–10 minutos, el jugador debería encontrar un problema nuevo que
*entender* (no solo un número en rojo).

### 4.2 Épocas: la mecánica central de progresión
Las épocas sustituyen al típico sistema de "desbloqueo por población".

**Cómo se avanza**
- El calendario avanza solo, pero **el paso de época lo decide el jugador** cuando cumple unos
  hitos: población, comercio del puerto, educación y un **gran proyecto de transición**.
  Ejemplos: la iglesia matriz, el derribo de la muralla, el puerto moderno y el ferrocarril,
  la rambla, la primera ley de vivienda, la rehabilitación del casco histórico.
- Quedarse atrás es una opción válida pero con costes: los vecinos se mudan a ciudades
  vecinas más avanzadas y el comercio regional pierde valor.

**Qué cambia con cada época**
- **Nuevos edificios, servicios y transporte**, y otros se vuelven obsoletos (la muralla deja
  de tener sentido tras la independencia; el tranvía, con el ómnibus).
- **El mundo exterior**, que se percibe por el puerto, cambia con cada época: qué se exporta,
  qué se importa, quién llega en los barcos (ver 4.10).
- **Las expectativas de los vecinos**: en 1750 basta con un aljibe y una pulpería cerca; en
  1900 se exige agua corriente; en 1960, ómnibus, liceo y electricidad.
- **Cómo se organiza la ciudad**: en la época colonial, vivienda, tienda y taller comparten
  edificio (zona **mixta**: la casa con la pulpería en la esquina). Con la industria llega la
  separación en residencial, comercial e industrial. En la época contemporánea vuelve el uso
  mixto. **El propio sistema de zonas evoluciona con la historia.**
- **Los problemas característicos**:

| Época | Problema central | Ejemplo de mecánica |
|---|---|---|
| Colonial | Agua, abastecimiento, límites de la muralla, contrabando | La ciudad crece apretada dentro de la muralla; el agua depende de aljibes y aguadores; el puerto da riqueza pero atrae contrabando |
| Ciudad Nueva | Crecer fuera de la muralla | Trazar el ensanche; conectar las quintas de las afueras; alumbrado a gas |
| Inmigración | Vivienda obrera e higiene | Llegan barcos de inmigrantes; si no hay vivienda, aparecen conventillos; epidemias por agua contaminada; hay que construir saneamiento |
| Art Déco y Rambla | Costa y movilidad | Construir la rambla gana costa y turismo pero cuesta muchísimo; el ómnibus reemplaza al tranvía |
| Modernismo | Densidad costera y crisis | Las torres en la costa suben el valor del suelo y vacían el centro; las cooperativas de vivienda como alternativa |
| Contemporánea | Vivienda, turismo, sostenibilidad | El casco histórico atrae turismo y encarece el alquiler; transición a energías renovables |

**La ciudad con memoria (el diferencial)**
Al cambiar de época, los edificios existentes **no desaparecen**. Para cada barrio el jugador
elige:
- **Conservar** → *casco histórico*: más turismo y valor cultural, pero limita la densidad.
- **Rehabilitar** → la estructura se mantiene con usos nuevos (una casa de patio convertida en
  hostel, una barraca del puerto convertida en centro cultural).
- **Reurbanizar** → derribar y construir según la época actual: más densidad, pero se pierde
  patrimonio y puede haber protestas vecinales.

**El momento estrella: la muralla.** Durante la época colonial la muralla es el límite de la
ciudad y el control de lo que entra (aduana, impuestos al comercio), y frena su crecimiento.
No hay asedios: su papel es urbano y económico. Para pasar a la segunda época, el jugador decide **cómo derribarla**:
dónde abrir la nueva plaza de acceso, qué tramo conservar como monumento (por ejemplo, una
puerta de la ciudadela) y cómo trazar el ensanche. Esa decisión marca la forma de la ciudad
para siempre.

Resultado: dos ciudades nunca son iguales, y en la época contemporánea se lee la historia de
la partida paseando: el casco colonial de la península, la puerta que quedó de la muralla, el
ensanche en damero, las casas de patio de los inmigrantes, los edificios art déco, la rambla
y las torres frente al río.

**Infraestructura que evoluciona**
Las redes se heredan y se mejoran, no se reconstruyen desde cero: el empedrado se adoquina y
después se asfalta; los aljibes dan paso a la red de agua; los faroles de sebo, al gas y a la
electricidad; las vías del tranvía quedan bajo el asfalto (o se recuperan). Las decisiones
antiguas pesan: las calles coloniales estrechas limitan la densidad y el tráfico posteriores.

### 4.3 Calles y transporte
- **Trazado libre con curvas amplias (decidido).** Las calles se dibujan como curvas suaves y
  no como casillas de una cuadrícula. Las herramientas siguen el modelo clásico del género,
  porque funciona y los jugadores ya lo conocen:

  | Herramienta | Cómo se usa |
  |---|---|
  | **Recta** | Clic en el inicio y clic en el final |
  | **Curva** | Clic en el inicio y en el final; luego se mueve el ratón para **doblarla** desde un punto de control y un tercer clic la fija |
  | **Arco** | Tres puntos que definen un arco de circunferencia (ideal para seguir la costa o una cuchilla) |
  | **Círculo** | Centro y radio: rotondas y plazas |
  | **Rectángulo / cuadrícula** | Arrastrar un área para crear una manzana o una retícula de calles (el damero colonial y el ensanche) |
  | **Mejorar vía** | Pasar sobre una calle existente para ensancharla o cambiarla de tipo sin perder lo que hay alrededor |

  Reglas de las curvas:
  - **Radio mínimo según el tipo de vía**: una calle colonial admite curvas cerradas; una
    avenida, la rambla o el ferrocarril exigen curvas amplias. Si la curva es demasiado
    cerrada, la vista previa se pone en rojo y muestra el radio mínimo.
  - **Ajuste y guías**: al dibujar aparecen guías para ir en paralelo, en perpendicular o a la
    distancia ideal de otra calle (la que deja manzanas de fondo justo para las parcelas).
    Las guías se pueden desactivar.
  - **Intersecciones automáticas**: al cruzar o tocar otra calle se crea el cruce. Dos calles
    no pueden cruzarse en un ángulo demasiado agudo.
  - **Vista previa con coste**: antes de confirmar se ve el trazado, el coste, la densidad que
    permitirá y los edificios que habría que derribar.
  - **Zonas pegadas a la calle**: las zonas se pintan con brocha a lo largo de la vía, a ambos
    lados, y las parcelas se generan siguiendo la curva, con la fachada hacia la calle.
  - **Puentes** automáticos al cruzar arroyos (piedra → hierro → hormigón según la época).
- El catálogo depende de la época: camino de tierra → calle empedrada → calle adoquinada con
  vías de tranvía → avenida y bulevar → rambla costanera → calle asfaltada → peatonal y
  bicisenda.
- **El ancho de la vía define la densidad** que pueden alcanzar las zonas a su lado:
  calle → baja, calle ancha → media, avenida/bulevar → alta.
- Transporte por agentes según la época: a pie, carreta, a caballo, diligencia, barco, tranvía
  de caballos, tranvía eléctrico, ferrocarril, ómnibus, coche y bicicleta.
- Mejora frente al referente: **rutas inteligentes**. Los agentes eligen la ruta según el
  tiempo estimado, la congestión y el modo de transporte, y cada vecino tiene un trabajo y una
  casa *asignados* (no "el más cercano libre").

### 4.4 Zonas y crecimiento
- Las zonas dependen de la época (ver 4.2): **mixta** (vivienda + comercio + taller) al
  principio; **residencial, comercial e industrial** a partir de la inmigración y la
  industria; **oficinas** y vuelta al **uso mixto** en la época contemporánea.
- Las parcelas se generan siguiendo la calle (también en curvas) y pueden fusionarse
  para formar edificios grandes.
- Cada edificio tiene **riqueza** (baja/media/alta) y **nivel**. La mejora depende de cosas
  visibles: valor del suelo, cercanía al río y la rambla, servicios, educación y felicidad.
- **Panel "¿Por qué no crece?"**: al seleccionar una parcela se ve qué le falta. Es la regla
  de oro de la legibilidad.

### 4.5 Servicios por red
- **Los servicios viajan por la red de calles** (y más adelante por cañerías y cables). Si un
  barrio no conecta con la fuente, no recibe servicio.
- **Los servicios evolucionan con la época**: agua (aljibes y aguadores → red de agua
  corriente), saneamiento (pozos negros → saneamiento), energía (sebo y leña → gas →
  electricidad → eólica), seguridad (guardia de la ciudadela → policía), salud (hospital de
  caridad → hospital moderno), fuego (vecinos con baldes → bomberos).
- **Servicios con agentes**: aguadores, serenos, bomberos, médicos y recolectores salen
  físicamente de su edificio y recorren la ciudad. Si las calles están colapsadas, llegan tarde.
- **Edificios modulares**: el hospital se amplía con pabellones; el puerto, con muelles y
  grúas; la usina, con generadores. Se crece por piezas en lugar de reconstruir.

### 4.6 Vecinos (simulación por agentes)
- Cada vecino tiene casa, trabajo o escuela, nivel educativo, riqueza, origen (criollo,
  inmigrante italiano o español…) y estado de ánimo.
- Se puede **seguir a cualquier vecino**: ver su rutina, su trayecto y sus quejas.
- **Vida de barrio**: la **feria vecinal** semanal ocupa una calle y atrae compradores; el
  **carnaval** y las **llamadas** recorren ciertos barrios; el fútbol llena el estadio. Son
  eventos con agentes reales que afectan al tráfico y a la felicidad.
- La simulación funciona por niveles de detalle: los agentes cercanos a la cámara se simulan
  completos y los lejanos de forma agregada. Así los mapas pueden ser mucho más grandes.

### 4.7 Economía y política
- Presupuesto con impuestos por zona y riqueza, mantenimiento y préstamos.
- **El puerto** es el motor económico de las primeras épocas: exporta cueros y tasajo de los
  **saladeros**, después carne de los **frigoríficos** y lana del campo.
- **El campo** (estancias en el borde del mapa y la región) abastece de ganado, cueros y lana.
- **Políticas** con efectos y costes claros (alumbrado público, educación gratuita, ley de
  vivienda cooperativa, peatonalización, energías renovables).
- **Especialización de la ciudad**: puerto y comercio, industria cárnica, turismo y balnearios,
  cultura y educación, tecnología y servicios. Cada una desbloquea edificios propios y cambia
  el aspecto de la ciudad.

### 4.8 Terreno y medio ambiente
- Mapas con **costa sobre el río**, penínsulas, bahías, arroyos, **cuchillas** (colinas suaves)
  y playas.
- Herramientas de modelado del terreno (con coste), incluido **ganar terreno al río** para la
  rambla o el puerto.
- La contaminación del aire, de los arroyos y de las playas se propaga de forma visible; una
  playa contaminada se cierra y baja el turismo.

### 4.9 Región
- Varias ciudades en un mapa regional: la ciudad portuaria, pueblos del interior, balnearios
  en la costa. **Un jugador o cooperativo asíncrono opcional**, siempre jugable sin conexión.
- Comercio entre ciudades (ganado del interior → frigoríficos de la capital → puerto), vecinos
  que viajan entre ellas y **grandes proyectos regionales** (ferrocarril central, puerto de
  aguas profundas, parque eólico).

### 4.10 Amenazas: la naturaleza y los flujos del puerto
**No hay guerras, asedios ni combates en la ciudad.** Las dificultades vienen de dos fuentes, y
el juego siempre avisa con antelación (el telégrafo, el diario, el parte meteorológico según la
época) para que el jugador pueda prepararse.

**1. Amenazas naturales**
| Amenaza | Efecto | Cómo se adapta la ciudad |
|---|---|---|
| **Sudestada** | Sube el nivel del río, inunda la costa baja y el puerto | Escolleras, rambla elevada, no construir en zonas bajas |
| **Temporal de pampero** | Viento fuerte: daños en techos y muelles, barcos que no pueden entrar | Puerto abrigado, refugios, mantenimiento |
| **Crecida de arroyos** | Inundación de barrios junto al cauce | Canalizar, parques inundables, reubicar viviendas |
| **Sequía** | El campo produce menos; falta agua en la ciudad | Reservas de agua, diversificar la economía |
| **Ola de calor** | Más consumo de agua y energía, playas llenas | Arbolado, parques, capacidad de red |
| **Niebla cerrada** | El puerto se paraliza unos días | Faros, señalización y después radar |

Los incendios y las epidemias **no son eventos al azar**: son consecuencia de cómo está
construida la ciudad (casas de madera, agua contaminada, falta de saneamiento) y se previenen
con servicios.

**2. Flujos del puerto: el mundo exterior**
El puerto es la **válvula** entre la ciudad y el mundo. Un panel del puerto muestra lo que entra
(mercancías, combustible, inmigrantes, capital) y lo que sale (cueros, carne, lana, granos).
Los acontecimientos lejanos, que nunca se ven pero se sienten, cambian esos flujos:

| Acontecimiento lejano | Qué cambia en el puerto | Cómo se adapta la ciudad |
|---|---|---|
| **Guerra en otro continente** | Sube la demanda de carne y cueros (auge exportador), pero escasean las importaciones | Ampliar frigoríficos y muelles; sustituir importaciones con industria local |
| **Crisis en Europa** | Llega una **ola de inmigrantes** | Vivienda rápida; si no hay, aparecen conventillos |
| **Bloqueo o cierre de rutas comerciales** | Cae el comercio y faltan combustible y maquinaria | Reservas, fuentes de energía locales, mercado interno |
| **Crisis económica mundial** | Cae el precio de lo que se exporta | Diversificar: turismo, servicios, industria |
| **Barcos más grandes** | Los buques ya no entran en el puerto viejo | Dragar, construir un puerto de aguas profundas, reconvertir los muelles viejos |
| **Epidemia en otro puerto** | Barcos en cuarentena | Lazareto, control sanitario, retrasos en el comercio |

La idea es que **el jugador no pelea contra el mundo, sino que se adapta a él**: una ciudad
diversificada aguanta mejor las crisis; una que depende solo de la carne vive auges y caídas.

Todas las amenazas son configurables (desactivables o más suaves en el modo sandbox).

### 4.11 Modos de juego
- **Carrera** a lo largo de las épocas, con objetivos y desbloqueos progresivos (tutorial
  integrado en la época colonial).
- **Sandbox** con todo desbloqueado y dinero ilimitado opcional.
- **Escenarios**: retos concretos (recuperar el casco histórico degradado, sobrevivir a una
  crisis, recibir una ola de inmigración).

---

## 5. Qué hace a este juego "más moderno"

1. **Progresión por épocas** de la colonia a la actualidad, con una ciudad que conserva su
   historia.
2. **Ambientación rioplatense**, rara en el género, y amenazas sin combate: naturaleza y
   cambios del mundo que llegan por el puerto.
3. **Calles curvas** con herramientas cómodas y parcelas que siguen la curva.
4. **Mapas grandes** gracias a la simulación con niveles de detalle.
5. **Rutas inteligentes** y transporte multimodal real.
6. **Legibilidad**: el panel "¿por qué?" está en todo, y se puede seguir a cualquier agente.
7. **Clima, estaciones y ciclo día/noche** con efecto en la simulación.
8. **Sin conexión obligatoria**; la región es local con cooperativo opcional.
9. **Modding desde el primer día**: edificios, estilos y políticas definidos en datos.
10. **Modo foto** y cámara a ras de vereda.

---

## 6. Alcance del primer hito jugable (cuando toque desarrollar)

Solo como referencia para el futuro. Todavía **no** se ha empezado.

- **Solo la época colonial**: una época bien hecha vale más que seis a medias.
- Una península sobre el río con puerto, calles curvas y en damero (herramientas de 4.3) y
  la muralla con su ciudadela.
- Zona mixta (casa de azotea con pulpería) que crece por demanda; la iglesia matriz como gran
  proyecto.
- Agua por aljibes y aguadores; incendios; el puerto y el saladero como economía.
- Agentes (vecinos a pie, carretas) casa ↔ trabajo ↔ puerto con rutas que evitan la congestión.
- Capa de datos de tráfico, panel "¿por qué no crece?" y guardado local.
- Iluminación de día y noche con la luz rioplatense, y una primera pasada de "realismo
  estilizado".
- Hito siguiente: la transición a la Ciudad Nueva con el **derribo de la muralla**, para
  validar la mecánica de conservar, rehabilitar o reurbanizar.

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
| Las épocas multiplican el arte necesario (×6) | Lanzar con 3–4 épocas y añadir el resto después; compartir volúmenes entre épocas y cambiar solo fachadas, cubiertas y detalles |
| Las primeras épocas son lentas (poca tecnología, poco que hacer) | Ritmo más rápido al principio, con eventos (llegada de barcos, temporales, contrabando) y la muralla como reto de planificación |
| El trazado libre con curvas es caro de programar (intersecciones, parcelas curvas, ajuste) | Es lo primero que se prototipa: calles por curvas de Bézier y arcos, parcelas generadas a lo largo de la curva |
| Que los agentes se perciban "tontos" | Asignación casa/trabajo estable y rutas por tiempo, con congestión |
| Ambientación demasiado local para el público internacional | La identidad se transmite por la estética y la vida de barrio, no por exigir conocer la historia; textos explicativos breves y opcionales |
| Alcance excesivo (región, multijugador) | La región y el cooperativo quedan para después del primer juego completo de una sola ciudad |

---

## 9. Preguntas abiertas

1. **Nombre definitivo:** pendiente. Conviene que hable del tiempo, del río o de la historia.
2. **Escala de los mapas:** ¿cuántos km² por ciudad?
3. **Detalle de las herramientas de calle:** ¿hacen falta curvas con dos puntos de control
   (forma de "S") además de la curva simple, o basta con encadenar curvas?
4. **Profundidad del vecino:** ¿nombres e historias individuales o anonimato con datos?
5. **Monetización y plataforma:** ¿PC primero (Steam)? ¿Precio único y expansiones?
6. **¿Multijugador regional?** ¿Imprescindible o un extra a largo plazo?
7. **¿Cuántas épocas en el lanzamiento?** ¿Las seis o, por ejemplo, de la colonia a la
   Belle Époque y el resto como expansión?
8. **¿Tiempo continuo o capítulos?** ¿Una partida recorre los ~300 años, o cada época se
   juega como capítulo con un salto de tiempo y un resumen de "lo que pasó"?
9. **¿Las ciudades de una región pueden estar en épocas distintas?** (Interesante para el
   comercio, pero difícil de equilibrar.)
10. **¿Los acontecimientos lejanos son históricos o inventados?** ¿Siguen una línea
    histórica aproximada (los auges exportadores, la gran inmigración, la crisis de 1929) o se
    generan al azar en cada partida?
