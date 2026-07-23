use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use directories::ProjectDirs;
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use image::{ExtendedColorType, codecs::jpeg::JpegEncoder};
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
const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");
const BG: Color32 = Color32::from_rgb(21, 20, 25);
const SURFACE: Color32 = Color32::from_rgb(29, 27, 33);
const SURFACE_STRONG: Color32 = Color32::from_rgb(37, 34, 41);
const INK: Color32 = Color32::from_rgb(247, 242, 237);
const MUTED: Color32 = Color32::from_rgb(174, 165, 176);
const FAINT: Color32 = Color32::from_rgb(112, 104, 115);
const ACCENT: Color32 = Color32::from_rgb(255, 128, 102);
const GREEN: Color32 = Color32::from_rgb(119, 185, 149);
const FEEDBACK_ENDPOINT: &str = "https://ttinker.net/chatcommons/api/app-feedback";

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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FeedbackReceipt {
    public_id: String,
    edit_token: String,
    status: String,
    #[serde(default)]
    admin_reply: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackPayload {
    surface: &'static str,
    screen: String,
    target_id: &'static str,
    target_text: &'static str,
    x: f32,
    y: f32,
    scroll_x: f32,
    scroll_y: f32,
    viewport_width: usize,
    viewport_height: usize,
    category: &'static str,
    priority: &'static str,
    message: String,
    screenshot: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackCreated {
    public_id: String,
    edit_token: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackStatus {
    public_id: String,
    status: String,
    admin_reply: String,
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
    feedback: PathBuf,
    node: PathBuf,
}

enum JobResult {
    Loaded(Snapshot),
    MessageSaved(Snapshot),
    Failed(String),
}

struct ChatCommonsApp {
    paths: Paths,
    snapshot: Snapshot,
    selected_channel: Option<String>,
    invite_code: String,
    draft: String,
    status: String,
    last_error: Option<String>,
    english: bool,
    feedback_open: bool,
    feedback_what: String,
    feedback_expected: String,
    feedback_confirmed: bool,
    feedback_notice: Option<String>,
    feedback_screenshot: Option<Arc<egui::ColorImage>>,
    feedback_texture: Option<egui::TextureHandle>,
    feedback_capture_pending: bool,
    feedback_job: Option<Receiver<Result<FeedbackReceipt, String>>>,
    feedback_receipt: Option<FeedbackReceipt>,
    job: Option<Receiver<JobResult>>,
}

impl ChatCommonsApp {
    fn new() -> Self {
        let paths = app_paths();
        let feedback_receipt = load_feedback_receipt(&paths.feedback).ok().flatten();
        let mut app = Self {
            paths,
            snapshot: Snapshot::default(),
            selected_channel: None,
            invite_code: String::new(),
            draft: String::new(),
            status: "正在准备本地身份……".into(),
            last_error: None,
            english: false,
            feedback_open: false,
            feedback_what: String::new(),
            feedback_expected: String::new(),
            feedback_confirmed: false,
            feedback_notice: None,
            feedback_screenshot: None,
            feedback_texture: None,
            feedback_capture_pending: false,
            feedback_job: None,
            feedback_receipt,
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
                self.adopt_snapshot(&snapshot);
                self.status = if snapshot.warning.is_some() {
                    if self.english {
                        "Home Server unavailable".into()
                    } else {
                        "社区服务器暂不可达".into()
                    }
                } else if self.english {
                    "Synchronized".into()
                } else {
                    "已同步".into()
                };
                self.last_error = snapshot.warning.clone();
                self.snapshot = snapshot;
            }
            JobResult::MessageSaved(snapshot) => {
                self.adopt_snapshot(&snapshot);
                self.snapshot = snapshot;
                self.status = if self.english {
                    "Saved locally · synchronizing…".into()
                } else {
                    "已在本地显示 · 正在同步……".into()
                };
                self.start_job(
                    if self.english {
                        "Synchronizing…"
                    } else {
                        "正在同步……"
                    },
                    |paths| load_snapshot(&paths, true),
                );
            }
            JobResult::Failed(error) => {
                self.status = if self.english {
                    "Operation failed · open Feedback for details".into()
                } else {
                    "操作失败 · 可在“反馈与问题”查看详情".into()
                };
                self.last_error = Some(error);
            }
        }
    }

    fn adopt_snapshot(&mut self, snapshot: &Snapshot) {
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
        if self.job.is_some() {
            return;
        }
        self.draft.clear();
        self.status = self.text("正在保存消息……", "Saving message…").into();
        let paths = self.paths.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = path_text(&paths.state)
                .and_then(|state| {
                    run_node(
                        &paths,
                        &[
                            "send-message",
                            "--state",
                            state,
                            "--community",
                            &community,
                            "--channel",
                            &channel,
                            "--text",
                            &text,
                        ],
                    )
                })
                .and_then(|_| load_snapshot(&paths, false));
            let _ = sender.send(match result {
                Ok(snapshot) => JobResult::MessageSaved(snapshot),
                Err(error) => JobResult::Failed(error),
            });
        });
        self.job = Some(receiver);
    }

    fn open_feedback(&mut self) {
        self.feedback_open = true;
        self.feedback_confirmed = false;
        self.feedback_notice = None;
    }

    fn issue_body(&self) -> String {
        let screen = if self.snapshot.community_id.is_some() {
            "community"
        } else {
            "join"
        };
        let error = self
            .last_error
            .as_deref()
            .map(redact_diagnostic)
            .unwrap_or_else(|| "No captured runtime error".into());
        format!(
            "## What happened\n\n{}\n\n## What did you expect?\n\n{}\n\n## App diagnostics\n\n- Version: `{PRODUCT_VERSION}`\n- OS: `{}`\n- Architecture: `{}`\n- Screen: `{screen}`\n- Community configured: `{}`\n- Selected channel: `{}`\n- Channels loaded: `{}`\n- Messages loaded: `{}`\n- Last error: `{error}`\n\n## Privacy confirmation\n\nThe reporter reviewed this text before submitting it to the private ChatCommons feedback inbox. Chat messages, invite links, identity keys, full user/community IDs, and local paths are intentionally excluded from diagnostics. An optional screenshot is submitted only after a separate capture action and may contain visible app content.\n",
            self.feedback_what.trim(),
            self.feedback_expected.trim(),
            std::env::consts::OS,
            std::env::consts::ARCH,
            self.snapshot.community_id.is_some(),
            self.selected_channel.is_some(),
            self.snapshot.channels.len(),
            self.snapshot.messages.len(),
        )
    }

    fn submit_feedback(&mut self) {
        if self.feedback_job.is_some() {
            return;
        }
        if self.feedback_what.trim().len() < 3 || self.feedback_expected.trim().len() < 3 {
            self.feedback_notice = Some(
                self.text(
                    "请分别说明发生了什么，以及你原本希望发生什么。",
                    "Describe what happened and what you expected.",
                )
                .into(),
            );
            return;
        }
        if !self.feedback_confirmed {
            self.feedback_notice = Some(
                self.text(
                    "请先确认预览中没有私人内容。",
                    "Confirm that the preview contains no private content.",
                )
                .into(),
            );
            return;
        }
        let report = self.issue_body();
        let screenshot = self.feedback_screenshot.clone();
        let screen = if self.snapshot.community_id.is_some() {
            "community".to_owned()
        } else {
            "join".to_owned()
        };
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(submit_app_feedback(report, screen, screenshot));
        });
        self.feedback_job = Some(receiver);
        self.feedback_notice = Some(
            self.text(
                "正在发送到私有反馈箱……",
                "Sending to the private feedback inbox…",
            )
            .into(),
        );
    }

    fn request_feedback_screenshot(&mut self, context: &egui::Context) {
        self.feedback_capture_pending = true;
        self.feedback_open = false;
        context.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
            "feedback-capture",
        )));
    }

    fn poll_feedback_screenshot(&mut self, context: &egui::Context) {
        if !self.feedback_capture_pending {
            return;
        }
        let captured = context.input(|input| {
            input.events.iter().find_map(|event| {
                if let egui::Event::Screenshot {
                    user_data, image, ..
                } = event
                    && user_data
                        .data
                        .as_ref()
                        .and_then(|data| data.downcast_ref::<&str>())
                        == Some(&"feedback-capture")
                {
                    return Some(image.clone());
                }
                None
            })
        });
        let Some(image) = captured else { return };
        self.feedback_texture = Some(context.load_texture(
            "feedback-screenshot-preview",
            (*image).clone(),
            egui::TextureOptions::LINEAR,
        ));
        self.feedback_screenshot = Some(image);
        self.feedback_capture_pending = false;
        self.feedback_open = true;
        self.feedback_notice = Some(
            self.text(
                "已截取应用窗口。请在提交前检查预览。",
                "App window captured. Review the preview before submitting.",
            )
            .into(),
        );
    }

    fn poll_feedback_job(&mut self) {
        let Some(receiver) = &self.feedback_job else {
            return;
        };
        let Ok(result) = receiver.try_recv() else {
            return;
        };
        self.feedback_job = None;
        match result {
            Ok(receipt) => {
                if let Err(error) = save_feedback_receipt(&self.paths.feedback, &receipt) {
                    self.feedback_notice = Some(error);
                } else {
                    self.feedback_notice = Some(
                        self.text(
                            "反馈已送达。你可以稍后在这里查看处理回复。",
                            "Feedback delivered. You can check the reply here later.",
                        )
                        .into(),
                    );
                }
                self.feedback_receipt = Some(receipt);
                self.feedback_what.clear();
                self.feedback_expected.clear();
                self.feedback_screenshot = None;
                self.feedback_texture = None;
                self.feedback_confirmed = false;
            }
            Err(error) => self.feedback_notice = Some(error),
        }
    }

    fn refresh_feedback_status(&mut self) {
        if self.feedback_job.is_some() {
            return;
        }
        let Some(receipt) = self.feedback_receipt.clone() else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(fetch_feedback_status(receipt));
        });
        self.feedback_job = Some(receiver);
        self.feedback_notice = Some(self.text("正在检查回复……", "Checking for a reply…").into());
    }
}

impl eframe::App for ChatCommonsApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = root.ctx().clone();
        self.poll_job();
        self.poll_feedback_job();
        self.poll_feedback_screenshot(&context);
        if self.job.is_some() || self.feedback_job.is_some() || self.feedback_capture_pending {
            root.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        egui::Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(egui::Margin::symmetric(24, 13))
                    .stroke(Stroke::new(1.0, SURFACE_STRONG)),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    brand_mark(ui);
                    ui.add_space(2.0);
                    ui.label(RichText::new("ChatCommons").size(17.0).strong().color(INK));
                    ui.label(
                        RichText::new(format!("v{PRODUCT_VERSION}"))
                            .size(9.0)
                            .color(ACCENT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(if self.english {
                                "中文"
                            } else {
                                "EN"
                            }))
                            .clicked()
                        {
                            self.english = !self.english;
                        }
                        let status_color = if self.last_error.is_some() {
                            ACCENT
                        } else if self.job.is_some() {
                            MUTED
                        } else {
                            GREEN
                        };
                        ui.label(RichText::new(&self.status).size(10.0).color(status_color));
                        ui.label(RichText::new("●").size(8.0).color(status_color));
                    });
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG))
            .show(root, |ui| {
                if self.snapshot.community_id.is_none() {
                    self.show_join(ui);
                } else {
                    self.show_chat(ui);
                }
            });
        self.show_feedback(&context);
    }
}

impl ChatCommonsApp {
    fn show_join(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width().min(760.0);
        ui.add_space((ui.available_height() * 0.08).max(24.0));
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(self.text(
                    "开源 · 社区自有 · 服务器可迁移",
                    "OPEN SOURCE · COMMUNITY-OWNED · PORTABLE",
                ))
                .size(9.0)
                .color(ACCENT)
                .strong(),
            );
            ui.add_space(10.0);
            ui.label(
                RichText::new(self.text(
                    "加入朋友的社区",
                    "Join your friends’ community",
                ))
                .size(38.0)
                .strong()
                .color(INK),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(self.text(
                    "身份已在这台设备上创建。粘贴朋友发来的单人邀请，剩下的交给客户端。",
                    "Your identity is ready on this device. Paste a one-person invite and the app handles the rest.",
                ))
                .size(13.0)
                .color(MUTED),
            );
            ui.add_space(28.0);
            egui::Frame::new()
                .fill(SURFACE)
                .corner_radius(22)
                .inner_margin(24)
                .stroke(Stroke::new(1.0, SURFACE_STRONG))
                .show(ui, |ui| {
                    ui.set_width(width - 48.0);
                    ui.label(
                        RichText::new(self.text("单人邀请", "ONE-PERSON INVITE"))
                            .size(9.0)
                            .color(FAINT)
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.add_sized(
                        [ui.available_width(), 118.0],
                        egui::TextEdit::multiline(&mut self.invite_code)
                            .hint_text("cc1_…")
                            .font(egui::TextStyle::Monospace),
                    );
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let join_label = if self.job.is_some() {
                            self.text("正在加入…", "Joining…")
                        } else {
                            self.text("加入社区 →", "Join community →")
                        };
                        if ui
                            .add_enabled(
                                self.job.is_none(),
                                egui::Button::new(RichText::new(join_label).strong())
                                    .fill(ACCENT)
                                    .min_size(Vec2::new(132.0, 42.0)),
                            )
                            .clicked()
                        {
                            self.join();
                        }
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(self.text(
                                "邀请只供一个人使用",
                                "Each invite is for one person",
                            ))
                            .size(10.0)
                            .color(FAINT),
                        );
                    });
                });
            if !self.snapshot.user_id.is_empty() {
                ui.add_space(18.0);
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        self.text("本机身份", "Local identity"),
                        short_id(&self.snapshot.user_id)
                    ))
                    .size(10.0)
                    .color(FAINT),
                );
            }
            ui.add_space(12.0);
            if ui
                .link(RichText::new(self.text("遇到问题？发送反馈", "Something wrong? Send feedback")).color(MUTED))
                .clicked()
            {
                self.open_feedback();
            }
        });
    }

    fn show_chat(&mut self, ui: &mut egui::Ui) {
        let height = ui.available_height();
        let width = ui.available_width();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(246.0, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.show_sidebar(ui),
            );
            ui.separator();
            ui.add_space(8.0);
            ui.allocate_ui_with_layout(
                Vec2::new((width - 266.0).max(360.0), height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.show_conversation(ui),
            );
        });
    }

    fn show_sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(SURFACE)
            .inner_margin(18)
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(210.0, (ui.available_height() - 36.0).max(300.0)));
                ui.set_max_width(210.0);
                ui.label(
                    RichText::new(self.text("永久测试社区", "PERMANENT TEST COMMUNITY"))
                        .size(8.0)
                        .color(ACCENT)
                        .strong(),
                );
                ui.add_space(5.0);
                ui.label(
                    RichText::new("chatcommonsTestCommunity")
                        .size(14.0)
                        .strong()
                        .color(INK),
                );
                ui.add_space(22.0);
                ui.label(
                    RichText::new(self.text("文字频道", "TEXT CHANNELS"))
                        .size(8.0)
                        .color(FAINT)
                        .strong(),
                );
                ui.add_space(6.0);
                for channel in self.snapshot.channels.clone() {
                    let selected = self.selected_channel.as_deref() == Some(&channel.channel_id);
                    if ui
                        .add_sized(
                            [210.0, 38.0],
                            egui::Button::new(
                                RichText::new(format!("#  {}", channel.name)).color(if selected {
                                    INK
                                } else {
                                    MUTED
                                }),
                            )
                            .selected(selected)
                            .fill(if selected {
                                SURFACE_STRONG
                            } else {
                                Color32::TRANSPARENT
                            }),
                        )
                        .clicked()
                    {
                        self.selected_channel = Some(channel.channel_id);
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    if ui
                        .button(self.text("反馈与问题", "Feedback & issues"))
                        .clicked()
                    {
                        self.open_feedback();
                    }
                    if ui
                        .add_enabled(
                            self.job.is_none(),
                            egui::Button::new(self.text("↻  同步", "↻  Sync"))
                                .fill(Color32::TRANSPARENT),
                        )
                        .clicked()
                    {
                        self.refresh();
                    }
                    ui.label(
                        RichText::new(format!(
                            "{}  {}",
                            self.text("本机身份", "Local identity"),
                            short_id(&self.snapshot.user_id)
                        ))
                        .size(9.0)
                        .color(FAINT),
                    );
                });
            });
    }

    fn show_conversation(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(360.0);
        let channel_name = self
            .snapshot
            .channels
            .iter()
            .find(|channel| Some(&channel.channel_id) == self.selected_channel.as_ref())
            .map(|channel| channel.name.as_str())
            .unwrap_or(self.text("消息", "Messages"));
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("#  {channel_name}"))
                    .size(22.0)
                    .strong()
                    .color(INK),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(self.text(
                        "消息由设备签名 · 服务器可迁移",
                        "Device-signed · Server portable",
                    ))
                    .size(9.0)
                    .color(FAINT),
                );
            });
        });
        ui.separator();
        let message_height = (ui.available_height() - 68.0).max(120.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), message_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(12.0);
                        let messages: Vec<Message> = self
                            .snapshot
                            .messages
                            .iter()
                            .filter(|message| {
                                Some(&message.channel_id) == self.selected_channel.as_ref()
                            })
                            .cloned()
                            .collect();
                        if messages.is_empty() {
                            ui.add_space(84.0);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new("●").size(18.0).color(ACCENT));
                                ui.label(
                                    RichText::new(self.text(
                                        "这是这个频道的开始。",
                                        "This is the beginning of the channel.",
                                    ))
                                    .size(17.0)
                                    .strong()
                                    .color(INK),
                                );
                                ui.label(
                                    RichText::new(self.text(
                                        "发一条消息，让朋友知道你已经到了。",
                                        "Send a message so your friends know you made it.",
                                    ))
                                    .size(11.0)
                                    .color(MUTED),
                                );
                            });
                        }
                        for message in messages {
                            self.show_message(ui, &message);
                        }
                    });
            },
        );
        egui::Frame::new()
            .fill(SURFACE_STRONG)
            .corner_radius(15)
            .inner_margin(7)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let hint = self.text("给这个频道发消息…", "Message this channel…");
                    let response = ui.add_sized(
                        [(ui.available_width() - 80.0).max(200.0), 38.0],
                        egui::TextEdit::singleline(&mut self.draft).hint_text(hint),
                    );
                    let enter = response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    if (ui
                        .add_enabled(
                            self.job.is_none(),
                            egui::Button::new(RichText::new(self.text("发送", "Send")).strong())
                                .fill(ACCENT)
                                .min_size(Vec2::new(64.0, 36.0)),
                        )
                        .clicked()
                        || enter)
                        && self.job.is_none()
                    {
                        self.send();
                    }
                });
            });
    }

    fn show_message(&self, ui: &mut egui::Ui, message: &Message) {
        ui.horizontal_top(|ui| {
            avatar(ui, &message.author_id);
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let author = if message.author_id == self.snapshot.user_id {
                        self.text("你", "You").to_owned()
                    } else {
                        format!("member {}", short_id(&message.author_id))
                    };
                    ui.label(RichText::new(author).size(12.0).strong().color(INK));
                    ui.label(
                        RichText::new(format!(
                            "{} · signed · {}",
                            message_time(message.timestamp_ms),
                            short_id(&message.event_id)
                        ))
                        .size(8.0)
                        .color(FAINT),
                    );
                });
                ui.label(RichText::new(&message.text).size(13.0).color(INK));
            });
        });
        ui.add_space(14.0);
    }

    fn show_feedback(&mut self, context: &egui::Context) {
        if !self.feedback_open {
            return;
        }
        let mut open = self.feedback_open;
        let mut submit = false;
        let mut capture = false;
        let mut remove_screenshot = false;
        let mut refresh_status = false;
        let english = self.english;
        let mut preview = self.issue_body();
        let form_height = (context.content_rect().height() - 260.0).clamp(220.0, 520.0);
        egui::Window::new(if english {
            "Feedback & issue report"
        } else {
            "反馈与问题报告"
        })
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(620.0)
        .default_height(680.0)
        .show(context, |ui| {
            ui.label(
                RichText::new(if english {
                    "Nothing is uploaded automatically. Text and an optional app-window screenshot go only to the private ChatCommons feedback inbox after your confirmation."
                } else {
                    "不会自动上传任何内容。确认后，文字和你主动截取的应用窗口截图只会进入 ChatCommons 私有反馈箱。"
                })
                .color(MUTED),
            );
            ui.add_space(10.0);
            egui::ScrollArea::vertical()
                .id_salt("feedback-form-scroll")
                .max_height(form_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
            ui.label(RichText::new(if english { "What happened?" } else { "发生了什么？" }).strong());
            ui.add_sized(
                [ui.available_width(), 72.0],
                egui::TextEdit::multiline(&mut self.feedback_what)
                    .hint_text(if english {
                        "Describe the steps and what went wrong."
                    } else {
                        "说明操作步骤和出现的问题。"
                    }),
            );
            ui.label(RichText::new(if english { "What did you expect?" } else { "你原本希望发生什么？" }).strong());
            ui.add_sized(
                [ui.available_width(), 58.0],
                egui::TextEdit::multiline(&mut self.feedback_expected)
                    .hint_text(if english {
                        "Describe the expected result."
                    } else {
                        "说明你认为正确的结果。"
                    }),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if self.feedback_screenshot.is_none() {
                    if ui
                        .button(if english {
                            "Capture app window"
                        } else {
                            "截取当前应用窗口"
                        })
                        .clicked()
                    {
                        capture = true;
                    }
                    ui.label(
                        RichText::new(if english {
                            "Optional. It may include visible messages."
                        } else {
                            "可选；截图可能包含当前可见消息。"
                        })
                        .size(9.0)
                        .color(FAINT),
                    );
                } else if ui
                    .button(if english {
                        "Remove screenshot"
                    } else {
                        "移除截图"
                    })
                    .clicked()
                {
                    remove_screenshot = true;
                }
            });
            if let Some(texture) = &self.feedback_texture {
                let max_width = ui.available_width();
                let source = texture.size_vec2();
                let scale = (max_width / source.x).min(220.0 / source.y).min(1.0);
                ui.image((texture.id(), source * scale));
            }
            ui.add_space(8.0);
            ui.label(RichText::new(if english { "Full report preview" } else { "完整报告预览" }).strong());
            ui.add_sized(
                [ui.available_width(), 180.0],
                egui::TextEdit::multiline(&mut preview)
                    .font(egui::TextStyle::Monospace)
                    .interactive(false),
            );
                });
            ui.separator();
            if let Some(receipt) = &self.feedback_receipt {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} · {}",
                            receipt.public_id,
                            feedback_status_text(&receipt.status, english)
                        ))
                        .color(MUTED),
                    );
                    if ui
                        .add_enabled(
                            self.feedback_job.is_none(),
                            egui::Button::new(if english {
                                "Check reply"
                            } else {
                                "检查处理回复"
                            }),
                        )
                        .clicked()
                    {
                        refresh_status = true;
                    }
                });
                if !receipt.admin_reply.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "{}：{}",
                            if english { "Reply" } else { "回复" },
                            receipt.admin_reply
                        ))
                        .color(GREEN),
                    );
                }
                ui.separator();
            }
            ui.checkbox(
                &mut self.feedback_confirmed,
                if english {
                    "I reviewed the report and optional screenshot and agree to send them to the private feedback inbox."
                } else {
                    "我已检查报告和可选截图，同意将它们发送到私有反馈箱。"
                },
            );
            if let Some(notice) = &self.feedback_notice {
                ui.label(RichText::new(notice).color(ACCENT));
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        self.feedback_confirmed && self.feedback_job.is_none(),
                        egui::Button::new(if english {
                            "Send feedback →"
                        } else {
                            "发送反馈 →"
                        })
                        .fill(ACCENT),
                    )
                    .clicked()
                {
                    submit = true;
                }
                ui.label(
                    RichText::new(if english {
                        "No GitHub account is required."
                    } else {
                        "不需要 GitHub 账号。"
                    })
                    .size(9.0)
                    .color(FAINT),
                );
            });
        });
        self.feedback_open = open;
        if capture {
            self.request_feedback_screenshot(context);
        }
        if remove_screenshot {
            self.feedback_screenshot = None;
            self.feedback_texture = None;
        }
        if refresh_status {
            self.refresh_feedback_status();
        }
        if submit {
            self.submit_feedback();
        }
    }
}

impl Clone for Paths {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            config: self.config.clone(),
            feedback: self.feedback.clone(),
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
        feedback: base.join("feedback-receipt.json"),
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

fn load_feedback_receipt(path: &Path) -> Result<Option<FeedbackReceipt>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|error| format!("无法读取反馈回执：{error}"))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("反馈回执无效：{error}"))
}

fn save_feedback_receipt(path: &Path, receipt: &FeedbackReceipt) -> Result<(), String> {
    let parent = path.parent().ok_or("反馈回执目录无效")?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建反馈回执目录：{error}"))?;
    let bytes =
        serde_json::to_vec(receipt).map_err(|error| format!("无法编码反馈回执：{error}"))?;
    fs::write(path, bytes).map_err(|error| format!("无法保存反馈回执：{error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("无法保护反馈回执：{error}"))?;
    }
    Ok(())
}

fn run_node(paths: &Paths, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new(&paths.node)
        .args(arguments)
        .output()
        .map_err(|error| format!("无法启动协议进程：{error}"))?;
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

fn message_time(timestamp_ms: i64) -> String {
    let seconds = (timestamp_ms / 1000).rem_euclid(86_400);
    format!("{:02}:{:02} UTC", seconds / 3_600, (seconds % 3_600) / 60)
}

fn brand_mark(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(29.0, 25.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 8.0, INK);
    for offset in [8.0, 14.5, 21.0] {
        painter.circle_filled(egui::pos2(rect.left() + offset, rect.center().y), 1.7, BG);
    }
    painter.line_segment(
        [
            egui::pos2(rect.left() + 5.0, rect.bottom() - 1.0),
            egui::pos2(rect.left() + 2.0, rect.bottom() + 3.0),
        ],
        Stroke::new(3.0, INK),
    );
}

fn avatar(ui: &mut egui::Ui, author_id: &str) {
    let colors = [
        Color32::from_rgb(239, 111, 85),
        Color32::from_rgb(103, 152, 223),
        Color32::from_rgb(119, 185, 149),
        Color32::from_rgb(157, 135, 215),
        Color32::from_rgb(221, 180, 93),
    ];
    let index = author_id
        .bytes()
        .fold(0_usize, |value, byte| value.wrapping_add(byte as usize))
        % colors.len();
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 12.0, colors[index]);
    let initial = author_id.chars().next().unwrap_or('?').to_ascii_uppercase();
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initial,
        egui::FontId::proportional(13.0),
        Color32::WHITE,
    );
}

fn redact_diagnostic(value: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let value = if home.is_empty() {
        value.to_owned()
    } else {
        value.replace(&home, "$HOME")
    };
    let mut output = String::with_capacity(value.len());
    let mut run = String::new();
    let flush = |run: &mut String, output: &mut String| {
        if run.starts_with("cc1_") {
            output.push_str("[redacted invite]");
        } else if run.len() >= 32
            && run
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        {
            output.push_str("[redacted identifier]");
        } else {
            output.push_str(run);
        }
        run.clear();
    };
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            run.push(character);
        } else {
            flush(&mut run, &mut output);
            output.push(character);
        }
    }
    flush(&mut run, &mut output);
    output
        .split_whitespace()
        .map(|token| {
            let candidate = token.trim_start_matches(['"', '\'', '(', '[', '{', '=', ':']);
            let is_network_multiaddr = [
                "/ip4/", "/ip6/", "/dns/", "/dns4/", "/dns6/", "/udp/", "/tcp/", "/p2p/",
            ]
            .iter()
            .any(|prefix| candidate.starts_with(prefix));
            let bytes = candidate.as_bytes();
            let is_windows_path =
                bytes.len() >= 3 && bytes[1] == b':' && matches!(bytes[2], b'\\' | b'/');
            if candidate.starts_with("$HOME")
                || candidate.starts_with("file://")
                || (candidate.starts_with('/') && !is_network_multiaddr)
                || is_windows_path
            {
                "[redacted path]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn screenshot_data_url(image: &egui::ColorImage) -> Result<String, String> {
    for (max_width, quality) in [(1_280_usize, 78_u8), (960, 64), (720, 54)] {
        let source_width = image.size[0];
        let source_height = image.size[1];
        if source_width == 0 || source_height == 0 {
            return Err("截图尺寸无效".into());
        }
        let scale = (max_width as f32 / source_width as f32).min(1.0);
        let width = ((source_width as f32 * scale).round() as usize).max(1);
        let height = ((source_height as f32 * scale).round() as usize).max(1);
        let mut rgb = Vec::with_capacity(width * height * 3);
        for y in 0..height {
            let source_y = y * source_height / height;
            for x in 0..width {
                let source_x = x * source_width / width;
                let pixel = image.pixels[source_y * source_width + source_x];
                rgb.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b()]);
            }
        }
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, quality)
            .encode(&rgb, width as u32, height as u32, ExtendedColorType::Rgb8)
            .map_err(|error| format!("无法编码截图：{error}"))?;
        if jpeg.len() <= 950_000 {
            return Ok(format!("data:image/jpeg;base64,{}", BASE64.encode(jpeg)));
        }
    }
    Err("截图压缩后仍然过大，请移除截图后提交".into())
}

fn submit_app_feedback(
    report: String,
    screen: String,
    screenshot: Option<Arc<egui::ColorImage>>,
) -> Result<FeedbackReceipt, String> {
    let (screenshot, width, height) = if let Some(image) = screenshot {
        (
            screenshot_data_url(&image)?,
            image.size[0].clamp(280, 10_000),
            image.size[1].clamp(300, 10_000),
        )
    } else {
        (String::new(), 1_180, 760)
    };
    let payload = FeedbackPayload {
        surface: "desktop",
        screen,
        target_id: "chat-window",
        target_text: "ChatCommons desktop feedback",
        x: 0.5,
        y: 0.5,
        scroll_x: 0.0,
        scroll_y: 0.0,
        viewport_width: width,
        viewport_height: height,
        category: "feature",
        priority: "normal",
        message: report,
        screenshot,
    };
    let mut response = ureq::post(FEEDBACK_ENDPOINT)
        .header(
            "User-Agent",
            &format!("ChatCommonsDesktop/{PRODUCT_VERSION}"),
        )
        .send_json(&payload)
        .map_err(|error| format!("反馈发送失败：{error}"))?;
    let created: FeedbackCreated = response
        .body_mut()
        .read_json()
        .map_err(|error| format!("反馈回执无效：{error}"))?;
    Ok(FeedbackReceipt {
        public_id: created.public_id,
        edit_token: created.edit_token,
        status: created.status,
        admin_reply: String::new(),
    })
}

fn fetch_feedback_status(receipt: FeedbackReceipt) -> Result<FeedbackReceipt, String> {
    let endpoint = format!("{FEEDBACK_ENDPOINT}/{}", receipt.public_id);
    let mut response = ureq::get(&endpoint)
        .header("X-Edit-Token", &receipt.edit_token)
        .header(
            "User-Agent",
            &format!("ChatCommonsDesktop/{PRODUCT_VERSION}"),
        )
        .call()
        .map_err(|error| format!("无法检查反馈状态：{error}"))?;
    let status: FeedbackStatus = response
        .body_mut()
        .read_json()
        .map_err(|error| format!("反馈状态无效：{error}"))?;
    if status.public_id != receipt.public_id {
        return Err("反馈回执编号不匹配".into());
    }
    Ok(FeedbackReceipt {
        status: status.status,
        admin_reply: status.admin_reply,
        ..receipt
    })
}

fn feedback_status_text(status: &str, english: bool) -> &'static str {
    match (status, english) {
        ("pending", false) => "待确认",
        ("pending", true) => "Pending",
        ("in_progress", false) => "处理中",
        ("in_progress", true) => "In progress",
        ("client_review", false) => "待你验收",
        ("client_review", true) => "Ready for your review",
        ("completed", false) => "已完成",
        ("completed", true) => "Completed",
        ("rejected", false) => "暂不处理",
        ("rejected", true) => "Not planned",
        (_, false) => "状态未知",
        (_, true) => "Unknown status",
    }
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
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([760.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ChatCommons Alpha",
        options,
        Box::new(|context| {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = BG;
            visuals.window_fill = SURFACE;
            visuals.extreme_bg_color = SURFACE;
            visuals.faint_bg_color = SURFACE_STRONG;
            visuals.selection.bg_fill = ACCENT;
            visuals.selection.stroke = Stroke::new(1.0, INK);
            visuals.widgets.noninteractive.bg_fill = SURFACE;
            visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
            visuals.widgets.inactive.bg_fill = SURFACE_STRONG;
            visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, MUTED);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(49, 45, 53);
            visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, INK);
            visuals.widgets.active.bg_fill = ACCENT;
            visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
            context.egui_ctx.set_visuals(visuals);
            let mut style = (*context.egui_ctx.style_of(egui::Theme::Dark)).clone();
            style.spacing.item_spacing = Vec2::new(10.0, 8.0);
            style.spacing.button_padding = Vec2::new(12.0, 8.0);
            context.egui_ctx.set_style_of(egui::Theme::Dark, style);
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

    #[test]
    fn issue_diagnostics_redact_invites_and_long_identifiers() {
        let input = "invite cc1_secretvalue and 0123456789abcdef0123456789abcdef0123456789abcdef paths /tmp/private/client.json C:\\Users\\friend\\state but network /ip4/47.254.94.170/udp/4001";
        let redacted = redact_diagnostic(input);
        assert!(!redacted.contains("cc1_secretvalue"));
        assert!(!redacted.contains("0123456789abcdef"));
        assert!(!redacted.contains("/tmp/private"));
        assert!(!redacted.contains("C:\\Users"));
        assert!(redacted.contains("[redacted invite]"));
        assert!(redacted.contains("[redacted identifier]"));
        assert!(redacted.contains("[redacted path]"));
        assert!(redacted.contains("/ip4/47.254.94.170/udp/4001"));
    }

    #[test]
    fn feedback_receipt_round_trips_with_private_edit_token()
    -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("feedback-receipt.json");
        let receipt = FeedbackReceipt {
            public_id: "RV-example".into(),
            edit_token: "secret-edit-token".into(),
            status: "pending".into(),
            admin_reply: String::new(),
        };
        save_feedback_receipt(&path, &receipt)?;
        let loaded = load_feedback_receipt(&path)?.ok_or("missing receipt")?;
        assert_eq!(loaded.public_id, receipt.public_id);
        assert_eq!(loaded.edit_token, receipt.edit_token);
        assert_eq!(loaded.status, "pending");
        Ok(())
    }

    #[test]
    fn screenshot_is_encoded_as_a_bounded_real_jpeg() {
        let image = egui::ColorImage::filled([4, 4], Color32::from_rgb(10, 20, 30));
        let encoded = screenshot_data_url(&image).expect("screenshot should encode");
        assert!(encoded.starts_with("data:image/jpeg;base64,/9j/"));
        assert!(encoded.len() < 1_500_000);
    }

    #[test]
    fn message_time_uses_stable_utc_clock_metadata() {
        assert_eq!(message_time(3_661_000), "01:01 UTC");
    }
}
