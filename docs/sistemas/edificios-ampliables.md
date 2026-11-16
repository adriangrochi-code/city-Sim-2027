# Edificios ampliables y modernización

> Documento de sistema. Complementa al [GDD](../GDD.md) (secciones 4.2 y 4.5).
> Estado: **implementado en el prototipo 1** para los servicios, el campo, el puerto y la
> iglesia matriz.

## Principio

**En una ciudad real casi nunca se tira un edificio abajo: se lo amplía y se lo moderniza.**
El juego sigue esa lógica. Los edificios crecen por piezas que se ven en el modelo, y cuando
llega una tecnología mejor se transforman en el mismo lugar, conservando su historia. Demoler
sigue siendo posible, pero es la excepción, no el camino normal.

Es la versión moderna de los edificios modulares del referente del género, unida al pilar
**"con memoria"**: lo que ves en la época contemporánea es el resultado de muchas ampliaciones
y reformas sobre lo que se construyó en la colonia.

## Tres formas de cambiar un edificio

| Forma | Qué hace | Ejemplo colonial |
|---|---|---|
| **Ampliar** | Agrega un módulo: más capacidad, alcance o viajes. Se ve en el modelo. | Aljibe → segunda y tercera cisterna |
| **Modernizar** | Lo convierte en su versión mejor en el mismo lugar (puede crecer de tamaño). | Pozo de balde → aljibe público |
| **Construir por fases** | Grandes proyectos que avanzan fase a fase, con obra visible. | Iglesia matriz en 5 fases |

Las casas siguen un camino propio: crecen solas en el mismo solar (rancho → casa de azotea →
casa de altos) y, en épocas posteriores, se **rehabilitan** con usos nuevos (ver GDD 4.2).

## Cómo se usa

Con la herramienta **Inspeccionar** se hace clic en el edificio. El panel muestra su estado,
las ampliaciones que ya tiene y un botón por cada ampliación o modernización disponible, con su
costo. Los botones que no se pueden pagar aparecen desactivados; si algo impide la
modernización (por ejemplo, no hay lugar), el botón lo explica.

## Catálogo colonial (implementado)

| Edificio | Ampliaciones | Efecto | Modernización |
|---|---|---|---|
| Pozo de balde | Brocal de piedra con techito | +25 personas | → Aljibe público |
| Aljibe público | Segunda cisterna · Tercera cisterna (requiere la segunda) · Canaletas desde las azoteas | +120 / +120 personas · +50 m de alcance | — |
| Puesto de aguadores | Más carros y mulas · Noria en el arroyo | +200 personas y +3 carros · +250 personas y +300 m | — |
| Puesto de serenos | Más serenos · Faroles de vidrio en las esquinas | +2 serenos y +90 m · +80 m de calles iluminadas | — |
| Estancia | Segundo corral · Puesto de peones con mangrullo | +2 carretas de ganado por día (cada uno) | — |
| Saladero | Galpón de salazón nuevo · Grasería | +2 carretas de cueros por día (cada uno) | — |
| Puerto | Alargar el muelle · Grúa (requiere muelle largo) · Segunda barraca | +1 lancha (descarga más rápida) · +1 lancha · +25 % de aduana | — |
| Iglesia matriz | 5 fases: cimientos, muros, naves y techo, cúpula, torres | Más pobladores por fase | — |

Todas las ampliaciones se guardan con la partida.

## Modernización a lo largo de las épocas (diseño)

La regla: **cada edificio de una época tiene un sucesor natural en la siguiente**, y el jugador
puede modernizarlo en el lugar en vez de demolerlo. Lo que no se moderniza sigue funcionando,
pero queda atrás: rinde menos y sus vecinos lo valoran menos.

| Colonial | Ciudad Nueva | Inmigración | Siglo XX | Contemporánea |
|---|---|---|---|---|
| Pozo / aljibe | Aljibe con bomba | Toma de la red de agua corriente | Tanque de agua del barrio | Depósito y planta modernizada |
| Puesto de serenos | Faroles a gas | Comisaría | Comisaría con patrullas | Centro de seguridad |
| Muelle de madera | Muelle de piedra | Puerto moderno con diques (gran proyecto) | Terminal de carga | Terminal de contenedores y de cruceros |
| Saladero | Saladero a vapor | Frigorífico | Frigorífico ampliado | Reconversión: centro cultural o parque (rehabilitación) |
| Estancia | Estancia con alambrado | Estancia con tren cercano | Tambo y agricultura | Agroindustria y turismo rural |
| Iglesia matriz | Restauración | Reloj y luz eléctrica | Patrimonio | Patrimonio y turismo |

Cuando un edificio se moderniza, **conserva algo de su forma anterior** (la fachada, una
torre, la chimenea), así la ciudad sigue mostrando sus capas históricas.

## Encaje con el motor de vóxeles

El catálogo de estructuras del motor ya está armado con un kit modular por piezas (zócalo,
cuerpo, pretil, vanos, techos, elementos). Las ampliaciones de este documento se traducen
directamente en piezas extra del kit sobre el mismo solar, y la modernización en un cambio de
kit conservando parte de la estructura original. Ver
[estetica-voxel.md](estetica-voxel.md).
