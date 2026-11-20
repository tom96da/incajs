// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scrollbars of a scroll container: the platform setting, the width a bar reserves and the
//! geometry of its track and thumb.

mod drag;
mod hover;
mod press;
mod view;

use std::time::Duration;

use gpui::{App, Axis, Bounds, Hsla, Pixels, Point, Size, point, px, rgb, size};

use inca_node::spec::WheelAxes;

pub(super) use hover::OverBar;
pub(super) use view::scrollbar;

/// The width of the strip a bar reserves, and the width of the area an overlay bar answers in.
pub(super) const GUTTER_PX: f32 = 15.;
/// The thickness of a thumb in a reserved strip.
const THUMB_THICKNESS_PX: f32 = 9.;
/// The distance between a thumb in a reserved strip and each edge of the strip.
const THUMB_INSET_PX: f32 = 3.;
/// The distance between a thumb and each end of its track.
const THUMB_END_INSET_PX: f32 = 3.;
/// The shortest thumb.
const MIN_THUMB_PX: f32 = 17.;
/// The length of an arrow button at each end of a track. The host draws none.
const BUTTON_PX: f32 = 0.;
/// The thickness of a thumb drawn over the content.
const OVERLAY_THICKNESS_PX: f32 = 4.;
/// The distance between a thumb drawn over the content and the edge of the padding box.
const OVERLAY_INSET_PX: f32 = 3.;
/// The time a thumb drawn over the content stays after it was shown.
const HIDE_DELAY: Duration = Duration::from_millis(1000);
const TRACK: u32 = 0x00fc_fcfc;
const THUMB: u32 = 0x008b_8b8b;
const OVERLAY_THUMB: u32 = 0x008d_8d8d;
/// The color of a thumb under the pointer and of a thumb being dragged.
const THUMB_HOVER: u32 = 0x0063_6363;

/// How the platform shows scrollbars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scrollbars {
    /// A `scroll` axis reserves the width of its bar, and a thumb stays visible.
    Classic,
    /// No bar reserves width. A thumb shows while the content scrolls.
    Overlay,
}

impl Scrollbars {
    /// `Overlay` when the platform hides scrollbars while the content stands still.
    #[must_use]
    pub fn from_auto_hide(auto_hide: bool) -> Self {
        if auto_hide {
            Self::Overlay
        } else {
            Self::Classic
        }
    }

    /// The setting of the platform of `cx`. Read it once per render.
    #[must_use]
    pub fn of(cx: &App) -> Self {
        #[cfg(feature = "test-support")]
        if let Some(setting) = cx.try_global::<ScrollbarsSetting>() {
            return setting.0;
        }
        Self::from_auto_hide(cx.should_auto_hide_scrollbars())
    }
}

/// The mode a test stored with [`Scrollbars::set_for_tests`].
#[cfg(feature = "test-support")]
struct ScrollbarsSetting(Scrollbars);

#[cfg(feature = "test-support")]
impl gpui::Global for ScrollbarsSetting {}

#[cfg(feature = "test-support")]
impl Scrollbars {
    /// Makes [`Scrollbars::of`] answer `mode` on `cx`, whatever the platform says.
    pub fn set_for_tests(cx: &mut App, mode: Self) {
        cx.set_global(ScrollbarsSetting(mode));
    }
}

/// The strips `mode` reserves for the axes in `scroll` (the axes whose overflow is `scroll`):
/// `width` for the vertical bar and `height` for the horizontal bar.
pub(super) fn gutter_of(scroll: WheelAxes, mode: Scrollbars) -> Size<Pixels> {
    let reserve = |axis: bool| {
        if axis && mode == Scrollbars::Classic {
            px(GUTTER_PX)
        } else {
            Pixels::ZERO
        }
    };
    size(reserve(scroll.y), reserve(scroll.x))
}

/// One bar of a container.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Bar {
    pub(super) axis: Axis,
    /// The strip along the axis, `GUTTER_PX` thick, at the edge of the padding box.
    pub(super) track: Bounds<Pixels>,
    /// `None` when the content fits or the track leaves no room.
    pub(super) thumb: Option<Bounds<Pixels>>,
    /// Whether the bar reserves its strip, so the track is painted.
    pub(super) strip: bool,
    /// The coordinate along the axis where the thumb starts at scroll position 0.
    pub(super) origin: f32,
    /// How far the thumb moves along the axis; 0 with no thumb.
    pub(super) travel: f32,
    /// The range of the axis.
    pub(super) max: f32,
}

/// The bars of a container and the corner between two reserved strips.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct Parts {
    pub(super) x: Option<Bar>,
    pub(super) y: Option<Bar>,
    pub(super) corner: Option<Bounds<Pixels>>,
}

impl Parts {
    /// The bars in the order x, y.
    pub(super) fn bars(&self) -> impl Iterator<Item = &Bar> {
        self.x.iter().chain(&self.y)
    }
}

/// The measures of one bar, in pixels.
struct Lane {
    axis: Axis,
    frame: Bounds<Pixels>,
    border: f32,
    /// The strip this bar reserves.
    strip: f32,
    /// The strip of the bar on the other axis.
    other: f32,
    /// The length the corner takes off the track of a bar with no strip.
    shorten: f32,
    /// The range of the axis.
    max: f32,
    /// The position along the axis, between 0 and `max`.
    scroll: f32,
}

fn rect(axis: Axis, along: f32, length: f32, across: f32, thickness: f32) -> Bounds<Pixels> {
    let (x, y, width, height) = match axis {
        Axis::Vertical => (across, along, thickness, length),
        Axis::Horizontal => (along, across, length, thickness),
    };
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

impl Lane {
    fn bar(&self) -> Option<Bar> {
        let (extent, start, edge) = match self.axis {
            Axis::Vertical => (self.frame.size.height, self.frame.top(), self.frame.right()),
            Axis::Horizontal => (
                self.frame.size.width,
                self.frame.left(),
                self.frame.bottom(),
            ),
        };
        let (extent, start, edge) = (f32::from(extent), f32::from(start), f32::from(edge));
        let client = (extent - self.border * 2. - self.other).max(0.);
        let track = client - self.shorten;
        if track <= 0. {
            return None;
        }
        let region = (track - 2. * (BUTTON_PX + THUMB_END_INSET_PX)).max(0.);
        let across = edge - self.border - GUTTER_PX;
        let origin = start + self.border + BUTTON_PX + THUMB_END_INSET_PX;
        let mut travel = 0.;
        let thumb = (self.max > 0. && region > 0.).then(|| {
            let length = (region * client / (client + self.max))
                .round()
                .max(MIN_THUMB_PX)
                .min(region);
            travel = region - length;
            let along = origin + travel * self.scroll / self.max;
            if self.strip > 0. {
                let inset = across + THUMB_INSET_PX;
                rect(self.axis, along, length, inset, THUMB_THICKNESS_PX)
            } else {
                let inset = edge - self.border - OVERLAY_INSET_PX - OVERLAY_THICKNESS_PX;
                rect(self.axis, along, length, inset, OVERLAY_THICKNESS_PX)
            }
        });
        Some(Bar {
            axis: self.axis,
            track: rect(self.axis, start + self.border, track, across, GUTTER_PX),
            thumb,
            strip: self.strip > 0.,
            origin,
            travel,
            max: self.max,
        })
    }
}

/// The bars of the container in `frame` (its border box), from its border width, the reserved
/// `gutter`, the `axes` that scroll, the range `max` and the `offset` of the handle.
///
/// A bar with no reserved strip has a track of the same size and no painted track. When both
/// axes carry such a bar, each track ends `GUTTER_PX` before the corner.
pub(super) fn parts(
    frame: Bounds<Pixels>,
    border: Pixels,
    gutter: Size<Pixels>,
    axes: WheelAxes,
    max: Point<Pixels>,
    offset: Point<Pixels>,
) -> Parts {
    let (gw, gh) = (f32::from(gutter.width), f32::from(gutter.height));
    let border = f32::from(border);
    let shorten = if axes.x && axes.y && gw == 0. && gh == 0. {
        GUTTER_PX
    } else {
        0.
    };
    let lane = |axis: Axis, strip: f32, other: f32, max: Pixels, offset: Pixels| Lane {
        axis,
        frame,
        border,
        strip,
        other,
        shorten,
        max: f32::from(max),
        scroll: (-f32::from(offset)).clamp(0., f32::from(max)),
    };
    let corner = (gw > 0. && gh > 0.).then(|| {
        let origin = point(
            frame.right() - px(border + gw),
            frame.bottom() - px(border + gh),
        );
        Bounds::new(origin, size(px(gw), px(gh)))
    });
    Parts {
        x: axes
            .x
            .then(|| lane(Axis::Horizontal, gh, gw, max.x, offset.x).bar())
            .flatten(),
        y: axes
            .y
            .then(|| lane(Axis::Vertical, gw, gh, max.y, offset.y).bar())
            .flatten(),
        corner,
    }
}

/// The thumb's extent along the axis and the whole strip across: where a press starts a drag.
pub(super) fn thumb_zone(bar: &Bar) -> Option<Bounds<Pixels>> {
    let thumb = bar.thumb?;
    Some(match bar.axis {
        Axis::Vertical => Bounds::new(
            point(bar.track.origin.x, thumb.origin.y),
            size(bar.track.size.width, thumb.size.height),
        ),
        Axis::Horizontal => Bounds::new(
            point(thumb.origin.x, bar.track.origin.y),
            size(thumb.size.width, bar.track.size.height),
        ),
    })
}

/// The scroll position a drag of `bar` gives with the pointer at `pointer` along the axis, `grab`
/// being the distance from the thumb start to the pointer at the press.
pub(super) fn drag_offset(bar: &Bar, grab: f32, pointer: f32) -> f32 {
    bar.max * (pointer - grab - bar.origin).clamp(0., bar.travel) / bar.travel
}

/// The color of a thumb, `hover` holding for the pointer over it and for a drag.
pub(super) fn thumb_color(hover: bool, strip: bool) -> Hsla {
    let color = match (hover, strip) {
        (true, _) => THUMB_HOVER,
        (false, true) => THUMB,
        (false, false) => OVERLAY_THUMB,
    };
    Hsla::from(rgb(color))
}

/// The areas that answer to the pointer, with the axis of each: the track of a bar that reserves
/// its strip, the strip of an overlay bar whose thumb shows (`shown`), and the extent of the
/// thumb of a classic bar that reserves nothing.
pub(super) fn hit_rects(
    parts: &Parts,
    mode: Scrollbars,
    shown: bool,
) -> Vec<(Axis, Bounds<Pixels>)> {
    parts
        .bars()
        .filter_map(|bar| {
            let rect = if bar.strip {
                bar.track
            } else {
                let zone = thumb_zone(bar)?;
                match mode {
                    Scrollbars::Overlay if shown => bar.track,
                    Scrollbars::Overlay => return None,
                    Scrollbars::Classic => zone,
                }
            };
            Some((bar.axis, rect))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use gpui::TestAppContext;
    use rstest::rstest;

    use super::*;
    use crate::tests::{BarScene, Probe, boxed, probe_of, rect};
    use inca_node::VirtualTree;
    use inca_test_macros::gpui_cases;

    const BOTH: WheelAxes = WheelAxes { x: true, y: true };
    const X: WheelAxes = WheelAxes { x: true, y: false };
    const Y: WheelAxes = WheelAxes { x: false, y: true };
    const NONE: WheelAxes = WheelAxes { x: false, y: false };

    fn strips(width: f32, height: f32) -> Size<Pixels> {
        size(px(width), px(height))
    }

    /// The bars of a `width` x `height` frame with no border, the strips `gutter` and the range
    /// `max`, scrolled by `scroll`.
    fn bars(
        (width, height): (f32, f32),
        gutter: Size<Pixels>,
        axes: WheelAxes,
        max: (f32, f32),
        scroll: (f32, f32),
    ) -> Parts {
        parts(
            rect(0., 0., width, height),
            px(0.),
            gutter,
            axes,
            point(px(max.0), px(max.1)),
            point(px(-scroll.0), px(-scroll.1)),
        )
    }

    /// The vertical bar of a strip bar of `height` with the range `max`, scrolled by `scroll`.
    fn vertical(height: f32, max: f32, scroll: f32) -> Bar {
        let parts = bars((100., height), strips(15., 0.), Y, (0., max), (0., scroll));
        parts.y.unwrap()
    }

    #[rstest]
    #[case::ratio_two(106., 106., 50.)]
    #[case::rounds_up(70., 49., 38.)]
    #[case::minimum(106., 1000., 17.)]
    fn the_thumb_length_follows_the_client_over_the_content(
        #[case] height: f32,
        #[case] max: f32,
        #[case] length: f32,
    ) {
        let thumb = vertical(height, max, 0.).thumb.unwrap();
        assert_eq!(thumb.size.height, px(length));
    }

    #[rstest]
    #[case::start(0., 3.)]
    #[case::middle(53., 28.)]
    #[case::end(106., 53.)]
    fn the_thumb_moves_through_the_region_with_the_scroll_position(
        #[case] scroll: f32,
        #[case] start: f32,
    ) {
        let thumb = vertical(106., 106., scroll).thumb.unwrap();
        assert_eq!(thumb.origin.y, px(start));
    }

    #[test]
    fn a_region_shorter_than_the_minimum_is_filled_by_the_thumb() {
        let thumb = vertical(16., 100., 0.).thumb.unwrap();
        assert_eq!((thumb.origin.y, thumb.size.height), (px(3.), px(10.)));
    }

    #[rstest]
    #[case::no_region(6.)]
    #[case::shorter(2.)]
    fn a_track_with_no_region_has_no_thumb(#[case] height: f32) {
        assert_eq!(vertical(height, 100., 0.).thumb, None);
    }

    #[test]
    fn a_strip_bar_keeps_its_track_when_the_content_fits() {
        let bar = vertical(100., 0., 0.);
        assert_eq!((bar.track, bar.thumb), (rect(85., 0., 15., 100.), None));
    }

    #[test]
    fn a_bar_with_no_strip_has_no_thumb_and_no_strip_when_the_content_fits() {
        let parts = bars((100., 100.), strips(0., 0.), Y, (0., 0.), (0., 0.));
        let bar = parts.y.unwrap();
        assert_eq!((bar.strip, bar.thumb), (false, None));
    }

    #[test]
    fn a_bar_with_no_strip_and_an_overflow_has_a_thumb_and_no_strip() {
        let parts = bars((100., 100.), strips(0., 0.), Y, (0., 100.), (0., 0.));
        let bar = parts.y.unwrap();
        assert_eq!((bar.strip, bar.thumb.is_some()), (false, true));
    }

    #[test]
    fn a_thumb_in_a_strip_is_thin_and_inset_from_the_strip_edges() {
        let thumb = vertical(100., 100., 0.).thumb.unwrap();
        assert_eq!((thumb.origin.x, thumb.size.width), (px(88.), px(9.)));
    }

    #[test]
    fn a_thumb_over_the_content_is_thinner_and_inset_from_the_padding_edge() {
        let parts = bars((100., 100.), strips(0., 0.), Y, (0., 100.), (0., 0.));
        let thumb = parts.y.unwrap().thumb.unwrap();
        assert_eq!((thumb.origin.x, thumb.size.width), (px(93.), px(4.)));
    }

    #[rstest]
    #[case::x_only(X, (true, false))]
    #[case::y_only(Y, (false, true))]
    #[case::both(BOTH, (true, true))]
    #[case::none(NONE, (false, false))]
    fn a_bar_exists_for_each_scrolling_axis(#[case] axes: WheelAxes, #[case] want: (bool, bool)) {
        let parts = bars((100., 100.), strips(15., 15.), axes, (100., 100.), (0., 0.));
        assert_eq!((parts.x.is_some(), parts.y.is_some()), want);
    }

    #[test]
    fn the_corner_lies_where_two_reserved_strips_meet() {
        let parts = bars((100., 100.), strips(15., 15.), BOTH, (100., 100.), (0., 0.));
        assert_eq!(parts.corner, Some(rect(85., 85., 15., 15.)));
    }

    #[rstest]
    #[case::one_strip(Y, strips(15., 0.))]
    #[case::no_strip(BOTH, strips(0., 0.))]
    fn a_corner_needs_two_reserved_strips(#[case] axes: WheelAxes, #[case] gutter: Size<Pixels>) {
        let parts = bars((100., 100.), gutter, axes, (100., 100.), (0., 0.));
        assert_eq!(parts.corner, None);
    }

    #[test]
    fn a_track_ends_before_the_strip_of_the_other_bar() {
        let parts = bars((100., 80.), strips(15., 15.), BOTH, (100., 100.), (0., 0.));
        let (x, y) = (parts.x.unwrap(), parts.y.unwrap());
        assert_eq!(
            (x.track.size.width, y.track.size.height),
            (px(85.), px(65.))
        );
    }

    #[rstest]
    #[case::both(BOTH, 85., 26.)]
    #[case::alone(Y, 100., 31.)]
    fn two_bars_over_the_content_end_their_tracks_before_the_corner(
        #[case] axes: WheelAxes,
        #[case] track: f32,
        #[case] length: f32,
    ) {
        let parts = bars((100., 100.), strips(0., 0.), axes, (200., 200.), (0., 0.));
        let bar = parts.y.unwrap();
        assert_eq!(
            (bar.track.size.height, bar.thumb.unwrap().size.height),
            (px(track), px(length))
        );
    }

    #[test]
    fn a_bar_over_the_content_beside_a_strip_bar_keeps_its_full_track() {
        let parts = bars((100., 100.), strips(15., 0.), BOTH, (200., 200.), (0., 0.));
        let (x, y) = (parts.x.unwrap(), parts.y.unwrap());
        assert_eq!(
            (x.track.size.width, y.track.size.height),
            (px(85.), px(100.))
        );
        assert_eq!(parts.corner, None);
    }

    #[rstest]
    #[case::classic_both(Scrollbars::Classic, BOTH, (15., 15.))]
    #[case::classic_x(Scrollbars::Classic, X, (0., 15.))]
    #[case::classic_none(Scrollbars::Classic, NONE, (0., 0.))]
    #[case::overlay(Scrollbars::Overlay, BOTH, (0., 0.))]
    fn the_gutter_follows_the_mode_and_the_scroll_axes(
        #[case] mode: Scrollbars,
        #[case] axes: WheelAxes,
        #[case] want: (f32, f32),
    ) {
        assert_eq!(gutter_of(axes, mode), strips(want.0, want.1));
    }

    #[rstest]
    #[case::hides(true, Scrollbars::Overlay)]
    #[case::stays(false, Scrollbars::Classic)]
    fn the_platform_setting_selects_the_mode(#[case] auto_hide: bool, #[case] want: Scrollbars) {
        assert_eq!(Scrollbars::from_auto_hide(auto_hide), want);
    }

    /// The recorded boxes of a tree of a 100x100 root styled `props` with a child of `child`
    /// styles, drawn in `mode`.
    fn bar_probe(
        cx: &mut TestAppContext,
        mode: Scrollbars,
        props: &[(&str, &str)],
        child: &[(&str, f64)],
    ) -> Probe {
        let mut tree = VirtualTree::new();
        let root = boxed(&mut tree, None, 100., 100.);
        for (key, value) in props {
            tree.set_style(root, *key, *value).unwrap();
        }
        let leaf = tree.create_node("div").unwrap();
        for (key, value) in child {
            tree.set_style(leaf, *key, *value).unwrap();
        }
        tree.append_child(root, leaf).unwrap();
        let probe = Probe {
            mode,
            ..probe_of(tree)
        };
        BarScene::with_probe(cx, probe.clone());
        probe
    }

    #[gpui::test]
    fn a_classic_scroll_container_lays_a_child_out_beside_its_strip(cx: &mut TestAppContext) {
        let probe = bar_probe(
            cx,
            Scrollbars::Classic,
            &[("overflow", "scroll")],
            &[("height", 30.)],
        );
        assert_eq!(probe.layouts.borrow()[&1].bounds.size.width, px(85.));
    }

    #[gpui::test]
    fn a_classic_auto_container_lays_a_child_out_at_its_full_width(cx: &mut TestAppContext) {
        let probe = bar_probe(
            cx,
            Scrollbars::Classic,
            &[("overflow", "auto")],
            &[("height", 30.)],
        );
        assert_eq!(probe.layouts.borrow()[&1].bounds, rect(0., 0., 100., 30.));
    }

    #[gpui::test]
    fn a_hidden_axis_beside_a_scroll_axis_reserves_no_strip(cx: &mut TestAppContext) {
        let props = [("overflow_x", "hidden"), ("overflow_y", "scroll")];
        let probe = bar_probe(cx, Scrollbars::Classic, &props, &[("height", 300.)]);
        let handle = probe.handles.borrow()[&0].clone();
        assert_eq!(handle.max_offset().y, px(200.));
        assert_eq!(probe.layouts.borrow()[&1].bounds.size.width, px(85.));
    }

    /// The range of a 200x100 scroller with border 3 and padding 10, holding `child`.
    fn bordered_range(
        cx: &mut TestAppContext,
        mode: Scrollbars,
        (width, height): (f64, f64),
    ) -> Point<Pixels> {
        let mut tree = VirtualTree::new();
        let scroller = boxed(&mut tree, None, 200., 100.);
        tree.set_style(scroller, "padding", 10.0).unwrap();
        tree.set_style(scroller, "border_width", 3.0).unwrap();
        tree.set_style(scroller, "overflow", "scroll").unwrap();
        let child = boxed(&mut tree, Some(scroller), width, height);
        tree.set_style(child, "flex_shrink", 0.0).unwrap();
        let probe = Probe {
            mode,
            ..probe_of(tree)
        };
        BarScene::with_probe(cx, probe.clone());
        probe.handles.borrow()[&scroller].max_offset()
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::classic(Scrollbars::Classic, 241.)]
    #[case::overlay(Scrollbars::Overlay, 226.)]
    fn the_scroll_range_of_a_bordered_container_ends_at_the_client_edge(
        cx: &mut TestAppContext,
        mode: Scrollbars,
        max: f32,
    ) {
        let range = bordered_range(cx, mode, (400., 300.));
        assert_eq!(range, point(px(max), px(max)));
    }

    #[gpui::test]
    fn an_empty_bordered_container_has_no_range_beside_its_strips(cx: &mut TestAppContext) {
        let mut tree = VirtualTree::new();
        let scroller = boxed(&mut tree, None, 38., 38.);
        tree.set_style(scroller, "padding", 10.0).unwrap();
        tree.set_style(scroller, "border_width", 3.0).unwrap();
        tree.set_style(scroller, "overflow", "scroll").unwrap();
        let probe = Probe {
            mode: Scrollbars::Classic,
            ..probe_of(tree)
        };
        BarScene::with_probe(cx, probe.clone());
        let range = probe.handles.borrow()[&scroller].max_offset();
        assert_eq!(range, point(px(0.), px(0.)));
    }

    #[rstest]
    #[case::strip_idle(false, true, THUMB)]
    #[case::overlay_idle(false, false, OVERLAY_THUMB)]
    #[case::strip_hover(true, true, THUMB_HOVER)]
    #[case::overlay_hover(true, false, THUMB_HOVER)]
    fn the_thumb_color_follows_the_hover_and_the_strip(
        #[case] hover: bool,
        #[case] strip: bool,
        #[case] want: u32,
    ) {
        assert_eq!(thumb_color(hover, strip), Hsla::from(rgb(want)));
    }

    /// A vertical bar with a thumb that travels 32 along a range of 765, starting at 3.
    fn travelling_bar() -> Bar {
        Bar {
            axis: Axis::Vertical,
            track: rect(85., 0., 15., 85.),
            thumb: None,
            strip: true,
            origin: 3.,
            travel: 32.,
            max: 765.,
        }
    }

    #[rstest]
    #[case::moved_ten(13., 239.0625)]
    #[case::before_the_origin(0., 0.)]
    #[case::far_beyond(1003., 765.)]
    fn a_drag_scrolls_by_the_pointer_travel_over_the_thumb_travel(
        #[case] pointer: f32,
        #[case] want: f32,
    ) {
        assert_eq!(drag_offset(&travelling_bar(), 0., pointer), want);
    }
}
