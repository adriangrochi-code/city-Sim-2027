# Costa: playas de arena, puntas de piedra y restingas

Cada tramo de la costa del mapa tiene su **tipo natural** (antes de ramblas, muelles y rellenos),
la **fracción de roca de la orilla** y **hasta dónde se meten las piedras en el agua**. Los datos
están en `assets/mapas/montevideo/costa.json` (27 tramos de oeste a este y los islotes que el MDT
no tiene); la investigación con sus fuentes, en [`costa-fuentes.md`](costa-fuentes.md).

## Qué dicen los datos

- **Oeste (Espinillo → Punta Lobos)**: la costa más rocosa. Roca medida 0,58–0,77 entre Punta
  Yeguas y Punta Lobos (granito deformado y anfibolitas de la Formación Montevideo), con
  plataformas emergidas de 20–80 m.
- **Bahía**: arena y fango (la playa de la Aguada se rellenó en 1866–69); roca solo en el Cerro.
- **Ciudad Vieja**: rocosa antes de la ciudad; queda un banco de ~590 × 130 m frente a la Rambla Sur.
- **Este (Ramírez → Punta Mansa)**: arcos de arena entre puntas de gneis. Roca 0,1–0,35 en las
  playas y 0,82–0,95 en Trouville y la Punta del Buceo; piedras emergidas de 10–100 m.
- **Carrasco y Ciudad de la Costa**: arena (roca 0).
- **Isla de las Gaviotas**, frente a Malvín (~320 m de la costa, 2,3 ha de roca con vegetación).

Calidad: la fracción de roca está **medida** (polígonos `natural=bare_rock` de OpenStreetMap y
playas de la IDE y de la Intendencia) en la mayoría de los tramos de Montevideo y **estimada** en
el oeste y en los tramos hoy cubiertos por la rambla. El largo de las restingas sale de la
ortoimagen (**no** de sondeos): se usan solo las piedras emergidas, no las "costillas" oscuras
sumergidas que podrían ser algas.

## En el motor (`terrain.rs`)

- **Orilla**: lajas y bochas de piedra en la fracción de roca del tramo (manchas de 5–15 m con el
  umbral calibrado por cuantiles), que levantan 0,3–1,3 m sobre la arena y se apagan tierra
  adentro en 30–70 m.
- **Restingas**: en el agua, hasta el largo del tramo, lajas que suben del fondo hasta quedar
  entre medio metro bajo el agua y un metro sobre ella (algunas asoman en bajante), cada vez más
  salteadas mar adentro.
- **Islotes**: roca con pasto arriba, borde irregular.
- Sin dato del tramo: arena donde hay playa oficial, piedra donde no.
