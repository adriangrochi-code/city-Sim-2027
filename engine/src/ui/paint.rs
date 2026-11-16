//! Dibujo de la interfaz (egui) sobre wgpu, encima de la escena ya dibujada.

pub struct UiPainter {
    renderer: egui_wgpu::Renderer,
}

impl UiPainter {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> UiPainter {
        UiPainter { renderer: egui_wgpu::Renderer::new(device, format, None, 1, false) }
    }

    /// Pinta la salida de egui en `view` (cargando lo que ya hay: la escena).
    #[allow(clippy::too_many_arguments)]
    pub fn paint(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, size: (u32, u32), ctx: &egui::Context, out: egui::FullOutput) {
        let ppp = out.pixels_per_point;
        let jobs = ctx.tessellate(out.shapes, ppp);
        for (id, delta) in &out.textures_delta.set {
            self.renderer.update_texture(device, queue, *id, delta);
        }
        let sd = egui_wgpu::ScreenDescriptor { size_in_pixels: [size.0, size.1], pixels_per_point: ppp };
        let extra = self.renderer.update_buffers(device, queue, enc, &jobs, &sd);
        if !extra.is_empty() {
            queue.submit(extra);
        }
        {
            let pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("interfaz"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.renderer.render(&mut pass.forget_lifetime(), &jobs, &sd);
        }
        for id in &out.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }
}
