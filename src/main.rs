mod network;

use eframe::egui;
use network::Loaded;
use std::{sync::mpsc, thread};
use tokio::sync::watch;

const DEMO: &str = "https://solia.home.rethinkos.com/counter.solia";

#[derive(Clone, Default)]
struct Request {
    id: u64,
    address: String,
}
struct Completion {
    request: Request,
    result: Result<Loaded, String>,
}

struct Browser {
    address: String,
    requests: watch::Sender<Request>,
    results: mpsc::Receiver<Completion>,
    latest: u64,
    loading: bool,
    error: Option<String>,
    current: Option<Loaded>,
    reload_address: Option<String>,
}

impl Browser {
    fn new(ctx: egui::Context, address: String) -> Result<Self, String> {
        let client = network::client()?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let (requests, mut pending) = watch::channel(Request::default());
        let (done, results) = mpsc::channel();
        thread::Builder::new()
            .name("solia-loader".into())
            .spawn(move || {
                runtime.block_on(async move {
                    if pending.changed().await.is_err() {
                        return;
                    }
                    loop {
                        let request = pending.borrow_and_update().clone();
                        let result = tokio::select! {
                            biased;
                            changed = pending.changed() => {
                                if changed.is_err() { break; }
                                continue;
                            }
                            result = network::load(&client, &request.address) => result,
                        };
                        if done.send(Completion { request, result }).is_err() {
                            break;
                        }
                        ctx.request_repaint();
                        if pending.changed().await.is_err() {
                            break;
                        }
                    }
                })
            })
            .map_err(|e| e.to_string())?;
        let mut browser = Self {
            address: address.clone(),
            requests,
            results,
            latest: 0,
            loading: false,
            error: None,
            current: None,
            reload_address: None,
        };
        browser.navigate(address);
        Ok(browser)
    }

    fn navigate(&mut self, address: String) {
        self.latest += 1;
        self.error = None;
        self.loading = true;
        self.requests.send_replace(Request {
            id: self.latest,
            address: address.trim().to_owned(),
        });
    }

    fn accept_results(&mut self) {
        for completed in self.results.try_iter() {
            if completed.request.id != self.latest {
                continue;
            }
            self.loading = false;
            match completed.result {
                Ok(app) => {
                    self.reload_address = Some(completed.request.address);
                    self.current = Some(app);
                }
                Err(error) => self.error = Some(format!("{}\n{error}", completed.request.address)),
            }
        }
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        self.accept_results();
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("URL");
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.address)
                        .desired_width((ui.available_width() - 145.).max(80.))
                        .hint_text("https://…/app.solia"),
                );
                let submit = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Open").clicked() || submit {
                    self.navigate(self.address.clone());
                }
                if ui
                    .add_enabled(self.reload_address.is_some(), egui::Button::new("Reload"))
                    .clicked()
                    && let Some(address) = self.reload_address.clone()
                {
                    self.address = address.clone();
                    self.navigate(address);
                }
            });
            if self.loading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Loading source…");
                });
            }
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            if let Some(current) = &mut self.current {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(current.app.title());
                    ui.weak(current.origin.as_str());
                });
                ui.separator();
                ui.scope(|ui| current.app.render(ui));
            }
        });
    }
}

impl eframe::App for Browser {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.show(ui);
    }
}

fn main() -> eframe::Result {
    let address = std::env::args().nth(1).unwrap_or_else(|| DEMO.into());
    eframe::run_native(
        "Solia Browser",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([960., 640.]),
            ..Default::default()
        },
        Box::new(move |cc| Ok(Box::new(Browser::new(cc.egui_ctx.clone(), address)?))),
    )
}

#[cfg(test)]
mod tests;
