# Clima, estaciones y el calendario del campo

> Documento de sistema. Estado: **implementado en el motor** (`engine/src/city/climate.rs`,
> viento en `engine/src/city/water.rs`). Datos en `assets/clima/`.

## Datos reales

- **Normales 1991–2020 de la estación Prado (Montevideo), INUMET**: temperatura media, máxima y
  mínima media, extremos absolutos, lluvia mensual y días con lluvia. Fuente: tablas
  estadísticas del Instituto Uruguayo de Meteorología
  (inumet.gub.uy → Clima → Estadísticas climatológicas). Copiadas en
  `assets/clima/montevideo_prado_1991_2020.json` con la fecha de consulta.
- **Viento medio mensual**: estación Prado, INUMET 1961–1990 (unos 3,3 a 4,2 m/s).
- **Época**: hacia 1726 (Pequeña Edad de Hielo) el clima era algo más fresco: se restan 0,5 °C.
- **Rosa de vientos**: aproximada (del este y noreste en verano; del norte y del sudoeste en
  invierno). Pendiente: reemplazarla por la frecuencia de direcciones de una estación oficial.

## Cómo se arma cada mes

| Qué | Cómo |
|---|---|
| Temperatura | Normal del mes + una anomalía que dura de un mes al otro (±1 °C) |
| Lluvia | Normal del mes × un factor lognormal (un mes puede tener la mitad o el doble) |
| Años lluviosos y secos | Regímenes de 1 a 3 años (como El Niño y La Niña): más o menos lluvia en primavera y verano |
| Agua del suelo | Balance de un balde de 120 mm: lluvia menos evapotranspiración de Thornthwaite. Por eso el verano queda seco aunque llueva parecido todo el año |
| Sequía | Dos meses seguidos con el suelo casi seco: baja el trigo, las fuentes y el río |
| Heladas | Con mínimas medias bajo 9 °C (junio a agosto); en Montevideo son pocas |
| Temporal de lluvias | Más del doble de la lluvia normal: crecen los arroyos e inundan los bajos |

## El calendario del campo (lo que se puede plantar)

| Cultivo | Siembra | Crece | Cosecha | Lo castiga |
|---|---|---|---|---|
| Trigo (el pan) | mayo a julio, con la tierra húmeda | invierno y primavera | diciembre (y se trilla en enero) | heladas en la espigazón (septiembre, octubre), seca de primavera, exceso de lluvia (roya) |
| Maíz | octubre y noviembre, con la tierra tibia | verano | marzo y abril | la seca de verano |
| Huertas (zapallo, porotos, hortalizas) | septiembre a noviembre | primavera y verano | diciembre a marzo | heladas tardías, seca |
| Pasturas del campo natural | — | primavera y otoño; poco en invierno | todo el año (carne, cueros, sebo) | frío y seca |

- El grano de la ciudad ya no llega igual todos los meses: entra con la cosecha y tiene que
  durar hasta la próxima. Por eso importan el pósito, el precio del pan y comprar por el puerto
  en los años malos.
- Los hornos de ladrillos producen menos con lluvia (el adobe no seca) y con frío.
- La ficha del clima (barra de arriba) dice qué es tiempo de sembrar y cómo vienen los cultivos.

## Viento

Dirección (de dónde viene) y velocidad en cada momento: cambia cada 3 a 12 horas según la rosa
de la estación y la velocidad media del INUMET. En una **sudestada** sopla del sudeste y crece
hasta temporal; el **pampero** entra del sudoeste con ráfagas. Mueve las olas del río y queda
listo para los incendios (el fuego avanza a favor del viento) y los barcos.
