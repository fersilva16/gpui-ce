#[cfg(not(target_os = "macos"))]
use gpui::HeadlessAppContext;
#[cfg(target_os = "macos")]
use gpui::VisualTestAppContext;
use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Element, ElementId, ElementTransform,
    GlobalElementId, InspectorElementId, IntoElement, LayoutId, ParentElement as _, Pixels, Render,
    Styled as _, Window, div, point, px, rgb, white,
};
use std::borrow::Cow;

use gpui_fonts::IBM_PLEX;

const WIDTH: f32 = 640.;
const HEIGHT: f32 = 240.;
const BACKGROUND: u32 = 0x101418;
const CARD: u32 = 0x2050c0;
/// Where each card is laid out, and the scale it is drawn at about its top left corner.
const CARDS: [(f32, f32); 3] = [(40., 1.), (240., 0.94), (440., 0.5)];
const TOP: f32 = 40.;
const CARD_WIDTH: f32 = 160.;
const CARD_HEIGHT: f32 = 120.;

struct Transformed {
    transform: ElementTransform,
    child: AnyElement,
}

impl IntoElement for Transformed {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Transformed {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_element_transform(self.transform, |window| self.child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        (): &mut (),
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_element_transform(self.transform, |window| self.child.paint(window, cx));
    }
}

struct Cards;

impl Render for Cards {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .relative()
            .bg(rgb(BACKGROUND))
            .font_family("IBM Plex Sans")
            .text_color(white())
            .children(CARDS.map(|(left, scale)| {
                Transformed {
                    transform: ElementTransform::scale_about(scale, point(px(left), px(TOP))),
                    child: div()
                        .absolute()
                        .left(px(left))
                        .top(px(TOP))
                        .w(px(CARD_WIDTH))
                        .h(px(CARD_HEIGHT))
                        .p(px(12.))
                        .bg(rgb(CARD))
                        .border_2()
                        .border_color(white())
                        .rounded(px(12.))
                        .text_size(px(18.))
                        .child("Open")
                        .into_any_element(),
                }
            }))
    }
}

fn main() {
    if std::env::var_os("GPUI_RUN_RENDERING_TESTS").is_none() {
        return;
    }

    #[cfg(target_os = "macos")]
    let mut cx = VisualTestAppContext::new(gpui_ce_platform::current_platform(false));

    #[cfg(not(target_os = "macos"))]
    let mut cx = {
        let platform = gpui_ce_platform::current_platform(true);
        let text_system = platform.text_system();
        HeadlessAppContext::with_platform(text_system, std::sync::Arc::new(()), || {
            Some(Box::new(
                gpui_wgpu::WgpuHeadlessRenderer::new()
                    .expect("failed to initialize the headless WGPU renderer"),
            ) as Box<dyn gpui::PlatformHeadlessRenderer>)
        })
    };

    cx.update(|cx| {
        cx.text_system()
            .add_fonts(vec![Cow::Borrowed(IBM_PLEX.data)])
    })
    .expect("failed to load the fixture font");

    #[cfg(target_os = "macos")]
    let window = cx
        .open_offscreen_window(gpui::size(px(WIDTH), px(HEIGHT)), |_, cx| cx.new(|_| Cards))
        .expect("failed to create the offscreen window");

    #[cfg(not(target_os = "macos"))]
    let window = cx
        .open_window(gpui::size(px(WIDTH), px(HEIGHT)), |_, cx| cx.new(|_| Cards))
        .expect("failed to create the headless window");

    let window = window.into();
    cx.run_until_parked();
    let image = cx
        .capture_screenshot(window)
        .expect("failed to capture the cards");
    if let Some(output) = std::env::var_os("GPUI_RENDERING_TEST_OUTPUT") {
        image.save(output).expect("failed to save the cards");
    }

    let scale = image.width() as f32 / WIDTH;
    let rgb_at = |x: f32, y: f32| {
        let pixel = image.get_pixel((x * scale) as u32, (y * scale) as u32);
        (u32::from(pixel[0]) << 16) | (u32::from(pixel[1]) << 8) | u32::from(pixel[2])
    };
    let near = |found: u32, want: u32| {
        let channel = |c: u32, shift: u32| ((c >> shift) & 0xff) as i32;
        [16, 8, 0]
            .iter()
            .all(|&shift| (channel(found, shift) - channel(want, shift)).abs() <= 8)
    };

    for (left, card_scale) in CARDS {
        let right = left + CARD_WIDTH * card_scale;
        let bottom = TOP + CARD_HEIGHT * card_scale;
        let inside = rgb_at(right - 10., bottom - 10.);
        assert!(
            near(inside, CARD),
            "the card at {left} drawn at {card_scale} shows {inside:06x} inside its drawn bounds"
        );
        let past = rgb_at(right + 6., bottom + 6.);
        assert!(
            near(past, BACKGROUND),
            "the card at {left} drawn at {card_scale} shows {past:06x} past its drawn corner"
        );
    }
}
