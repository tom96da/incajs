// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use crate::config::DEFAULT_WINDOW_SIZE;

/// Opens a window directly (bypassing [`open`]'s disk-backed config
/// read) so a test can hand it a chosen `window_config` and starting
/// size without a real `inca.json` on disk.
fn open_hosted(
    cx: &mut TestAppContext,
    window_config: Option<config::WindowConfig>,
    initial_size: (f32, f32),
) -> WindowHandle<HostedApp> {
    let session = load("").unwrap();
    cx.update(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(initial_size.0), px(initial_size.1)),
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| HostedApp {
                    session,
                    window_config,
                    auto_resized: Cell::new((false, false)),
                })
            },
        )
        .unwrap()
    })
}

/// Mounts a `div` under the session's root with a declared
/// `width`/`height`, replacing any prior mount, which simulates content
/// becoming available after the window already opened.
fn mount_content(app: &HostedApp, width: f32, height: f32) {
    let mut host = app.session.host.borrow_mut();
    let root = host.root;
    let existing = host.tree.get(root).unwrap().children().first().copied();
    let content = if let Some(id) = existing {
        id
    } else {
        let id = host.tree.create_node("div").unwrap();
        host.tree.append_child(root, id).unwrap();
        id
    };
    host.tree
        .set_style(content, "width", f64::from(width))
        .unwrap();
    host.tree
        .set_style(content, "height", f64::from(height))
        .unwrap();
}

fn bounds_size(cx: &mut TestAppContext, window: WindowHandle<HostedApp>) -> (f32, f32) {
    cx.update(|cx| {
        window
            .update(cx, |_, window, _| {
                let bounds = window.bounds();
                (f32::from(bounds.size.width), f32::from(bounds.size.height))
            })
            .unwrap()
    })
}

#[gpui::test]
fn auto_resize_leaves_the_window_alone_until_content_becomes_usable(cx: &mut TestAppContext) {
    let window = open_hosted(cx, None, DEFAULT_WINDOW_SIZE);
    cx.run_until_parked();
    assert_eq!(bounds_size(cx, window), DEFAULT_WINDOW_SIZE);

    // Nothing mounted yet: the check runs (as it would on every
    // relay) but finds no usable content, so the window stays put.
    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });
    assert_eq!(bounds_size(cx, window), DEFAULT_WINDOW_SIZE);

    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                mount_content(app, 300.0, 150.0);
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });
    assert_eq!(bounds_size(cx, window), (300.0, 150.0));
}

#[gpui::test]
fn auto_resize_keeps_an_explicit_dimension_but_still_picks_up_the_other_from_content(
    cx: &mut TestAppContext,
) {
    let window_config = config::WindowConfig {
        width: Some(500.0),
        ..config::WindowConfig::default()
    };
    let window = open_hosted(cx, Some(window_config), (500.0, DEFAULT_WINDOW_SIZE.1));
    cx.run_until_parked();

    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                mount_content(app, 300.0, 150.0);
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });

    assert_eq!(
        bounds_size(cx, window),
        (500.0, 150.0),
        concat!(
            "the configured width must win, but the never-configured height still ",
            "comes from content, matching window_size's own per-dimension precedence"
        )
    );
}

#[gpui::test]
fn start_auto_resizes_content_that_mounts_only_after_it_returns(cx: &mut TestAppContext) {
    const MOUNTS_LATER: &str = r"
        Promise.resolve().then(() => {
            const node = __inca_native__.createNode('div');
            __inca_native__.appendChild(__inca_native__.rootNodeId(), node);
            __inca_native__.setStyle(node, 'width', 300);
            __inca_native__.setStyle(node, 'height', 150);
        });
    ";
    let window =
        cx.update(|cx| start(cx, TEST_ENTRY_PATH, MOUNTS_LATER, stderr_reporter()).unwrap());
    cx.run_until_parked();

    assert_eq!(
        bounds_size(cx, window),
        (300.0, 150.0),
        concat!(
            "start must give its own initial drain a chance to auto-resize, ",
            "not just a later reload or relay"
        )
    );
}

#[gpui::test]
fn auto_resize_never_fires_once_both_dimensions_are_configured(cx: &mut TestAppContext) {
    let window_config = config::WindowConfig {
        width: Some(500.0),
        height: Some(400.0),
        ..config::WindowConfig::default()
    };
    let window = open_hosted(cx, Some(window_config), (500.0, 400.0));
    cx.run_until_parked();

    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                mount_content(app, 300.0, 150.0);
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });

    assert_eq!(
        bounds_size(cx, window),
        (500.0, 400.0),
        "content can't change either dimension once config fixes both"
    );
}

#[gpui::test]
fn auto_resize_only_ever_fires_once(cx: &mut TestAppContext) {
    let window = open_hosted(cx, None, DEFAULT_WINDOW_SIZE);
    cx.run_until_parked();

    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                mount_content(app, 300.0, 150.0);
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });
    assert_eq!(bounds_size(cx, window), (300.0, 150.0));

    // Content changes again later; the latch must stop a second resize.
    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                mount_content(app, 900.0, 700.0);
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });
    assert_eq!(bounds_size(cx, window), (300.0, 150.0));
}

/// Mounts an empty `div` under the session's root, then declares each
/// given dimension on it.
fn mount_partial(app: &HostedApp, width: Option<f64>, height: Option<f64>) {
    let mut host = app.session.host.borrow_mut();
    let root = host.root;
    let content = host.tree.create_node("div").unwrap();
    host.tree.append_child(root, content).unwrap();
    if let Some(width) = width {
        host.tree.set_style(content, "width", width).unwrap();
    }
    if let Some(height) = height {
        host.tree.set_style(content, "height", height).unwrap();
    }
}

#[gpui::test]
fn auto_resize_fits_the_width_alone_when_the_root_sets_only_a_width(cx: &mut TestAppContext) {
    let window = open_hosted(cx, None, DEFAULT_WINDOW_SIZE);
    cx.run_until_parked();

    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                mount_partial(app, Some(300.0), None);
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });

    assert_eq!(bounds_size(cx, window), (300.0, DEFAULT_WINDOW_SIZE.1));
}

#[gpui::test]
fn auto_resize_fits_the_height_alone_when_the_root_sets_only_a_height(cx: &mut TestAppContext) {
    let window = open_hosted(cx, None, DEFAULT_WINDOW_SIZE);
    cx.run_until_parked();

    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                mount_partial(app, None, Some(150.0));
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });

    assert_eq!(bounds_size(cx, window), (DEFAULT_WINDOW_SIZE.0, 150.0));
}

#[gpui::test]
fn auto_resize_picks_up_the_second_dimension_when_it_arrives_later(cx: &mut TestAppContext) {
    let window = open_hosted(cx, None, DEFAULT_WINDOW_SIZE);
    cx.run_until_parked();

    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                mount_partial(app, Some(300.0), None);
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });
    assert_eq!(bounds_size(cx, window), (300.0, DEFAULT_WINDOW_SIZE.1));

    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                mount_content(app, 900.0, 150.0);
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });
    assert_eq!(
        bounds_size(cx, window),
        (300.0, 150.0),
        "the width fired once already, the height still has its turn"
    );
}

#[gpui::test]
fn auto_resize_leaves_a_dimension_the_user_resized_while_the_other_one_fires(
    cx: &mut TestAppContext,
) {
    let window = open_hosted(cx, None, DEFAULT_WINDOW_SIZE);
    cx.run_until_parked();

    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                mount_partial(app, Some(300.0), None);
                maybe_auto_resize_to_content(app, window);
                window.resize(size(px(350.0), px(450.0)));
                mount_content(app, 300.0, 150.0);
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });

    assert_eq!(bounds_size(cx, window), (350.0, 150.0));
}

#[gpui::test]
fn auto_resize_keeps_the_current_height_when_only_the_width_fires(cx: &mut TestAppContext) {
    let window = open_hosted(cx, None, (700.0, 450.0));
    cx.run_until_parked();

    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                mount_partial(app, Some(300.0), None);
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });

    assert_eq!(bounds_size(cx, window), (300.0, 450.0));
}

#[gpui::test]
fn auto_resize_treats_an_unusable_configured_dimension_as_not_ready(cx: &mut TestAppContext) {
    let window_config = config::WindowConfig {
        width: Some(-100.0),
        ..config::WindowConfig::default()
    };
    let window = open_hosted(cx, Some(window_config), DEFAULT_WINDOW_SIZE);
    cx.run_until_parked();

    // A configured width that no window could open at must not count
    // as ready just because it's `Some(_)`. Nothing usable exists yet.
    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });
    assert_eq!(bounds_size(cx, window), DEFAULT_WINDOW_SIZE);

    cx.update(|cx| {
        window
            .update(cx, |app, window, _| {
                mount_content(app, 300.0, 150.0);
                maybe_auto_resize_to_content(app, window);
            })
            .unwrap();
    });
    assert_eq!(bounds_size(cx, window), (300.0, 150.0));
}

#[test]
fn content_window_size_reads_the_mounted_root_childs_style() {
    let mut host = Host::default();
    let root = host.root;
    let content = host.tree.create_node("div").unwrap();
    host.tree.set_style(content, "width", 300.0).unwrap();
    host.tree.set_style(content, "height", 150.0).unwrap();
    host.tree.append_child(root, content).unwrap();

    assert_eq!(content_window_size(&host, root), (Some(300.0), Some(150.0)));
}

#[test]
fn content_window_size_is_none_when_unset() {
    let mut host = Host::default();
    let root = host.root;
    let content = host.tree.create_node("div").unwrap();
    host.tree.append_child(root, content).unwrap();

    assert_eq!(content_window_size(&host, root), (None, None));
}

#[test]
fn content_window_size_is_none_when_nothing_mounted() {
    let host = Host::default();

    assert_eq!(content_window_size(&host, host.root), (None, None));
}

#[gpui::test]
fn bringing_the_window_up_runs_what_mounting_only_queued(cx: &mut TestAppContext) {
    cx.update(|cx| start(cx, TEST_ENTRY_PATH, DEFERS_ITS_MOUNT, stderr_reporter()).unwrap());
    cx.run_until_parked();

    let ran: bool = cx.update(|cx| {
        cx.windows()
            .first()
            .and_then(|window| {
                window
                    .downcast::<HostedApp>()?
                    .read_with(cx, |app, _| {
                        app.session.engine.eval::<bool>("globalThis.mounted;").ok()
                    })
                    .ok()
                    .flatten()
            })
            .unwrap_or(false)
    });

    assert!(ran, "an onMounted-style effect must not wait for an event");
}
