# Recursos, mercado y políticas

> Documento de sistema. Complementa al [GDD](../GDD.md) (secciones 4.5 y 4.7).
> Estado: **primera versión implementada en el prototipo** (época colonial) y **portada y
> ampliada en el motor** (`engine/src/city/economy.rs`: existencias por recurso, comercio por el
> puerto, impuestos ajustables y presupuesto mensual). Alcantarillas y alumbrado por épocas:
> diseñados, pendientes de implementar.

## 0. En el motor: existencias, comercio, impuestos y presupuesto

- **Existencias por recurso** (pan, carne, ladrillos, piedra, cueros, sebo) con capacidad de
  depósito; lo que no entra se pierde. El precio sale de cuántos meses de reserva quedan.
- **Comercio por el puerto**: con muelle y barcos, se importa lo que escasea (hasta mes y medio
  de consumo) y se exporta lo que sobra de tres meses de reserva; los cueros se embarcan todos.
  Un bloqueo lejano corta los barcos de ultramar.
- **Impuestos ajustables** (0 a 20 %): contribución de los vecinos (por casa, según su nivel),
  alcabala (ventas) y derechos de aduana. Por encima de la tasa habitual recaudan más pero
  enojan a su sector y espantan pobladores; la aduana alta además alimenta el contrabando.
- **Presupuesto mensual** línea por línea (ingresos y gastos), como el panel de presupuesto del
  género: contribución, alcabala, aduana, mantenimiento de calles y servicios, faroles, pósito,
  subsidio del pan, limpieza.

## Objetivo

Que el juego sea **gobernar una ciudad**, no solo construirla. La ciudad necesita comer, beber
y construir; esos recursos tienen **precio**, se pueden **acumular**, **escasean** con las
sequías y las guerras lejanas, y el jugador decide **políticas** (las *ordenanzas del cabildo*)
que conforman a unos vecinos y molestan a otros. Cada decisión es un dilema, no un botón de
"ganar".

## 1. Recursos de la época colonial

| Recurso | De dónde sale | Para qué se usa | Qué pasa si falta |
|---|---|---|---|
| **Pan** (trigo y harina) | Chacras privadas del entorno + atahonas | Comer: 1 ración por habitante | Sube el precio; los vecinos se enojan y llega menos gente |
| **Carne** | Estancias (y el tasajo de los saladeros) | Comer | Sube el precio |
| **Agua** | Pozos, aljibes, aguadores | Beber, apagar incendios, crecer | No se crece; más epidemias |
| **Ladrillos** | Hornos de ladrillos | Casas de azotea y de altos, iglesia | No se construye |
| **Cueros** | Estancias → saladeros → puerto | Exportar (aduana) | Menos ingresos |
| **Grano del pósito** | Lo que la ciudad compra y guarda | Reserva para años malos | — |

**Mercado**: cada mes se compara lo que hay (oferta) con lo que se necesita (demanda) y sale un
**precio** del pan y de la carne. El precio normal es 1,0; con escasez sube (hasta 3,0) y con
abundancia baja. El precio pesa en el ánimo de los vecinos más pobres.

**Sequía** (evento natural): durante unos meses las chacras rinden menos (−40 % de trigo) y las
fuentes de agua dan menos (−30 %). Es el momento en que las políticas importan de verdad.

## 2. Los tres sectores de la ciudad

La opinión se mide por sectores (0 a 100). Cada uno reacciona a cosas distintas:

| Sector | Le importa | Si está muy descontento (< 30) | Si está muy conforme (> 70) |
|---|---|---|---|
| **Vecinos** (artesanos, peones, pulperos) | Precio del pan y la carne, agua, faroles, epidemias | Protestas en la plaza; llega menos gente (−30 %) | Llegan más familias (+15 %) |
| **Comerciantes** (puerto, tiendas) | Comercio libre, aduana, barcos | Crece el contrabando: la aduana cobra −20 % | Más barcos y mercancías |
| **Hacendados** (estancias, saladeros) | Poder exportar, precio del cuero | Menos carretas de ganado y cueros | Más producción en el campo |

## 3. Ordenanzas del cabildo (época colonial, implementadas)

Se activan y desactivan desde la herramienta **Ordenanzas** (tecla K). Cada una tiene un costo
mensual (o un ingreso), efectos en la simulación y efectos en la opinión.

| Ordenanza | Efecto | Costo | Vecinos | Comerciantes | Hacendados |
|---|---|---|---|---|---|
| **Precio fijo del pan** | El pan no supera 1,3 aunque falte; la ciudad paga la diferencia | Subsidio según la escasez | +15 | −10 | 0 |
| **Pósito de granos** | La ciudad compra el trigo que sobra y lo reparte en la escasez | 40 pesos/mes + compras | +5 | −5 | +5 |
| **Prohibir exportar en la escasez** | Si falta comida, la carne no se embarca: más comida, menos cueros | Menos aduana | +10 | −10 | −15 |
| **Libre comercio con ultramar** | +30 % de barcos, aduana −30 %, menos contrabando | Menos aduana por barco | −5 | +15 | +10 |
| **Turnos de agua en la sequía** | Las fuentes alcanzan para un 30 % más durante la sequía | — | −5 | 0 | 0 |
| **Aljibe obligatorio en casas de altos** | Las casas de altos tienen agua propia (buena agua) | +10 ladrillos por casa de altos | 0 | −5 | −5 |
| **Prohibido techar con paja cerca de la plaza** | Ranchos a menos de 400 m de la plaza: fuego −70 % | — | −10 | +5 | 0 |
| **Limpieza de calles y basurales** | Las epidemias se contagian −40 % | 20 pesos/mes por cada 1.000 habitantes | +5 | 0 | 0 |
| **Faroles a cargo de los vecinos** | El mantenimiento de los puestos de serenos no lo paga la ciudad | Ahorro | −8 | 0 | 0 |

**Políticas sociales** (también ordenanzas, implementadas con la población; detalle y tabla en
[poblacion.md](poblacion.md), sección 7): reparto de solares a familias pobres, contribución
progresiva, pan barato para los pobres, hospital de caridad, escuelas de primeras letras,
préstamos del cabildo, fondo de manumisión, libertad de vientres y abolición (estas dos no se
derogan). La felicidad de la población de cada sector se suma a las causas de su opinión.

Reglas de diseño:
- **Ninguna ordenanza es gratis**: todas cuestan dinero o molestan a algún sector.
- **Las ordenanzas tienen sentido en un momento**: los turnos de agua solo actúan en la sequía; la
  prohibición de exportar solo en la escasez.
- **Se pueden revertir**, pero cambiar de opinión seguido cuesta: activar o desactivar una
  ordenanza tiene un costo único de 100 pesos (papeleo del cabildo).

## 4. Políticas en las épocas siguientes (diseño)

| Época | Ejemplos |
|---|---|
| 2. Ciudad Nueva | Cementerio fuera de la ciudad, alumbrado a gas obligatorio en la avenida, plan del ensanche |
| 3. Inmigración | Educación pública gratuita y obligatoria, reglamento de conventillos, jornada de ocho horas, concesiones ferroviarias |
| 4. Art Déco y la Rambla | Empresas públicas de energía y agua, protección de playas, feriados y deporte |
| 5. Modernismo | Ley de propiedad horizontal, cooperativas de vivienda, control de precios, presupuesto de emergencia |
| 6. Contemporánea | Alquiler protegido, vivienda social en el centro, energía renovable, prioridad al transporte público |

## 5. Alcantarillas y saneamiento (diseño, pendiente)

Las aguas servidas son la otra cara del agua: si no se sacan bien, **contaminan pozos, arroyos y
playas** y alimentan las epidemias.

| Época | Sistema | Mecánica |
|---|---|---|
| Colonial | Pozos negros en cada casa | Contaminan los pozos de agua cercanos según la densidad; las casas de altos los saturan antes |
| Ciudad Nueva | Zanjas y caños por las calles hacia el río | Primera red: hay que elegir dónde desagua (si es junto al puerto o a la playa, se nota) |
| Inmigración | Red de saneamiento con colectores | Red por debajo de las calles con capacidad por colector; sin ella, los conventillos enferman |
| Modernismo | Planta de tratamiento | Recupera las playas cerradas por contaminación |
| Contemporánea | Ampliación y separación de pluviales | Menos inundaciones con las lluvias fuertes |

Funciona como el agua y la electricidad del género: **viaja por la red de calles**, cada tramo
tiene capacidad, y el lugar de desagüe tiene consecuencias visibles (agua marrón, playas
cerradas, olor).

## 6. Alumbrado (diseño; la primera versión ya existe)

| Época | Sistema | Mecánica |
|---|---|---|
| Colonial | Velas, faroles de sebo y **serenos** (implementado) | Calles iluminadas: tertulias nocturnas y rondas |
| Ciudad Nueva | Faroles a **gas** con usina y cañerías por las calles | Más alcance, faroleros que los encienden; vida nocturna en la avenida |
| Inmigración | **Alumbrado eléctrico** | Primero en el centro; tranvías eléctricos y cafés abiertos de noche |
| Art Déco | Rambla iluminada, neones | Turismo nocturno |
| Modernismo | Vapor de **sodio** (luz naranja) | Avenidas; costo energético |
| Contemporánea | **LED** | Menos consumo, luz más blanca; contaminación lumínica como dilema |

Efectos del alumbrado: **vida nocturna** (comercio, tertulias, teatro), **seguridad** (menos
delitos y accidentes de noche, cuando exista ese sistema) y **costo** (mantenimiento o energía).
