# Modelos de detalle (micro-vóxeles de 1/16 m)

> Código: `engine/src/detalle/` (biblioteca, paletas, ropa, animación),
> `engine/src/render/shaders/agentes.wgsl` (dibujo). Personas y animales en
> `docs/sistemas/personajes-y-animales.md`.

## La regla de resolución

Lo que no necesita vóxeles chicos se queda como está: terreno de 0,25 m, edificios en módulos de
0,5 m, flora baja en micro-vóxeles de 0,125 m. Lo que queda mucho mejor con vóxeles más chicos
—personas, animales, carretas, faroles, toneles, brocales, botes, letreros— usa **modelos de
detalle** de **1/16 m (6,25 cm)**.

**Por qué 1/16 y no 1/12:**

- Es potencia de dos y cae en la misma grilla que todo lo demás: 4 por vóxel de terreno, 2 por
  micro-vóxel de flora, 8 por módulo de edificio. Las aristas de un modelo coinciden con las de
  la calzada y las de la flora (con 1/12 no coinciden nunca y se ve el desfase al lado de un
  cordón o un escalón).
- Una persona de 1,70 m son 27 vóxeles de alto: cabeza de 4×5×4, brazos de 2, sombrero con ala.
  Con 1/12 serían 20: ya no entran el ala del sombrero, las vueltas de la casaca ni la trenza.
- **El costo no depende del tamaño del vóxel**: cada modelo se pinta en una grilla y al armarse
  se funde en cajas (fusión voraz por parte y color). Lo que se dibuja son cajas, y su cantidad
  depende de cuántas piezas distintas tiene el modelo (≈ 25 por persona), no de cuántos vóxeles.
  Bajar a 1/32 solo costaría más cajas donde se pintara más detalle. Los vóxeles se ven igual:
  el shader le da a cada vóxel de 1/16 su matiz (±5 %) y un borde apenas más oscuro, que se
  apagan entre 25 y 70 m para que no titilen.

## Cómo se arma un modelo

`detalle::Lienzo` es una grilla 3D chica (por ejemplo 16 × 60 × 12 para una persona). Se pinta
por datos, en código, con lo último pintado encima de lo anterior:

- `caja(parte, ranura, a, b)`, `centrada(...)` (simétrica en x), `par(...)` (la misma caja a
  izquierda y derecha, para piernas, brazos y patas), `borrar(...)`;
- `cilindro_y` / `cilindro_x` (toneles, barricas, brocales redondos);
- `girada(...)`: una caja fuera de la grilla, girada alrededor del pivote de su parte (los rayos
  y la llanta de las ruedas, la lanza apoyada en el suelo); `rueda(...)` arma una rueda entera
  (llanta de n tramos, rayos y maza) con ~20 cajas giradas.
- `pivote(parte, punto)`: alrededor de qué gira cada parte.

`armar()` funde la grilla en cajas (`CajaM`: esquinas en micro-vóxeles, ranura, parte) y arma el
`Modelo`. Las cajas de una misma grilla nunca se pisan (prueba
`las_cajas_de_la_grilla_no_se_pisan_y_cubren_lo_pintado`).

**Ranuras de color** (`detalle::r`): piel, pelo, ropa, calzón, sombrero, capa, calzado, acento,
pelaje, pelaje oscuro, crin, madera, madera oscura, cuero, hierro, claro. El modelo no tiene
colores; cada instancia trae su `Paleta` (16 colores): la ropa de cada vecino, el pelaje de cada
animal, la pintura de cada bote (`modelos::paleta(id, semilla)`, `ropa::paleta`).

**Partes** (`Parte`): fijo, cabeza, brazos, piernas, cuatro patas, cola y dos ruedas. Cada una
gira alrededor de su pivote: giro alrededor de y (la cabeza que mira) e inclinación alrededor de
x (piernas, patas, la cabeza que baja a pastar, las ruedas).

## La biblioteca

| Modelo | Cajas | Notas |
|---|---|---|
| Caballo / ensillado | 38 / 51 | criollo de 1,40 m a la cruz; recado con pellón, cincha, estribos y cabezada; 7 pelajes |
| Vaca / overa / buey | 37 / 49 / 37 | ganado criollo con astas en lira; el buey más grande, con giba y astas largas; 5 pelajes |
| Oveja, perro, gallina | 18, 23, 17 | |
| Carreta de bueyes / estacionada | 217 / 213 | ruedas de 2,1 m de 12 rayos, toldo de cueros en arco con costuras, lanza y yugo; parada, la lanza en el suelo |
| Carro | 63 | dos ruedas, varas, bolsas y un barril, pescante |
| Bote | 130 | bote de remos de 5 m: proa aguda, bancadas y remos, casco pintado |
| Farol de aceite / de gas | 16 / 21 | colonial en poste con brazo; el de gas (columna de hierro con anillos y apoyo de escalera) queda listo para la época 2 |
| Tonel, barrica | 75, 49 | duelas y aros de hierro |
| Banco, palenque, cruz, veleta | 4, 3, 5, 15 | |
| Brocal de aljibe | 22 | mármol, arco de hierro forjado, roldana y balde |
| Pozo con roldana | 42 | brocal de piedra, horcones, travesaño, roldana, soga y balde |
| Letrero de pulpería | 20 | tabla pintada colgada de un brazo |
| Personas (19 atuendos × de pie / montado) | 13 – 66 | ver `personajes-y-animales.md` |

Toda la biblioteca: **2217 cajas, 43 KB**, armada una vez al primer uso (`detalle::biblioteca()`).

## Dibujo

Una instancia de **48 bytes** por caja (`detalle::Caja`): pivote en el mundo, giro, desde el
pivote hasta el centro del piso de la caja, inclinación, tamaño y color (el cuarto byte: 0 = caja
lisa de lejos; otro = micro-vóxeles con esa semilla de matiz). El vertex shader arma las 6 caras,
inclina la caja alrededor del pivote de su parte y la gira; luz del sol, sombras en cascadas
recibidas (`sombras.wgsl`), luz del cielo, niebla, curvatura y `curva_tonal` como el resto.
Mismo pipeline y mismo búfer que los vecinos (`Renderer::set_agents`), subido cada cuadro solo si
cambió.

**Sombra de contacto:** los modelos todavía no proyectan sombra en los mapas de cascadas (que se
guardan entre cuadros; redibujar la cascada cercana cada cuadro por los que se mueven costaría
más que todo el resto). Debajo de cada modelo va una elipse suave (una instancia con alto 0),
corrida hacia donde no da el sol y más tenue de noche, dibujada en una segunda pasada que solo
oscurece lo que ya hay (mezcla `dst × (1 − a)`, sin escribir profundidad).

## Detalles fijos: la API para los demás sistemas

```rust
use crate::detalle::{Detalles, Id};
let mut d = Detalles::default();
d.poner(Id::FarolAceite, Vec3::new(x, suelo, z), giro, semilla);   // base en el suelo, giro en rad (0 = +z)
d.quitar(|i| /* por ejemplo, los del solar demolido */ false);
d.cajas(centro_de_la_vista, &mut cajas);                          // los de menos de 140 m, con su sombra
```

`Detalles` guarda las instancias por zonas de 64 m; no se guarda con la partida (cada sistema lo
rehace desde su propio estado). La ciudad tiene uno general (`city.agentes.detalles`), que se
dibuja con los vecinos; la escena de la ciudad (pulperías, estancias, muelle, plaza) se deriva
sola en `agentes::fauna` y se rehace cuando cambia su firma (solares, servicios, año, clima).

**Para agregar un modelo:** una variante en `detalle::Id` (y en `Id::TODOS`), su función en
`detalle/modelos.rs` pintada en un `Lienzo`, su paleta en `modelos::paleta`. Las pruebas de
`detalle/tests.rs` revisan todos los modelos (cajas no vacías, ranuras válidas, pivotes adentro,
tamaños razonables).

## Gancho para los constructores de edificios (pendiente)

Los edificios se arman con vóxeles de 0,5 m (`structures/`); lo fino de las fachadas queda tosco
a esa escala: **rejas** de ventana, **postigos** y tableros, **macetas** en los balcones,
**faroles de fachada** (de brazo), el **cancel** de hierro del zaguán, aldabas y llamadores,
bancos de piedra junto a la puerta. La idea, sin tocar los constructores todavía:

1. El constructor, además de sus `Edits`, devuelve una lista de pedidos de detalle en las
   coordenadas de su `Frame` (u, y, w en módulos de 0,5 m + giro relativo al frente):
   `Vec<(detalle::Id, IVec3, f32 /* giro */, u32 /* semilla */)>`, por ejemplo una reja por
   cada vano de ventana o un farol de brazo junto a la puerta de las casas de nivel 3.
2. Al construir o mejorar el solar (`build_lot`), la ciudad pasa esos pedidos a mundo con el
   mismo `Frame` y los pone en un `Detalles` del solar (`Detalles::poner`); al demolerlo,
   `Detalles::quitar` los de ese solar. Como los edificios, se regeneran al cargar.
3. Los modelos nuevos (`Reja`, `Postigo`, `Maceta`, `FarolBrazo`, `Cancel`) se pintan como los
   de la biblioteca; la reja de hierro forjado de una ventana colonial son ~15 cajas.
4. Para no repetir la geometría ya hecha en vóxeles de 0,5 m, el vano queda vacío (o con el
   vidrio) y el detalle lo llena.

El punto de enganche ya existe (`Detalles` + `Id` + biblioteca); falta que `structures/` emita
los pedidos (lo está tocando la época 2).
