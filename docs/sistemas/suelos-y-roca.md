# Suelos, roca y transiciones del terreno

> Documento de sistema. Estado: **datos en el mapa** (`assets/mapas/montevideo/rocas.bin`);
> afloramientos y transiciones graduales en el motor: en curso.

Los recursos que salen de estos datos (productividad, aptitud para cultivo y pasturas, piedra,
arcilla, arena) y la carta geológica de DINAMIGE están en
[recursos-del-suelo.md](recursos-del-suelo.md).

## Fuente

- **Grupos de suelos CONEAT** (RENARE, MGAP), servicio ArcGIS del SNIG
  `MapasBase/Renare_Coneat` (polígonos con el código del grupo `SC` y su aptitud). El servicio
  no trae la descripción de cada grupo; el porcentaje de rocosidad sale de las descripciones
  publicadas de los grupos, resumido por zona (tabla `ROCOSIDAD_ZONA` del constructor). Es una
  aproximación: falta cruzarla con la descripción oficial de cada grupo.
- **Carta Geológica (DINAMIGE)**: desde noviembre de 2026 se usa el mapa base de unidades
  geológicas 2017 del geoportal del MIEM (`geoportal.miem.gub.uy/arcgis1091`), que sí responde;
  hoy alimenta los materiales (`materiales.bin`), no todavía los afloramientos de este documento.

## Qué dicen los datos en Montevideo

| Grupos CONEAT | Qué son | Roca |
|---|---|---|
| 4.2, 5.02b | Basamento cristalino del sur (granitos, gneises, anfibolitas) con suelos superficiales | Afloramientos en la franja costera: del Cerro a Punta Gorda y Carrasco |
| 10.x | Lomadas sobre los limos de la Formación Libertad | Sin roca |
| 03.x | Planicies bajas y bañados (Pantanoso, Miguelete, Carrasco) | Sin roca |
| 07.x | Arenas | Sin roca |

La zona urbana actual no tiene grupos CONEAT; ahí la roca sale solo de la costa (puntas de
piedra) y de la pendiente.

## En el juego

- **Afloramientos**: donde hay rocosidad, manchones de roca de unos metros (bochas y lajas)
  que asoman sobre el pasto, más altos en las laderas y las crestas, como en las sierras de
  Minas pero mucho menos densos. El umbral del ruido de las bochas sale de sus cuantiles
  medidos (media 0,5, desvío ≈ 0,11), así la fracción de suelo con roca a la vista coincide
  con la rocosidad CONEAT (14,5 % en las colinas del basamento del sur). La piedra tiene borde
  neto, pero el montículo sube en una rampa más ancha: cima de roca y faldón de pasto seco, sin
  escalones de más de 1,5 m por metro (lo verifica `mapa_real_de_montevideo`).
- **Transiciones**: entre pasto verde, seco y húmedo, una banda mezclada de varios metros en
  lugar de un borde neto.
