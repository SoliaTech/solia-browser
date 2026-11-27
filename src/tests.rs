use super::*;
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

const SOURCE: &str = r#"app Counter {
    state { count = 0 }
    view { Column { Text("Count: {count}") Button("Increment") { count += 1 } } }
}"#;

fn server(responses: Vec<(&'static str, Vec<u8>)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/counter.solia", listener.local_addr().unwrap());
    thread::spawn(move || {
        for (headers, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
            }
            write!(stream, "HTTP/1.1 {headers}\r\nConnection: close\r\n\r\n").unwrap();
            let _ = stream.write_all(&body);
        }
    });
    url
}

fn settle(browser: &mut Browser) {
    let start = Instant::now();
    while browser.loading {
        browser.accept_results();
        assert!(start.elapsed() < Duration::from_secs(20));
        thread::sleep(Duration::from_millis(5));
    }
}

fn render(
    browser: &mut Browser,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut frame = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960., 640.),
            )),
            events,
            ..Default::default()
        },
        |ui| browser.show(ui),
    );
    frame.textures_delta.clear();
    frame
}

fn label(frame: &egui::FullOutput, text: &str) -> Option<egui::Pos2> {
    frame.shapes.iter().find_map(|s| match &s.shape {
        egui::Shape::Text(t) if t.galley.text() == text => Some(t.visual_bounding_rect().center()),
        _ => None,
    })
}

fn open_and_click(url: &str) -> (Browser, egui::Context) {
    let ctx = egui::Context::default();
    let mut browser = Browser::new(ctx.clone(), url.into()).unwrap();
    settle(&mut browser);
    assert!(browser.error.is_none(), "{:?}", browser.error);
    let first = render(&mut browser, &ctx, vec![]);
    assert!(label(&first, "Count: 0").is_some());
    let pos = label(&first, "Increment").unwrap();
    for pressed in [true, false] {
        render(
            &mut browser,
            &ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    pressed,
                    button: egui::PointerButton::Primary,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    assert!(label(&render(&mut browser, &ctx, vec![]), "Count: 1").is_some());
    (browser, ctx)
}

#[test]
fn reload_recovers_from_compile_errors_and_fetches_updated_source() {
    let url = server(vec![
        ("200 OK", SOURCE.as_bytes().into()),
        (
            "200 OK",
            b"app Counter { view { Column { Text(\"{missing}\") } } }".to_vec(),
        ),
        ("200 OK", SOURCE.replace("Count:", "Total:").into_bytes()),
    ]);
    let (mut browser, ctx) = open_and_click(&url);
    browser.navigate(browser.reload_address.clone().unwrap());
    settle(&mut browser);
    let error = browser.error.as_deref().unwrap();
    assert!(
        error.contains("unknown state") && error.contains(".solia:1:"),
        "{error}"
    );
    assert!(label(&render(&mut browser, &ctx, vec![]), "Count: 1").is_some());
    browser.navigate(browser.reload_address.clone().unwrap());
    settle(&mut browser);
    assert!(label(&render(&mut browser, &ctx, vec![]), "Total: 0").is_some());
}

#[tokio::test]
async fn network_limits_and_redirects() {
    let client = network::client().unwrap();
    for (headers, bytes, reason) in [
        ("404 Not Found", vec![], "404"),
        ("200 OK\r\nContent-Type: text/html", vec![], "Content-Type"),
        ("200 OK", vec![0xff], "UTF-8"),
        ("200 OK", vec![0; 1024 * 1024 + 1], "1 MiB"),
        (
            "302 Found\r\nLocation: http://example.org/app.solia",
            vec![],
            "not allowed",
        ),
    ] {
        let url = server(vec![(headers, bytes)]);
        let error = network::load(&client, &url).await.err().unwrap();
        assert!(error.contains(reason), "{error}");
    }
    let redirect = ("302 Found\r\nLocation: /another.solia", vec![]);
    let url = server(vec![redirect.clone(); 4]);
    assert!(
        network::load(&client, &url)
            .await
            .err()
            .unwrap()
            .contains("Too many")
    );
    let url = server(vec![
        redirect,
        (
            "200 OK\r\nContent-Type: text/plain; charset=utf-8",
            SOURCE.into(),
        ),
    ]);
    assert_eq!(
        network::load(&client, &url).await.unwrap().origin.path(),
        "/another.solia"
    );
}

#[test]
fn navigation_cancels_a_stalled_response() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/slow.solia", listener.local_addr().unwrap());
    let mut browser = Browser::new(egui::Context::default(), url).unwrap();
    let (_stalled, _) = listener.accept().unwrap();
    let next = server(vec![("200 OK", SOURCE.into())]);
    browser.navigate(next.clone());
    settle(&mut browser);
    assert_eq!(browser.reload_address, Some(next));
}

#[test]
#[ignore = "uses the configured HTTPS test domain"]
fn deployed_counter_renders_and_increments() {
    open_and_click(DEMO);
}

#[tokio::test]
async fn stalled_download_reaches_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/timeout.solia", listener.local_addr().unwrap());
    let error = network::load(&network::client().unwrap(), &url)
        .await
        .err()
        .unwrap();
    assert!(error.contains("timed out"), "{error}");
}

fn exercise_settings(url: &str) {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut browser = Browser::new(ctx.clone(), url.into()).unwrap();
    settle(&mut browser);
    assert!(browser.error.is_none(), "{:?}", browser.error);
    let frame = render(&mut browser, &ctx, vec![]);
    let nodes = &frame
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes;
    assert!(
        nodes
            .iter()
            .any(|(_, n)| n.role() == egui::accesskit::Role::Heading)
    );
    let slider = nodes
        .iter()
        .find(|(_, n)| n.role() == egui::accesskit::Role::Slider)
        .unwrap();
    assert_eq!(slider.1.min_numeric_value(), Some(0.));
    assert_eq!(slider.1.max_numeric_value(), Some(100.));
    let rail = slider.1.bounds().unwrap();
    let click = |browser: &mut Browser, pos| {
        for pressed in [true, false] {
            render(
                browser,
                &ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    };
    let key = |browser: &mut Browser, key| {
        for pressed in [true, false] {
            render(
                browser,
                &ctx,
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Default::default(),
                }],
            );
        }
    };
    click(&mut browser, label(&frame, "Mute").unwrap());
    assert!(label(&render(&mut browser, &ctx, vec![]), "Muted: true").is_some());
    // Tab through the address bar and navigation buttons into the application.
    for _ in 0..8 {
        let f = render(&mut browser, &ctx, vec![]);
        let tree = f.platform_output.accesskit_update.unwrap();
        if tree
            .nodes
            .iter()
            .any(|(id, node)| *id == tree.focus && node.role() == egui::accesskit::Role::CheckBox)
        {
            break;
        }
        key(&mut browser, egui::Key::Tab);
    }
    key(&mut browser, egui::Key::Space);
    assert!(label(&render(&mut browser, &ctx, vec![]), "Muted: false").is_some());
    key(&mut browser, egui::Key::Tab);
    key(&mut browser, egui::Key::ArrowRight);
    assert!(label(&render(&mut browser, &ctx, vec![]), "41%").is_some());
    click(
        &mut browser,
        egui::pos2((rail.x1 - 1.) as f32, ((rail.y0 + rail.y1) / 2.) as f32),
    );
    assert!(label(&render(&mut browser, &ctx, vec![]), "100%").is_some());
    click(&mut browser, label(&frame, "Reset").unwrap());
    assert!(label(&render(&mut browser, &ctx, vec![]), "40%").is_some());
    click(&mut browser, label(&frame, "Mute").unwrap());
    browser.navigate(url.into());
    settle(&mut browser);
    assert!(browser.error.is_none());
    assert!(label(&render(&mut browser, &ctx, vec![]), "Muted: false").is_some());
}

#[test]
fn settings_inputs_keyboard_reset_and_reload() {
    let source = include_bytes!("../tests/fixtures/settings.solia");
    exercise_settings(&server(vec![("200 OK", source.to_vec()); 2]));
}

#[test]
#[ignore = "uses the configured HTTPS test domain"]
fn deployed_settings_inputs_and_reload() {
    exercise_settings("https://solia.home.rethinkos.com/settings.solia");
}
