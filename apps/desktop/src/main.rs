use directories::ProjectDirs;
use eframe::egui::{self, Color32, RichText};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        mpsc::{self, Receiver},
    },
    thread,
};

const CONFIG_VERSION: u16 = 1;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Channel {
    channel_id: String,
    name: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Message {
    event_id: String,
    channel_id: String,
    author_id: String,
    timestamp_ms: i64,
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClientConfig {
    version: u16,
    community_id: Option<String>,
}

#[derive(Default)]
struct Snapshot {
    user_id: String,
    community_id: Option<String>,
    channels: Vec<Channel>,
    messages: Vec<Message>,
    warning: Option<String>,
}

struct Paths {
    state: PathBuf,
    config: PathBuf,
    node: PathBuf,
}

enum JobResult {
    Loaded(Snapshot),
    Failed(String),
}

struct ChatCommonsApp {
    paths: Paths,
    snapshot: Snapshot,
    selected_channel: Option<String>,
    invite_code: String,
    draft: String,
    status: String,
    english: bool,
    job: Option<Receiver<JobResult>>,
}

impl ChatCommonsApp {
    fn new() -> Self {
        let paths = app_paths();
        let mut app = Self {
            paths,
            snapshot: Snapshot::default(),
            selected_channel: None,
            invite_code: String::new(),
            draft: String::new(),
            status: "正在准备本地身份……".into(),
            english: false,
            job: None,
        };
        app.start_job("正在准备本地身份……", |paths| {
            load_snapshot(&paths, true)
        });
        app
    }

    fn text<'a>(&self, chinese: &'a str, english: &'a str) -> &'a str {
        if self.english { english } else { chinese }
    }

    fn start_job(
        &mut self,
        status: &str,
        operation: impl FnOnce(Paths) -> Result<Snapshot, String> + Send + 'static,
    ) {
        if self.job.is_some() {
            return;
        }
        self.status = status.into();
        let paths = self.paths.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = match operation(paths) {
                Ok(snapshot) => JobResult::Loaded(snapshot),
                Err(error) => JobResult::Failed(error),
            };
            let _ = sender.send(result);
        });
        self.job = Some(receiver);
    }

    fn poll_job(&mut self) {
        let Some(receiver) = &self.job else { return };
        let Ok(result) = receiver.try_recv() else {
            return;
        };
        self.job = None;
        match result {
            JobResult::Loaded(snapshot) => {
                self.selected_channel = self
                    .selected_channel
                    .take()
                    .filter(|selected| snapshot.channels.iter().any(|c| &c.channel_id == selected))
                    .or_else(|| {
                        snapshot
                            .channels
                            .first()
                            .map(|channel| channel.channel_id.clone())
                    });
                self.status = snapshot.warning.clone().unwrap_or_else(|| {
                    if self.english {
                        "Synchronized".into()
                    } else {
                        "已同步".into()
                    }
                });
                self.snapshot = snapshot;
            }
            JobResult::Failed(error) => self.status = error,
        }
    }

    fn join(&mut self) {
        let invite = self.invite_code.trim().to_owned();
        if invite.is_empty() {
            self.status = self.text("请先粘贴邀请", "Paste an invite first").into();
            return;
        }
        self.start_job(
            if self.english {
                "Joining…"
            } else {
                "正在加入……"
            },
            move |paths| {
                ensure_identity(&paths)?;
                let output = run_node(
                    &paths,
                    &[
                        "join",
                        "--state",
                        path_text(&paths.state)?,
                        "--invite-code",
                        &invite,
                    ],
                )?;
                let community = output_field(&output, "COMMUNITY_ID")?;
                save_config(&paths.config, Some(community))?;
                load_snapshot(&paths, false)
            },
        );
    }

    fn refresh(&mut self) {
        self.start_job(
            if self.english {
                "Synchronizing…"
            } else {
                "正在同步……"
            },
            |paths| load_snapshot(&paths, true),
        );
    }

    fn send(&mut self) {
        let text = self.draft.trim().to_owned();
        let Some(channel) = self.selected_channel.clone() else {
            self.status = self.text("没有可用频道", "No channel is available").into();
            return;
        };
        let Some(community) = self.snapshot.community_id.clone() else {
            return;
        };
        if text.is_empty() {
            return;
        }
        self.draft.clear();
        self.start_job(
            if self.english {
                "Sending…"
            } else {
                "正在发送……"
            },
            move |paths| {
                run_node(
                    &paths,
                    &[
                        "send-message",
                        "--state",
                        path_text(&paths.state)?,
                        "--community",
                        &community,
                        "--channel",
                        &channel,
                        "--text",
                        &text,
                    ],
                )?;
                load_snapshot(&paths, true)
            },
        );
    }
}

impl eframe::App for ChatCommonsApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_job();
        if self.job.is_some() {
            root.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        egui::Panel::top("header").show(root, |ui| {
            ui.horizontal(|ui| {
                ui.heading(RichText::new("ChatCommons").strong());
                ui.label(RichText::new("0.1 alpha").color(Color32::from_rgb(255, 128, 102)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(if self.english { "中文" } else { "EN" })
                        .clicked()
                    {
                        self.english = !self.english;
                    }
                    ui.label(&self.status);
                });
            });
        });
        egui::CentralPanel::default().show(root, |ui| {
            if self.snapshot.community_id.is_none() {
                ui.add_space(36.0);
                ui.vertical_centered(|ui| {
                    ui.heading(self.text("加入朋友的社区", "Join your friends' community"));
                    ui.label(self.text(
                        "身份已经在本机创建。粘贴朋友发来的单人邀请即可加入。",
                        "Your identity is ready on this device. Paste a single-person invite to join.",
                    ));
                    ui.add_space(16.0);
                    ui.add_sized(
                        [ui.available_width().min(620.0), 130.0],
                        egui::TextEdit::multiline(&mut self.invite_code).hint_text("cc1_…"),
                    );
                    ui.add_space(8.0);
                    if ui
                        .add_enabled(
                            self.job.is_none(),
                            egui::Button::new(self.text("加入社区", "Join community")),
                        )
                        .clicked()
                    {
                        self.join();
                    }
                    if !self.snapshot.user_id.is_empty() {
                        ui.add_space(18.0);
                        ui.small(format!("User ID · {}", short_id(&self.snapshot.user_id)));
                    }
                });
                return;
            }

            ui.columns(2, |columns| {
                columns[0].set_min_width(170.0);
                columns[0].heading(self.text("频道", "Channels"));
                for channel in &self.snapshot.channels {
                    let selected = self.selected_channel.as_deref() == Some(&channel.channel_id);
                    if columns[0]
                        .selectable_label(selected, format!("# {}", channel.name))
                        .clicked()
                    {
                        self.selected_channel = Some(channel.channel_id.clone());
                    }
                }
                if columns[0]
                    .add_enabled(self.job.is_none(), egui::Button::new(self.text("同步", "Sync")))
                    .clicked()
                {
                    self.refresh();
                }

                columns[1].heading(
                    self.snapshot
                        .channels
                        .iter()
                        .find(|channel| Some(&channel.channel_id) == self.selected_channel.as_ref())
                        .map(|channel| format!("# {}", channel.name))
                        .unwrap_or_else(|| self.text("消息", "Messages").into()),
                );
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(&mut columns[1], |ui| {
                        for message in self.snapshot.messages.iter().filter(|message| {
                            Some(&message.channel_id) == self.selected_channel.as_ref()
                        }) {
                            ui.group(|ui| {
                                let author = if message.author_id == self.snapshot.user_id {
                                    self.text("你", "You").to_owned()
                                } else {
                                    short_id(&message.author_id)
                                };
                                ui.horizontal(|ui| {
                                    ui.strong(author);
                                    ui.small(format!("{} · {}", message.timestamp_ms, short_id(&message.event_id)));
                                });
                                ui.label(&message.text);
                            });
                        }
                });
                columns[1].separator();
                columns[1].horizontal(|ui| {
                    let message_hint = if self.english {
                        "Send a message"
                    } else {
                        "发送一条消息"
                    };
                    let response = ui.add_sized(
                        [ui.available_width() - 72.0, 42.0],
                        egui::TextEdit::singleline(&mut self.draft).hint_text(message_hint),
                    );
                    let enter = response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    if (ui
                        .add_enabled(self.job.is_none(), egui::Button::new(self.text("发送", "Send")))
                        .clicked()
                        || enter)
                        && self.job.is_none()
                    {
                        self.send();
                    }
                });
            });
        });
    }
}

impl Clone for Paths {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            config: self.config.clone(),
            node: self.node.clone(),
        }
    }
}

fn app_paths() -> Paths {
    let base = ProjectDirs::from("net", "ttinker", "ChatCommonsAlpha")
        .map(|dirs| dirs.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".chatcommons-alpha"));
    let node = std::env::var_os("CHATCOMMONS_NODE_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe().ok().and_then(|path| {
                path.parent().map(|parent| {
                    parent.join(if cfg!(windows) {
                        "chatcommons-node.exe"
                    } else {
                        "chatcommons-node"
                    })
                })
            })
        })
        .unwrap_or_else(|| PathBuf::from("chatcommons-node"));
    Paths {
        state: base.join("node"),
        config: base.join("client.json"),
        node,
    }
}

fn ensure_identity(paths: &Paths) -> Result<(), String> {
    if paths.state.join("identity.json").is_file() {
        return Ok(());
    }
    fs::create_dir_all(paths.state.parent().ok_or("应用数据目录无效")?)
        .map_err(|error| format!("无法创建应用数据目录：{error}"))?;
    run_node(paths, &["init", "--state", path_text(&paths.state)?]).map(|_| ())
}

fn load_snapshot(paths: &Paths, synchronize: bool) -> Result<Snapshot, String> {
    ensure_identity(paths)?;
    let info = run_node(paths, &["info", "--state", path_text(&paths.state)?])?;
    let user_id = output_field(&info, "USER_ID")?;
    let config = load_config(&paths.config)?;
    let Some(community) = config.community_id else {
        return Ok(Snapshot {
            user_id,
            ..Snapshot::default()
        });
    };
    let warning = if synchronize {
        run_node(
            paths,
            &[
                "sync-home-server",
                "--state",
                path_text(&paths.state)?,
                "--community",
                &community,
                "--listen",
                "/ip4/0.0.0.0/udp/0/quic-v1",
                "--idle-timeout-ms",
                "1500",
            ],
        )
        .err()
    } else {
        None
    };
    let channels = run_node(
        paths,
        &[
            "list-channels",
            "--state",
            path_text(&paths.state)?,
            "--community",
            &community,
        ],
    )?;
    let messages = run_node(
        paths,
        &[
            "list-messages",
            "--state",
            path_text(&paths.state)?,
            "--community",
            &community,
        ],
    )?;
    Ok(Snapshot {
        user_id,
        community_id: Some(community),
        channels: serde_json::from_str(&channels)
            .map_err(|error| format!("频道数据无效：{error}"))?,
        messages: serde_json::from_str(&messages)
            .map_err(|error| format!("消息数据无效：{error}"))?,
        warning,
    })
}

fn load_config(path: &Path) -> Result<ClientConfig, String> {
    if !path.is_file() {
        return Ok(ClientConfig {
            version: CONFIG_VERSION,
            community_id: None,
        });
    }
    let bytes = fs::read(path).map_err(|error| format!("无法读取客户端配置：{error}"))?;
    let config: ClientConfig =
        serde_json::from_slice(&bytes).map_err(|error| format!("客户端配置无效：{error}"))?;
    if config.version != CONFIG_VERSION {
        return Err("客户端配置版本不受支持".into());
    }
    Ok(config)
}

fn save_config(path: &Path, community_id: Option<String>) -> Result<(), String> {
    let parent = path.parent().ok_or("客户端配置目录无效")?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建客户端配置目录：{error}"))?;
    let bytes = serde_json::to_vec(&ClientConfig {
        version: CONFIG_VERSION,
        community_id,
    })
    .map_err(|error| format!("无法编码客户端配置：{error}"))?;
    fs::write(path, bytes).map_err(|error| format!("无法保存客户端配置：{error}"))
}

fn run_node(paths: &Paths, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new(&paths.node)
        .args(arguments)
        .output()
        .map_err(|error| format!("无法启动协议进程 {}：{error}", paths.node.display()))?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|error| format!("协议输出不是 UTF-8：{error}"))
    } else {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if error.is_empty() {
            "协议操作失败".into()
        } else {
            error
        })
    }
}

fn output_field(output: &str, name: &str) -> Result<String, String> {
    let prefix = format!("{name}=");
    output
        .lines()
        .find_map(|line| line.strip_prefix(&prefix).map(str::to_owned))
        .ok_or_else(|| format!("协议输出缺少 {name}"))
}

fn path_text(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| "应用数据路径不是有效 UTF-8".into())
}

fn short_id(value: &str) -> String {
    value.chars().take(10).collect()
}

fn install_system_cjk_font(context: &egui::Context) {
    let candidates = if cfg!(target_os = "macos") {
        vec!["/System/Library/Fonts/PingFang.ttc"]
    } else if cfg!(target_os = "windows") {
        vec![r"C:\Windows\Fonts\msyh.ttc", r"C:\Windows\Fonts\msyhbd.ttc"]
    } else {
        vec![
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        ]
    };
    let Some(bytes) = candidates.into_iter().find_map(|path| fs::read(path).ok()) else {
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    let name = "system-cjk".to_owned();
    fonts
        .font_data
        .insert(name.clone(), Arc::new(egui::FontData::from_owned(bytes)));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push(name.clone());
    }
    context.set_fonts(fonts);
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 640.0])
            .with_min_inner_size([640.0, 460.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ChatCommons Alpha",
        options,
        Box::new(|context| {
            context.egui_ctx.set_visuals(egui::Visuals::dark());
            install_system_cjk_font(&context.egui_ctx);
            Ok(Box::new(ChatCommonsApp::new()))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_fields_are_parsed_without_accepting_partial_names() {
        let output = "USER_ID=abc\nCOMMUNITY_ID=def\n";
        assert_eq!(output_field(output, "USER_ID").as_deref(), Ok("abc"));
        assert_eq!(output_field(output, "COMMUNITY_ID").as_deref(), Ok("def"));
        assert!(output_field(output, "ID").is_err());
    }

    #[test]
    fn client_config_round_trips_and_rejects_unknown_versions()
    -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("client.json");
        save_config(&path, Some("ab".repeat(32)))?;
        let config = load_config(&path)?;
        assert_eq!(
            config.community_id.as_deref(),
            Some("ab".repeat(32).as_str())
        );

        fs::write(&path, br#"{"version":2,"community_id":null}"#)?;
        assert!(load_config(&path).is_err());
        Ok(())
    }

    #[test]
    fn identifiers_are_shortened_for_display_only() {
        assert_eq!(short_id("0123456789abcdef"), "0123456789");
    }
}
