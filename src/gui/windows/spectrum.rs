use crate::gui::{layout, model::AppModel};
use crate::spectrum::{SpectrumController, BYTES_PER_PIXEL, IMAGE_HEIGHT, IMAGE_WIDTH};
use gpui_kit::{
    component::{button::Button, Selectable},
    *,
};
use std::{sync::Arc, time::Duration};
pub struct SpectrumView {
    controller: SpectrumController,
    model: Entity<AppModel>,
    _observe: Subscription,
    pixels: Vec<u8>,
    generation: u64,
    image: Option<Arc<RenderImage>>,
    last_rendered: Option<Arc<RenderImage>>,
    wide: bool,
    frozen: bool,
    _tick: Task<()>,
}
impl SpectrumView {
    pub fn new(
        controller: SpectrumController,
        model: Entity<AppModel>,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe(&model, |_, _, cx| cx.notify());
        let executor = cx.background_executor().clone();
        let tick = cx.spawn(async move |this, cx| loop {
            executor.timer(Duration::from_millis(33)).await;
            if this
                .update(cx, |v, cx| {
                    if v.controller.copy_image(&mut v.pixels, &mut v.generation) {
                        v.image = Some(bgra_image(&v.pixels));
                    }
                    cx.notify();
                })
                .is_err()
            {
                break;
            }
        });
        Self {
            controller,
            model,
            _observe: observe,
            pixels: vec![0; IMAGE_WIDTH * IMAGE_HEIGHT * BYTES_PER_PIXEL],
            generation: 0,
            image: None,
            last_rendered: None,
            wide: true,
            frozen: false,
            _tick: tick,
        }
    }
}
pub fn bgra_image(pixels: &[u8]) -> Arc<RenderImage> {
    // RenderImage accepts BGRA bytes; converting channels would invert the heat map.
    let buffer =
        image::RgbaImage::from_raw(IMAGE_WIDTH as u32, IMAGE_HEIGHT as u32, pixels.to_vec())
            .expect("spectrum image dimensions");
    Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]))
}
impl Render for SpectrumView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.image.as_ref().map(|image| image.id)
            != self.last_rendered.as_ref().map(|image| image.id)
        {
            if let Some(old) = self.last_rendered.take() {
                let _ = window.drop_image(old);
            }
            self.last_rendered = self.image.clone();
        }
        let m = self.model.read(cx);
        let levels = self.controller.levels();
        let mut body = layout::surface(cx)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("wideband")
                            .label(m.text("広帯域"))
                            .selected(self.wide)
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.wide = true;
                                v.controller.set_wideband(true);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("narrowband")
                            .label(m.text("狭帯域"))
                            .selected(!self.wide)
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.wide = false;
                                v.controller.set_wideband(false);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("freeze")
                            .label(m.text("フリーズ"))
                            .selected(self.frozen)
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.frozen = !v.frozen;
                                v.controller.set_frozen(v.frozen);
                                cx.notify();
                            })),
                    ),
            )
            .child(format!(
                "RMS {:.1} dBFS     Peak {:.1} dBFS     Hold {:.1} dBFS",
                levels.rms_db, levels.peak_db, levels.hold_db
            ));
        if let Some(image) = &self.image {
            body = body.child(
                div().flex_1().min_h_0().child(
                    img(image.clone())
                        .w_full()
                        .h_full()
                        .object_fit(ObjectFit::Contain),
                ),
            );
        }
        body.child("8 kHz ↑ · 20 Hz ↓     −80 … 0 dBFS     ← history · now →")
    }
}

pub fn window_options() -> WindowOptions {
    layout::window_options("スペアナ / Spectrum", 840., 560., 640., 400.)
}

#[cfg(test)]
mod tests {
    use super::{bgra_image, BYTES_PER_PIXEL, IMAGE_HEIGHT, IMAGE_WIDTH};
    #[test]
    fn image_upload_preserves_bgra_colors_and_top_to_bottom_rows() {
        let mut pixels = vec![0; IMAGE_WIDTH * IMAGE_HEIGHT * BYTES_PER_PIXEL];
        pixels[..4].copy_from_slice(&[7, 13, 239, 255]);
        let last = (IMAGE_HEIGHT - 1) * IMAGE_WIDTH * 4;
        pixels[last..last + 4].copy_from_slice(&[211, 31, 17, 255]);
        let image = bgra_image(&pixels);
        assert_eq!(image.as_bytes(0).unwrap(), pixels);
    }
}
