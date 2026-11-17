# Nivel del río: mareas, luna, sudestadas e inundaciones

> Documento de sistema. Estado: **implementado en el motor** (`engine/src/city/water.rs`).

El nivel del Río de la Plata frente a la ciudad es la suma de:

| Componente | Magnitud | De dónde sale |
|---|---|---|
| Marea astronómica | ±0,15 m (muertas) a ±0,3 m (vivas) | Semidiurna (12 h 25 min) más una diurna; mareas vivas en luna nueva y llena |
| Viento | +0,5 a +2,2 m (sudestada), −0,4 a −1,2 m (pampero) | Temporales de 1 a 2 días; las sudestadas, más frecuentes de abril a noviembre |
| Caudal de los ríos Uruguay y Paraná | ±0,3 m durante meses | Años lluviosos (crecientes) y secos (bajantes); la sequía baja el río |

- **Luna**: un mes del calendario es una lunación. La fase decide las mareas vivas y muertas y
  la luz de la noche (en luna llena las calles se ven).
- **Inundaciones**: al pasar una sudestada, los solares por debajo del nivel que alcanzó el
  río (según el relieve real) se mojan y los ranchos de barro se pierden; las casas de material
  aguantan. Un temporal de lluvias hace crecer los arroyos e inunda los bajos de sus orillas.
- **Bajantes**: con pampero o sequía el agua se retira y quedan al descubierto los bancos de
  arena del fondo.
- **Pendiente**: datos reales de niveles (el mareógrafo del puerto de Montevideo y las
  estaciones de DINAGUA en el río Uruguay) para calibrar las magnitudes, y el río Uruguay en la
  escala país (crecientes de Salto, Paysandú y Fray Bentos).
