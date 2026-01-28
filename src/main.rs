#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod crypto;
mod csv;
mod externals;
mod models;
use crate::models::{AddressLabel, AdvisoryLevel};
use chrono::{DateTime, Utc};
use futures::SinkExt;
use futures::channel::mpsc::Sender;
use iced::Alignment::Center;
use iced::widget::{Button, Row, TextInput};
use iced::widget::{button, column, container, row, svg, text, text_input};
use iced::{
    Background, Border, Color, Element, Length, Padding, Shadow, Size, Subscription, Task, Theme,
    Vector,
};
use iced::{border, stream, window};
use log::error;
use log::info;
use rust_embed::RustEmbed;
use serde_json::Value;
use std::future;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

const AUTO_CLOSE_WELCOME_WINDOW_AFTER: chrono::Duration = chrono::Duration::seconds(5);
const AUTO_CLOSE_PROMPT_WINDOW_AFTER: chrono::Duration = chrono::Duration::seconds(10);

#[derive(RustEmbed)]
#[folder = "gallery/"]
struct Gallery;

// ------------------------------------------------------------------
//                              STRUCT
// ------------------------------------------------------------------
#[derive(Default)]
struct Daemon {
    csv_context: Arc<Mutex<csv::Context>>,
    welcome_window: Option<(window::Id, WelcomeWindow)>,
    prompt_window: Option<(window::Id, PromptWindow)>,
    snooze_until: Option<DateTime<Utc>>,
    pending_ctx: Option<Value>,
}

#[derive(Default)]
struct WelcomeWindow {}

#[derive(Default)]
struct PromptWindow {
    ctx: serde_json::Value,
    current_time: Option<DateTime<Utc>>,
    auto_close_window_at: Option<DateTime<Utc>>,
    is_editing: bool,
    user_input: String,
}
// ------------------------------------------------------------------
//                               ENUM
// ------------------------------------------------------------------
#[derive(Debug, Clone)]
enum DaemonMessage {
    // Request to open `WelcomeWindow`
    OpenWelcomeWindow,
    // Request to open `PromptWindow`
    OpenPromptWindow,
    // Notify that `WelcomeWindow` is opened
    WelcomeWindowOpened(window::Id),
    // Notify that `PromptWindow` is opened
    PromptWindowOpened(window::Id),
    // Auto close `WelcomeWindow` (only triggered by `WelcomeWindowOpened`)
    AutoCloseWelcomeWindow(window::Id),
    // Request to close a specific window
    KillWindow(window::Id),
    // A notification that a wallet address is detected, triggered by clipboard monitoring thread
    WalletAddressDetected(AdvisoryLevel, AddressLabel),
    // Daemon will propagating `WelcomeMessage` to `WelcomeWindow`
    Welcome(WelcomeMessage),
    // Daemon will propagating `PromptMessage` to `PromptWindow`
    Prompt(PromptMessage),
}

#[derive(Debug, Clone)]
enum WelcomeMessage {
    DismissBtnClicked,
}

#[derive(Debug, Clone)]
enum PromptMessage {
    Tick(DateTime<Utc>),
    SetMuteUntil(DateTime<Utc>),
    SetContext(serde_json::Value),
    CloseBtnOnClicked,
    EditBtnOnClicked,
    SaveBtnOnClicked,
    InputChanged(String),
}

#[derive(Debug, Clone, Default)]
enum PromptCallback {
    RequestToKillSelf,
    RequestToResizeWindow(Size),
    RequestToSaveContext(serde_json::Value),
    #[default]
    None,
}

// ------------------------------------------------------------------
//                               IMPL
// ------------------------------------------------------------------
impl Daemon {
    /// Dispatches `OpenWelcomeWindow` message after initialization.
    fn new() -> (Self, Task<DaemonMessage>) {
        (
            Self::default(),
            Task::done(DaemonMessage::OpenWelcomeWindow),
        )
    }

    fn title(&self, _: window::Id) -> String {
        "Dome3: Your Personal Web3 Sentinel".to_string()
    }

    fn update(&mut self, message: DaemonMessage) -> Task<DaemonMessage> {
        let default_return = Task::none();

        match message {
            // React to `OpenWelcomeWindow` actions:
            //  1. Open `WelcomeWindow`
            //  2. Emit `WelcomeWindowOpened` and continue next steps
            DaemonMessage::OpenWelcomeWindow => {
                info!("Receive request to open WelcomeWindow");
                if self.welcome_window.is_none() {
                    let (window_id, task) = window::open(default_welcome_window_setting());
                    self.welcome_window = Some((window_id, WelcomeWindow::default()));
                    return task.map(DaemonMessage::WelcomeWindowOpened);
                } else {
                    error!("WelcomeWindow is already opened!!! (bug exist ⚠️)");
                }
            }
            // React to `WelcomeWindowOpened` actions:
            //  1. Waiting for certain seconds (determined by `AUTO_CLOSE_WELCOME_WINDOW_AFTER`)
            //  2. Request to `AutoCloseWelcomeWindow`
            DaemonMessage::WelcomeWindowOpened(window_id) => {
                debug_assert!(self.welcome_window.is_some());
                info!("WelcomeWindow has opened");

                info!(
                    "Waiting for {} seconds to auto close it",
                    AUTO_CLOSE_WELCOME_WINDOW_AFTER.num_seconds()
                );

                return Task::future(async move {
                    tokio::time::sleep(
                        AUTO_CLOSE_WELCOME_WINDOW_AFTER
                            .to_std()
                            .expect("Must be positive Duration"),
                    )
                    .await;

                    DaemonMessage::AutoCloseWelcomeWindow(window_id)
                });
            }
            // React to `AutoCloseWelcomeWindow` actions:
            //  1. Kill `WelcomeWindow`.
            //      If the `WelcomeWindow` is already killed => Do nothing.
            DaemonMessage::AutoCloseWelcomeWindow(window_id) => {
                if self.welcome_window.is_some() {
                    info!("Auto close the WelcomeWindow");
                    return Task::done(DaemonMessage::KillWindow(window_id));
                }
            }
            // React to `OpenPromptWindow` actions:
            //  1. Open `PromptWindow`
            //  2. Notify `PromptWindowOpened` and continue next steps
            DaemonMessage::OpenPromptWindow => {
                info!("Receive request to open PromptWindow");
                debug_assert!(self.prompt_window.is_none());

                if self.prompt_window.is_none() {
                    let (window_id, task) = window::open(default_prompt_window_setting());
                    self.prompt_window = Some((window_id, PromptWindow::default()));
                    return task.map(DaemonMessage::PromptWindowOpened);
                } else {
                    error!("PromptWindow is already opened!!! (bug exist ⚠️)");
                }
            }
            // React to `PromptWindowOpened` actions:
            //  1. Set an auto close window time (determined by `AUTO_CLOSE_PROMPT_WINDOW_AFTER`)
            //  2. Pass the `pending_prompt_ctx` to `PromptWindow` if exists
            DaemonMessage::PromptWindowOpened(_window_id) => {
                debug_assert!(self.prompt_window.is_some());
                info!("PromptWindow has opened");

                self.prompt_window.as_mut().unwrap().1.auto_close_window_at =
                    Some(Utc::now() + AUTO_CLOSE_PROMPT_WINDOW_AFTER);

                if let Some(pending_ctx) = self.pending_ctx.take() {
                    if let Some((_, instance)) = &mut self.prompt_window {
                        info!("Passing pending context to PromptWindow");
                        instance.update(PromptMessage::SetContext(pending_ctx));
                    }
                }
            }
            // React to `KillWindow` actions:
            //  1. Clean the associated state of the `Daemon`
            //  2. Close the window
            DaemonMessage::KillWindow(window_id) => {
                self.clean_state(window_id);
                return window::close(window_id);
            }
            // React to `WalletAddressDetected` actions:
            //  1. Early return if app is snoozing. (e.g. do nothing)
            //  2. Serialize the input args into JSON context.
            //  3. Request to `OpenPromptWindow`
            //  3a. If the `PromptWindow` already exists, UPDATE its context.
            //
            // Note:
            //  All the contexts that are going to be dispatched to `PromptWindow`
            //      should be JSON serializable.
            //  We believe by following this convention, it adds a bit future extensibility.
            //      (e.g. adding new context fields)
            DaemonMessage::WalletAddressDetected(level, label) => {
                info!("WalletAddressDetected({:?}, {:?})", level, label);

                // Check if app is in snoozing mode
                if let Some(snooze_until) = self.snooze_until {
                    if Utc::now() < snooze_until {
                        info!("App is snoozing until {}", snooze_until);
                        return Task::none();
                    }
                }

                // Serialize input args into JSON context
                let ctx = serde_json::to_value((level, label)).unwrap_or(serde_json::Value::Null);

                // If the `PromptWindow` exists, update its context.
                // If the `PromptWindow` doesn't exist, store the `ctx` in `pending_prompt_ctx`,
                //  and request to `OpenPromptWindow` to proceeding `pending_prompt_ctx`.
                if let Some((_, instance)) = &mut self.prompt_window {
                    instance.update(PromptMessage::SetContext(ctx));
                } else {
                    self.pending_ctx = Some(ctx);
                    return Task::done(DaemonMessage::OpenPromptWindow);
                }
            }

            // React to `Welcome` actions:
            //  1. Handle `WelcomeMessage` that need to be propagated to `WelcomeWindow`
            DaemonMessage::Welcome(msg) => {
                if let Some((window_id, _instance)) = &mut self.welcome_window {
                    match msg {
                        // React to `WelcomeMessage::DismissBtnClicked` actions:
                        //  1. Kill the `WelcomeWindow`
                        WelcomeMessage::DismissBtnClicked => {
                            info!("DismissBtnClicked => Kill WelcomeWindow");
                            return Task::done(DaemonMessage::KillWindow(*window_id));
                        }
                    }
                }
            }

            // React to `Prompt` actions:
            //  1. Handle `PromptMessage` that doesn't need to be propagated.
            //  2. Propagate `PromptMessage` to `PromptWindow`.
            //  3. Handle callback action from `PromptWindow`.
            DaemonMessage::Prompt(msg) => {
                if let Some((window_id, instance)) = &mut self.prompt_window {
                    // Handle `PromptMessage` that doesn't need to be propagated
                    match msg {
                        // React to `PromptMessage::SetMuteUntil` actions:
                        //  1. Set the `snooze_until` to activate snoozing mode
                        //  2. Kill the `PromptWindow`
                        PromptMessage::SetMuteUntil(deadline) => {
                            info!("Set snooze until {}", deadline);
                            self.snooze_until = Some(deadline);
                            return Task::done(DaemonMessage::KillWindow(*window_id));
                        }
                        _ => {}
                    }

                    // Propagate `PromptMessage` to `PromptWindow`.
                    let callback = instance.update(msg);

                    // Handle callback action from `PromptWindow`
                    match callback {
                        PromptCallback::RequestToKillSelf => {
                            info!("PromptWindow requested to kill itself (callback)");
                            return Task::done(DaemonMessage::KillWindow(*window_id));
                        }
                        PromptCallback::RequestToResizeWindow(size) => {
                            info!("PromptWindow requested to resize (callback)");
                            return window::resize(*window_id, size);
                        }
                        PromptCallback::RequestToSaveContext(ctx) => {
                            info!("PromptWindow requested to save context (callback)");
                            let ctx_deserialized: Result<(AdvisoryLevel, AddressLabel), _> =
                                serde_json::from_value(ctx);

                            if let Ok((_, address_label)) = ctx_deserialized {
                                let new_entry = AddressLabel {
                                    network: address_label.network,
                                    address: address_label.address,
                                    label: std::mem::take(&mut instance.user_input),
                                };

                                if let Err(e) = self.csv_context.lock().unwrap().append(new_entry) {
                                    error!("Failed to append csv: {e}");
                                } else {
                                    info!("Successfully appended csv");
                                }
                            }
                            return Task::done(DaemonMessage::KillWindow(*window_id));
                        }
                        PromptCallback::None => {
                            return Task::none();
                        }
                    }
                }
            }
        }

        default_return
    }

    fn view(&self, wid: window::Id) -> Element<'_, DaemonMessage> {
        if let Some((id, state)) = &self.welcome_window {
            if *id == wid {
                return state.view().map(DaemonMessage::Welcome);
            }
        }

        if let Some((id, state)) = &self.prompt_window {
            if *id == wid {
                return state.view(wid).map(DaemonMessage::Prompt);
            }
        }

        container(
            text("Unhandled Window (If you're seeing this, it means something went wrong...)")
                .center(),
        )
        .center(Length::Fill)
        .into()
    }

    fn subscribe(&self) -> Subscription<DaemonMessage> {
        struct HashableCsvContext(Arc<Mutex<csv::Context>>); // just a wrapper of `self.csv_context`

        impl Hash for HashableCsvContext {
            fn hash<H: Hasher>(&self, state: &mut H) {
                Arc::as_ptr(&self.0).hash(state);
            }
        }

        let clipboard_monitoring = if self.welcome_window.is_some() {
            Subscription::none() // waiting for `WelcomeWindow` is gone
        } else {
            Subscription::run_with(
                HashableCsvContext(self.csv_context.clone()),
                |hashable_csv_context| {
                    // unwrap to get the origin `self.csv_context`
                    let csv_context = hashable_csv_context.0.clone();

                    stream::channel(100, |mut output: Sender<DaemonMessage>| async move {
                        let (tx, mut rx) = mpsc::unbounded_channel();

                        thread::spawn(move || {
                            crate::clipboard::start_listening(csv_context, tx);
                        });

                        while let Some((level, label)) = rx.recv().await {
                            let _ = output
                                .send(DaemonMessage::WalletAddressDetected(level, label))
                                .await;
                        }

                        future::pending().await
                    })
                },
            )
        };

        let ticker = if self.prompt_window.is_some() {
            iced::time::every(Duration::from_secs_f32(0.01))
                .map(|_| DaemonMessage::Prompt(PromptMessage::Tick(Utc::now())))
        } else {
            Subscription::none()
        };

        Subscription::batch(vec![clipboard_monitoring, ticker])
    }

    /// A helper method to clean up the state of `self`
    /// SHOULD be called when every time a window is closed
    fn clean_state(&mut self, target_window_id: window::Id) {
        if let Some((window_id, _)) = self.welcome_window {
            if target_window_id == window_id {
                self.welcome_window = None;
            }
        }

        if let Some((window_id, _)) = self.prompt_window {
            if target_window_id == window_id {
                self.prompt_window = None;
            }
        }
    }
}

impl WelcomeWindow {
    fn view(&self) -> Element<'_, WelcomeMessage> {
        container(
            column![
                column![
                    text("Dome3: Your Personal Wallet Sentinel")
                        .center()
                        .size(40),
                    text("App is successfully launched as a daemon process 🚀")
                        .center()
                        .size(20),
                    text("I will be activated when you copy a wallet address 😉")
                        .center()
                        .size(20),
                ]
                .spacing(10)
                .align_x(Center),
                button(text("Close Window").center())
                    .padding(12)
                    .style(button::secondary)
                    .on_press(WelcomeMessage::DismissBtnClicked)
            ]
            .spacing(20)
            .align_x(Center),
        )
        .width(10)
        .padding(30)
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color::BLACK)),
            border: border::rounded(16),
            shadow: Shadow {
                color: [0.0, 0.0, 0.0, 0.1].into(),
                offset: Vector::new(0.0, 4.0),
                blur_radius: 20.0,
            },
            ..Default::default()
        })
        .height(Length::Fill)
        .width(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Color::from_str("#F2F5FA").unwrap().into()),
            ..Default::default()
        })
        .into()
    }
}

impl PromptWindow {
    fn update(&mut self, message: PromptMessage) -> PromptCallback {
        match message {
            PromptMessage::Tick(now) => {
                self.current_time = Some(now);

                if let Some(deadline) = self.auto_close_window_at {
                    let ctx_deserialized_result: Result<(AdvisoryLevel, AddressLabel), _> =
                        serde_json::from_value(self.ctx.clone());

                    if let Ok((advisory_level, _)) = ctx_deserialized_result {
                        if advisory_level == AdvisoryLevel::Unknown {
                            return PromptCallback::None;
                        }
                    }

                    if now >= deadline {
                        info!("PromptWindow auto close times out");
                        return PromptCallback::RequestToKillSelf;
                    }
                }
            }
            PromptMessage::SetMuteUntil(_) => { /* state is updated in the parent `Daemon` */ }
            PromptMessage::SetContext(value) => {
                self.current_time = Some(Utc::now());
                self.auto_close_window_at = Some(Utc::now() + AUTO_CLOSE_PROMPT_WINDOW_AFTER);
                self.ctx = value;
            }
            PromptMessage::EditBtnOnClicked => {
                info!("PromptWindow Edit button on clicked");
                self.is_editing = true;
                self.auto_close_window_at = None;
                let new_size = Size::new(760.0, 350.0);
                return PromptCallback::RequestToResizeWindow(new_size);
            }
            PromptMessage::InputChanged(text) => {
                self.user_input = text;
            }
            PromptMessage::SaveBtnOnClicked => {
                info!("PromptWindow Save button on clicked");
                self.is_editing = false;
                self.auto_close_window_at = None;
                return PromptCallback::RequestToSaveContext(self.ctx.clone());
            }
            PromptMessage::CloseBtnOnClicked => {
                info!("PromptWindow Close button on clicked");
                self.is_editing = false;
                self.auto_close_window_at = None;
                return PromptCallback::RequestToKillSelf;
            }
        }
        PromptCallback::None
    }

    fn view(&self, _wid: window::Id) -> Element<'_, PromptMessage> {
        let information: Element<'_, PromptMessage> = if self.ctx.is_null() {
            text("If you see this, something went wrong...")
                .size(20)
                .into()
        } else {
            let ctx_deserialized_result: Result<(AdvisoryLevel, AddressLabel), _> =
                serde_json::from_value(self.ctx.clone());

            match ctx_deserialized_result {
                Ok((advisory_level, address_label)) => {
                    let mut info_column = column![
                        self.wallet_address_row(&address_label),
                        self.wallet_label_row(&address_label),
                        self.wallet_risk_row(&advisory_level),
                    ]
                    .spacing(8);

                    // If editing, insert a text input (for editing custom label)
                    if self.is_editing {
                        info_column = info_column.push(
                            column![self.custom_label_text_input()]
                                .spacing(5)
                                .padding(Padding {
                                    top: 10.0,
                                    ..Default::default()
                                }),
                        );
                    }
                    info_column.into()
                }
                Err(e) => {
                    error!(
                        "Failed to deserialize prompt window context. Expected (AdvisoryLevel, AddressLabel), got {:?}. Error: {}",
                        self.ctx, e
                    );
                    let raw_json_string = format!("{}", self.ctx);
                    text(raw_json_string).size(20).into()
                }
            }
        };

        let title_row = self.notification_title();

        let information_box = container(information)
            .padding(15)
            .width(Length::Fill)
            .style(container::rounded_box);

        let button_row = {
            let close_btn = self.close_button();
            let edit_or_save_btn = self.edit_or_save_button();
            let mute_for_10_mins_btn = self.mute_button(
                "Mute for 10 mins",
                PromptMessage::SetMuteUntil(Utc::now() + chrono::Duration::minutes(10)),
            );

            row![close_btn, edit_or_save_btn, mute_for_10_mins_btn,].spacing(10)
        };

        column![title_row, information_box, button_row]
            .spacing(20)
            .padding(20)
            .into()
    }

    // ------------------------------------------------------------------
    //                            COMPONENTS
    // ------------------------------------------------------------------
    fn notification_title(&self) -> Row<'_, PromptMessage> {
        let bell_icon_file = Gallery::get("RingingBell.svg").unwrap();
        let bell_icon_handle = svg::Handle::from_memory(bell_icon_file.data.into_owned());
        let bell_icon = svg::<Theme>(bell_icon_handle).width(30).height(30);
        let title = text("Crypto Wallet Address Detected")
            .size(30)
            .color(Color::from_str("#CC0000").unwrap());

        row![bell_icon, title].spacing(12).align_y(Center)
    }

    fn wallet_address_row(&self, address_label: &AddressLabel) -> Row<'_, PromptMessage> {
        let network_icon_file = Gallery::get(format!("{:?}.svg", address_label.network).as_str())
            .unwrap_or(Gallery::get("Other.svg").unwrap());
        let network_icon_handle = svg::Handle::from_memory(network_icon_file.data.into_owned());
        let network_icon = svg::<Theme>(network_icon_handle).width(20).height(20);
        let wallet_address = text(address_label.address.clone()).size(20);

        row![network_icon, wallet_address,]
            .spacing(5)
            .align_y(Center)
    }

    fn wallet_label_row(&self, address_label: &AddressLabel) -> Row<'_, PromptMessage> {
        // use `text_input` allowing user select the wallet label
        let wallet_label_text =
            text_input("", &address_label.label)
                .size(20)
                .style(move |theme: &Theme, _status| text_input::Style {
                    background: Background::Color(Color::TRANSPARENT),
                    border: Border {
                        width: 0.0,
                        ..Default::default()
                    },
                    value: Color::from_str("#3399FF").unwrap(),
                    selection: Color::from_str("#CCCFFF").unwrap(),
                    icon: theme.extended_palette().background.weak.text,
                    placeholder: theme.extended_palette().secondary.base.color,
                });

        row![wallet_label_text]
    }

    fn wallet_risk_row(&self, advisory_level: &AdvisoryLevel) -> Row<'_, PromptMessage> {
        let risk_tag_bg_color = match advisory_level {
            AdvisoryLevel::Unknown => Color::from_str("#F0F0F0").unwrap(),
            AdvisoryLevel::Known => Color::from_str("#F0F0F0").unwrap(),
            AdvisoryLevel::Warning => Color::from_str("#FFD700").unwrap(),
            AdvisoryLevel::Risky => Color::from_str("#FFA500").unwrap(),
            AdvisoryLevel::Danger => Color::from_str("#FF4500").unwrap(),
        };

        let risk_level_title = text("Risk Level:").size(15).style(text::secondary);

        let risk_level_tag = container(
            text(format!("{:?}", advisory_level))
                .size(15)
                .color(Color::BLACK),
        )
        .padding([4, 8])
        .style(move |_theme| container::Style {
            background: Some(Background::Color(risk_tag_bg_color)),
            border: border::rounded(4),
            ..Default::default()
        });

        row![risk_level_title, risk_level_tag]
            .spacing(10)
            .align_y(Center)
    }

    fn custom_label_text_input(&self) -> TextInput<'_, PromptMessage> {
        let placeholder_text = "Tom's Binance Deposit Address #1337";

        text_input(placeholder_text, &self.user_input)
            .on_input(PromptMessage::InputChanged)
            .padding(10)
            .size(20)
    }

    // ------------------------------------------------------------------
    //                       COMPONENTS - BUTTONS
    // ------------------------------------------------------------------

    fn close_button(&self) -> Button<'_, PromptMessage> {
        let auto_close_remaining_seconds_text =
            if let (Some(deadline), Some(now)) = (self.auto_close_window_at, self.current_time) {
                let diff = deadline - now;
                if diff.num_milliseconds() <= 0 {
                    0
                } else {
                    diff.num_seconds() + 1 // +1 for making the display text looks like: '3', '2', '1'
                }
            } else {
                AUTO_CLOSE_PROMPT_WINDOW_AFTER.num_seconds()
            };

        let display_text = if self.is_editing {
            "Cancel".to_string()
        } else {
            let ctx_deserialized_result: Result<(AdvisoryLevel, AddressLabel), _> =
                serde_json::from_value(self.ctx.clone());

            if let Ok((advisory_level, _address_label)) = ctx_deserialized_result {
                if advisory_level == AdvisoryLevel::Unknown {
                    "Close".to_string()
                } else {
                    format!("Close({})", auto_close_remaining_seconds_text)
                }
            } else {
                "Close".to_string()
            }
        };

        button(text(display_text).center())
            .width(Length::Fill)
            .on_press(PromptMessage::CloseBtnOnClicked)
            .style(button::danger)
    }

    /// Determine which button to display based on the current state (either "Edit" or "Save")
    fn edit_or_save_button(&self) -> Button<'_, PromptMessage> {
        if self.is_editing {
            let mut btn = button(text("Save").center());

            if self.user_input.trim().is_empty() {
                btn = btn.style(button::secondary)
            } else {
                btn = btn
                    .style(button::primary)
                    .on_press(PromptMessage::SaveBtnOnClicked);
            }
            btn
        } else {
            button(text("Edit").center())
                .style(button::secondary)
                .on_press(PromptMessage::EditBtnOnClicked)
        }
        .width(Length::Fill)
    }

    fn mute_button<'a>(
        &self,
        display_text: &'a str,
        msg: PromptMessage,
    ) -> Button<'a, PromptMessage> {
        button(text(display_text).center())
            .on_press(msg)
            .width(Length::Fill)
    }
}

// ------------------------------------------------------------------
//                         WINDOW SETTINGS
// ------------------------------------------------------------------
fn default_welcome_window_setting() -> window::Settings {
    window::Settings {
        size: (700.0, 240.0).into(),
        position: window::Position::Centered,
        level: window::Level::AlwaysOnTop,
        resizable: true,
        decorations: false,
        transparent: true,
        blur: true,
        exit_on_close_request: false,
        ..Default::default()
    }
}

fn default_prompt_window_setting() -> window::Settings {
    window::Settings {
        size: (760.0, 290.0).into(),
        position: window::Position::Centered,
        level: window::Level::AlwaysOnTop,
        resizable: true,
        decorations: false,
        transparent: true,
        blur: true,
        exit_on_close_request: false,
        ..Default::default()
    }
}

// ------------------------------------------------------------------
//                            ENTRYPOINT
// ------------------------------------------------------------------
fn main() -> iced::Result {
    env_logger::init();

    iced::daemon(Daemon::new, Daemon::update, Daemon::view)
        .subscription(Daemon::subscribe)
        .title(Daemon::title)
        // .style(App::style)
        .run()
}
