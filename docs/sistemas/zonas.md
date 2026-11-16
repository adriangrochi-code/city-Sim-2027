# Zonas: residencial, comercial e industrial

> Documento de sistema. Complementa al [GDD](../GDD.md) y a
> [políticas y recursos](politicas-y-recursos.md). Estado: **implementado en el motor**
> (`engine/src/city/lots.rs`, demanda en `engine/src/city/mod.rs`); el prototipo de Godot
> todavía tiene una sola zona mixta.

## Tipos y densidades

Como en el género, el jugador pinta solares de tres tipos, cada uno en densidad baja, media o
alta. La densidad fija el **primer y el último nivel** que puede construirse:

| Densidad | Niveles | Regla |
|---|---|---|
| Baja | 1 → 2 | Una planta. No pasa de casa de azotea, tienda o curtiembre |
| Media | 1 → 3 | Empieza modesto y llega a dos plantas |
| Alta | 2 → 3 | No empieza con ranchos ni galpones de paja; necesita calle empedrada. Las casas de renta alojan más gente |

Edificios de la época colonial (todos caben en un solar de 12 × 38 m):

| Nivel | Residencial | Comercial | Industrial |
|---|---|---|---|
| 1 | Rancho (4 hab.) | Pulpería (3 empleos) | Taller de herrero o carpintero (6 empleos) |
| 2 | Casa de azotea (8; 10 en alta) | Tienda con toldo y mercadería en la vereda (6) | Curtiembre y jabonería, con pozas y tendederos (12) |
| 3 | Casa de altos (14; 20 en alta, de renta) | Almacén de altos con depósito (12) | Barraca de cueros con dos galpones (24) |

En épocas siguientes la densidad alta es la que se transforma: conventillos y edificios de
renta (época 3), propiedad horizontal (época 5), torres (época 6).

## Demanda

Tres barras (R, C, I) de −100 % a +100 % en el cajón de Zonas y en cada tarjeta:

- **Residencial**: familias que esperan casa y empleos sin cubrir. Con desocupación baja.
- **Comercial**: una tienda cada tantos vecinos (unos 8 empleos de comercio por cada 100
  habitantes). La alcabala alta la frena.
- **Industrial**: brazos sin trabajo y la venta de cueros al exterior. La aduana alta la frena.

Las casas crecen con los pobladores que llegan; comercios y talleres crecen solo con demanda
positiva. Con más empleos que trabajadores llegan más familias (+20 %); con mucha desocupación,
menos (−25 %).

## Efectos

- **Industria molesta**: cada taller a menos de 70 m baja el valor del suelo de casas y
  comercios (−8 % cada uno) y las casas vecinas crecen la mitad de rápido.
- **Economía**: los talleres producen ladrillos, sebo y cueros; los comercios multiplican la
  alcabala (sin tiendas, la ciudad casi no cobra ventas) y todos pagan contribución según su
  nivel.
- **Rezonificar**: cambiar de tipo derriba lo construido; cambiar solo la densidad lo conserva,
  salvo que el edificio quede por encima del nuevo tope.
