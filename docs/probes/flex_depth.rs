//! Gap 26: layout time against the depth of nested flex columns.
//!
//! Copy into `tests/` of the probe crate (`docs/probes/Cargo.toml`) and run
//! `cargo test --test flex_depth -- --nocapture`.

use div::prelude::*;
use div::testing::TestApp;
use std::time::Instant;

fn row(document: &Document, n: usize) -> Element {
    let row = Element::create(document, "div").unwrap();
    row.apply_style(&style! { display: flex; white-space: pre }).unwrap();
    for _ in 0..2 {
        let cell = Element::create(document, "span").unwrap();
        cell.apply_style(&style!("width: 44px; flex-shrink: 0; text-align: right"))
            .unwrap();
        cell.set_text(&n.to_string()).unwrap();
        row.append_child(&cell).unwrap();
    }
    let content = Element::create(document, "span").unwrap();
    content.set_text(&format!("+line {n}")).unwrap();
    row.append_child(&content).unwrap();
    row
}

/// Appends `rows` rows to a scroller under `depth` nested containers and
/// returns how long `run_until_idle` takes to lay them out.
fn run(rows: usize, depth: usize, flex: bool) -> std::time::Duration {
    let host = create_signal(None::<Element>);
    let mut ui = TestApp::new(LogicalSize::new(800.0, 600.0)).mount(move || {
        let document = current_document();
        let outer = Element::create(&document, "div").unwrap();
        if flex {
            outer
                .apply_style(&style! { display: flex; flex-direction: column; height: 600px })
                .unwrap();
        } else {
            outer.apply_style(&style! { height: 600px }).unwrap();
        }
        let mut parent = outer.clone();
        for _ in 0..depth {
            let level = Element::create(&document, "div").unwrap();
            if flex {
                level
                    .apply_style(&style! {
                        display: flex; flex-direction: column; flex-grow: 1; min-height: 0
                    })
                    .unwrap();
            } else {
                level.apply_style(&style! { height: 100% }).unwrap();
            }
            parent.append_child(&level).unwrap();
            parent = level;
        }
        let body = Element::create(&document, "div").unwrap();
        if flex {
            body.apply_style(&style! { overflow: auto; flex-grow: 1; min-height: 0 })
                .unwrap();
        } else {
            body.apply_style(&style! { overflow: auto; height: 100% }).unwrap();
        }
        parent.append_child(&body).unwrap();
        host.set(Some(body));
        ViewNode::Element(outer)
    });
    ui.run_until_idle();
    let body = host.get().unwrap();
    let document = ui.document().clone();
    let start = Instant::now();
    for n in 0..rows {
        body.append_child(&row(&document, n)).unwrap();
    }
    ui.run_until_idle();
    start.elapsed()
}

#[test]
fn layout_time_by_flex_depth() {
    for depth in 0..=6 {
        println!(
            "depth {depth}: flex columns {:?}, blocks {:?}",
            run(100, depth, true),
            run(100, depth, false)
        );
    }
}
