//! Capability probes for the sample-git design. Each test prints what it saw.

use div::prelude::*;
use div::testing::{Key, TestApp};
use std::time::Instant;

fn app(view: impl FnOnce() -> ViewNode + 'static) -> TestApp {
    TestApp::new(LogicalSize::new(800.0, 600.0)).mount(view)
}

fn pixel(ui: &TestApp, x: u32, y: u32) -> [u8; 4] {
    let bitmap = ui.document().render_to_bitmap().unwrap();
    let w = bitmap.pixels().len() / 600 / 4;
    let scale = w as u32 / 800;
    let i = (((y * scale) as usize) * w + (x * scale) as usize) * 4;
    let p = bitmap.pixels();
    [p[i], p[i + 1], p[i + 2], p[i + 3]]
}

#[test]
fn textarea_typing_and_value() {
    let seen = create_signal(String::new());
    let mut ui = app(move || {
        view! {
            <div>
                <textarea id="msg" style:width="300px" style:height="80px"
                    on:input={move |e: &Event| {
                        if let Some(t) = e.target() { seen.set(t.control_value().unwrap().unwrap_or_default()); }
                    }}></textarea>
            </div>
        }
    });
    let ta = ui.by_id("msg");
    ui.click(&ta);
    println!("textarea focused after click: {:?}", ta.has_focus());
    ui.type_text("fix: bug");
    ui.press(Key::Enter);
    ui.type_text("body");
    println!("textarea value: {:?}", ta.control_value());
    println!("on:input saw: {:?}", seen.get());
    ta.set_control_value("").unwrap();
    println!("after clear: {:?}", ta.control_value());
}

#[test]
fn input_text_value() {
    let mut ui = app(move || view! { <input id="i" type="text" style:width="200px" /> });
    let i = ui.by_id("i");
    ui.click(&i);
    ui.type_text("main");
    println!("input value: {:?}", i.control_value());
}

#[test]
fn reactive_style_updates() {
    let on = create_signal(false);
    let ui = app(move || {
        view! {
            <div id="box" style:width="100px" style:height="100px"
                style:background-color={if on.get() { Color::from_rgba8(255, 0, 0, 255) } else { Color::from_rgba8(0, 0, 255, 255) }}></div>
        }
    });
    println!("before: {:?}", pixel(&ui, 50, 50));
    on.set(true);
    println!("after set(true): {:?}", pixel(&ui, 50, 50));
}

#[test]
fn hover_events() {
    let hovered = create_signal(String::new());
    let mut ui = app(move || {
        view! {
            <div id="h" style:width="100px" style:height="100px"
                on:mouseenter={move |_| hovered.set("enter".into())}
                on:mouseleave={move |_| hovered.set("leave".into())}
                on:pointerenter={move |_| hovered.update(|s| s.push_str("+penter"))}
                on:mouseover={move |_| hovered.update(|s| s.push_str("+over"))}
            ></div>
        }
    });
    let doc = ui.document().clone();
    doc.dispatch_mouse_event(MouseEventType::Move, 50.0, 50.0, MouseButton::Left, Modifiers::NONE).unwrap();
    ui.run_until_idle();
    println!("after move in: {:?}", hovered.get());
    doc.dispatch_mouse_event(MouseEventType::Move, 500.0, 500.0, MouseButton::Left, Modifiers::NONE).unwrap();
    ui.run_until_idle();
    println!("after move out: {:?}", hovered.get());
}

#[test]
fn scrolling_list_and_for_cost() {
    let rows = create_signal((0..2000).collect::<Vec<u32>>());
    let start = Instant::now();
    let mut ui = app(move || {
        let list = For(move || rows.get(), |r| *r, |r| view! {
            <div style:height="20px" style:font-family="Geist Mono, monospace" style:font-size="12px">{format!("row {r}")}</div>
        });
        let outer = view! { <div id="scroll" style:height="400px" style:overflow-y="auto" style:width="400px"></div> };
        if let ViewNode::Element(e) = &outer { mount_view(e, list); }
        outer
    });
    println!("mount 2000 rows: {:?}", start.elapsed());
    let t = Instant::now();
    let _ = pixel(&ui, 10, 10);
    println!("first render 2000 rows: {:?}", t.elapsed());
    let s = ui.by_id("scroll");
    println!("metrics: {:?}", s.scroll_metrics().map(|m| m.map(|m| (m.client_height, m.scroll_height))));
    ui.document().dispatch_wheel_event(100.0, 100.0, 0.0, 300.0, Modifiers::NONE).unwrap();
    ui.run_until_idle();
    println!("scroll_top after wheel: {:?}", s.scroll_top());
    s.scroll_to(0.0, 1000.0).unwrap();
    println!("scroll_top after scroll_to: {:?}", s.scroll_top());
    let t = Instant::now();
    rows.set((0..2001).collect());
    println!("append one row: {:?}", t.elapsed());
    let t = Instant::now();
    rows.set((5000..7000).collect());
    let _ = pixel(&ui, 10, 10);
    println!("replace all 2000 rows + render: {:?}", t.elapsed());
    ui.run_until_idle();
}

#[test]
fn monospace_text() {
    let ui = app(move || {
        view! {
            <div>
                <span id="a" style:font-family="Geist Mono" style:font-size="12px" style:white-space="pre">"iiiiiiiiii"</span>
                <br/>
                <span id="b" style:font-family="Geist Mono" style:font-size="12px" style:white-space="pre">"WWWWWWWWWW"</span>
                <br/>
                <span id="c" style:font-family="Consolas, monospace" style:font-size="12px" style:white-space="pre">"iiiiiiiiii"</span>
                <br/>
                <span id="d" style:font-family="Consolas, monospace" style:font-size="12px" style:white-space="pre">"WWWWWWWWWW"</span>
                <br/>
                <span id="e" style:font-family="monospace" style:font-size="12px" style:white-space="pre">"  a\tb"</span>
            </div>
        }
    });
    let reg = div_components::register_fonts(ui.document());
    println!("register fonts: {:?}", reg.as_ref().map(|v| v.len()));
    for id in ["a", "b", "c", "d", "e"] {
        println!("{id} width: {:?}", ui.by_id(id).bounding_rect().unwrap().map(|r| r.width));
    }
}

#[test]
fn overlay_and_modal_root() {
    let clicked = create_signal(String::new());
    let mut ui = app(move || {
        view! {
            <div style:position="relative" style:width="800px" style:height="600px">
                <button id="under" on:click={move |_| clicked.set("under".into())}>"under"</button>
                <div id="scrim" style:position="absolute" style:top="0" style:left="0" style:right="0" style:bottom="0" style:background-color="rgba(0, 0, 0, 0.5)" style:z-index="10"
                    style:display="flex" style:align-items="center" style:justify-content="center">
                    <div style:background-color="white" style:padding="16px">
                        <button id="ok" on:click={move |_| clicked.set("ok".into())}>"OK"</button>
                    </div>
                </div>
            </div>
        }
    });
    let under = ui.by_id("under");
    ui.click(&under);
    println!("click on covered button reached: {:?}", clicked.get());
    let ok = ui.by_id("ok");
    ui.document().set_modal_root(Some(&ui.by_id("scrim"))).unwrap();
    ui.click(&ok);
    println!("click on dialog: {:?}", clicked.get());
}

#[test]
fn keyboard_events() {
    let log = create_signal(Vec::<String>::new());
    let mut ui = app(move || {
        view! {
            <div id="k" tabindex="0" style:width="100px" style:height="100px"
                on:keydown={move |e: &Event| log.update(|l| l.push(format!("down code={} text={:?} mods={}", e.key_code, e.key_text, e.modifiers)))}></div>
        }
    });
    let k = ui.by_id("k");
    k.focus().unwrap();
    ui.press(Key::ArrowDown);
    ui.press_with(Modifiers::CTRL, Key::Enter);
    ui.press(Key::Char('j'));
    println!("keys: {:?}", log.get());
}

#[test]
fn component_on_click_is_dropped() {
    use div_components::{Button, ButtonProps, ButtonSize, ButtonVariant};
    let clicks = create_signal(0);
    let mut ui = app(move || {
        view! {
            <div>
                <Button variant={ButtonVariant::Default} size={ButtonSize::Default} disabled={false}
                    on:click={move |_| clicks.update(|c| *c += 1)}>"Commit"</Button>
                <span on:click={move |_| clicks.update(|c| *c += 10)}>
                    <Button variant={ButtonVariant::Outline} size={ButtonSize::Sm} disabled={false}>"Wrapped"</Button>
                </span>
            </div>
        }
    });
    let b = ui.by_text("Commit");
    ui.click(&b);
    let w = ui.by_text("Wrapped");
    ui.click(&w);
    println!("clicks (1 = direct on:click works, 10 = wrapper works): {}", clicks.get());
}

#[test]
fn select_element() {
    let mut ui = app(move || {
        view! {
            <select id="s"><option value="a">"main"</option><option value="b">"dev"</option></select>
        }
    });
    let s = ui.by_id("s");
    ui.click(&s);
    println!("select value: {:?}", s.control_value());
    ui.press(Key::ArrowDown);
    println!("select value after down: {:?}", s.control_value());
}

#[test]
fn split_pane_drag() {
    let width = create_signal(200.0_f32);
    let mut ui = app(move || {
        view! {
            <div style:display="flex" style:height="600px">
                <div style:width={LengthValue::px(width.get())} style:background-color="rgb(240, 240, 240)"></div>
                <div id="handle" style:width="4px" style:background-color="rgb(0, 0, 0)"
                    on:pointerdown={move |e: &Event| { if let Some(t) = e.current_target() { let _ = t.set_pointer_capture(e.pointer_id); } }}
                    on:pointermove={move |e: &Event| { if e.mouse_button >= 0 && e.event_type == "pointermove" { width.set(e.mouse_x); } }}
                ></div>
                <div style:flex-grow="1"></div>
            </div>
        }
    });
    let doc = ui.document().clone();
    doc.dispatch_mouse_event(MouseEventType::Move, 202.0, 100.0, MouseButton::Left, Modifiers::NONE).unwrap();
    doc.dispatch_mouse_event(MouseEventType::Down, 202.0, 100.0, MouseButton::Left, Modifiers::NONE).unwrap();
    doc.dispatch_mouse_event(MouseEventType::Move, 350.0, 100.0, MouseButton::Left, Modifiers::NONE).unwrap();
    ui.run_until_idle();
    println!("width after drag: {}", width.get());
    let h = ui.by_id("handle");
    println!("handle x after drag: {:?}", h.bounding_rect().unwrap().map(|r| r.x));
}

#[test]
fn context_menu_event() {
    let got = create_signal(false);
    let ui = app(move || view! { <div id="c" style:width="100px" style:height="100px" on:contextmenu={move |_| got.set(true)}></div> });
    let doc = ui.document().clone();
    doc.dispatch_mouse_event(MouseEventType::Down, 50.0, 50.0, MouseButton::Right, Modifiers::NONE).unwrap();
    doc.dispatch_mouse_event(MouseEventType::Up, 50.0, 50.0, MouseButton::Right, Modifiers::NONE).unwrap();
    println!("contextmenu fired: {}", got.get());
}

#[component]
fn Slot(view: ViewNode) -> ViewNode {
    view
}

#[test]
fn for_inside_view_needs_a_slot() {
    let items = create_signal(vec!["a".to_string(), "b".to_string()]);
    let ui = app(move || {
        view! {
            <div id="list">
                <Slot view={For(move || items.get(), |s| s.clone(), |s| view! { <div>{s}</div> })} />
                <Slot view={Show(move || items.get().len() > 2, || view! { <span>"few"</span> }, || view! { <span>"many"</span> })} />
            </div>
        }
    });
    println!("slot text: {:?}", ui.by_id("list").text_content());
    items.update(|v| v.push("c".into()));
    println!("slot text after push: {:?}", ui.by_id("list").text_content());
}

#[test]
fn ellipsis_truncation() {
    let ui = app(move || view! {
        <div id="t" style:width="80px" style:overflow="hidden" style:white-space="nowrap" style:text-overflow="ellipsis">"a/very/long/path/to/some/file.rs"</div>
    });
    let png = ui.render_png();
    std::fs::write(std::env::temp_dir().join("probe-ellipsis.png"), png).unwrap();
    println!("ellipsis rendered to temp probe-ellipsis.png");
}

#[test]
fn pointer_event_log() {
    let log = create_signal(Vec::<String>::new());
    let ui = app(move || {
        let rec = move |e: &Event| { let s = format!("{} b={} x={} id={}", e.event_type, e.mouse_button, e.mouse_x, e.pointer_id); log.update(|l| l.push(s)); };
        view! {
            <div style:display="flex" style:height="600px">
                <div style:width="200px"></div>
                <div id="handle" style:width="4px" style:background-color="rgb(0, 0, 0)"
                    on:pointerdown={move |e: &Event| { rec(e); if let Some(t) = e.current_target() { println!("capture: {:?}", t.set_pointer_capture(e.pointer_id)); } }}
                    on:pointermove={rec} on:mousemove={rec} on:mousedown={rec} on:pointerup={rec}
                ></div>
                <div style:flex-grow="1"></div>
            </div>
        }
    });
    let doc = ui.document().clone();
    doc.dispatch_mouse_event(MouseEventType::Move, 202.0, 100.0, MouseButton::Left, Modifiers::NONE).unwrap();
    doc.dispatch_mouse_event(MouseEventType::Down, 202.0, 100.0, MouseButton::Left, Modifiers::NONE).unwrap();
    doc.dispatch_mouse_event(MouseEventType::Move, 350.0, 100.0, MouseButton::Left, Modifiers::NONE).unwrap();
    doc.dispatch_mouse_event(MouseEventType::Up, 350.0, 100.0, MouseButton::Left, Modifiers::NONE).unwrap();
    for l in log.get() { println!("  {l}"); }
}

#[test]
fn select_initial_value() {
    let ui = app(move || view! { <select id="s"><option value="a">"main"</option><option value="b">"dev"</option></select> });
    println!("select initial value: {:?}", ui.by_id("s").control_value());
}
