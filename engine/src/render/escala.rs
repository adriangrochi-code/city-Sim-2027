//! [render] Escala de render (técnicas del mod LOD §2.16, `render/EscaladoFsr` + `PruebaEscalado`
//! del mod): el mundo se dibuja en una imagen más chica y se escala a la ventana; la interfaz se
//! dibuja después, a resolución completa.

/// Escalas posibles (% del lado de la ventana).
pub const ESCALAS: [u32; 4] = [100, 85, 75, 67];

/// La escala válida más cercana.
pub fn valida(e: u32) -> u32 {
    ESCALAS.iter().copied().min_by_key(|v| v.abs_diff(e)).unwrap()
}
