use crate::{
    api::AuthError,
    constants::{APP_VERSION, ICON_BYTES},
    host::remove_host_entry,
    patch::{try_patch_game, try_remove_patch},
    try_update_create, try_update_host, try_update_login, LookupData, LookupError,
};
use iced::{
    executor,
    theme::Palette,
    widget::{
        button, column, container, row, text, text_input, Button, Column, Row, Text, TextInput,
    },
    window::{self, icon},
    Application, Color, Command, Length, Settings, Theme,
};

use super::{show_error, show_info};

/// The window size
pub const WINDOW_SIZE: (u32, u32) = (500, 280);

pub fn init(_: tokio::runtime::Runtime) {
    App::run(Settings {
        window: window::Settings {
            icon: icon::from_file_data(ICON_BYTES, None).ok(),
            size: WINDOW_SIZE,
            resizable: false,

            ..window::Settings::default()
        },

        ..Settings::default()
    })
    .unwrap();
}

struct App {
    lookup_state: LookupState,
    auth_state: AuthState,
    target: String,
    username: String,
    password: String,
    state: AppState,
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = remove_host_entry();
    }
}

#[derive(Debug, Clone)]
enum AppState {
    Default,
    Login,
    Create,
    Running,
}

/// Messages used for updating the game state
#[derive(Debug, Clone)]
enum AppMessage {
    /// The redirector target address changed
    TargetChanged(String),

    UsernameChanged(String),
    PasswordChanged(String),

    /// The redirector target should be updated
    UpdateTarget,
    /// Display the patch game dialog asking the player to patch
    PatchGame,
    /// Remove the patch from the game
    RemovePatch,
    /// Message for setting the current lookup result state
    LookupState(LookupState),
    /// Message for setting the current lookup result state
    AuthState(AuthState),

    AttemptLogin,
    AttemptCreate,

    SetState(AppState),

    Disconnect,
}

/// Different states that lookup process can be in
#[derive(Debug, Clone)]
enum LookupState {
    /// Lookup not yet done
    None,
    /// Looking up value
    Loading,
    /// Lookup complete success
    Success(LookupData),
    /// Lookup failed error
    Error(String),
}

#[derive(Debug, Clone)]
enum AuthState {
    None,
    Loading,
    Error(String),
}

impl Application for App {
    type Message = AppMessage;
    type Executor = executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (
            App {
                lookup_state: LookupState::None,
                auth_state: AuthState::None,
                target: String::new(),
                state: AppState::Default,
                username: String::new(),
                password: String::new(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        format!("Pocket Relay Client v{}", APP_VERSION)
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            // Update the stored target
            AppMessage::TargetChanged(value) => self.target = value,
            // Handle new target being set
            AppMessage::UpdateTarget => {
                // Don't try to lookup if already looking up
                if let LookupState::Loading = self.lookup_state {
                    return Command::none();
                }

                self.lookup_state = LookupState::Loading;

                let target = self.target.clone();

                // Handling for once the async lookup is complete
                let post_lookup = |result: Result<LookupData, LookupError>| {
                    let result = match result {
                        Ok(value) => LookupState::Success(value),
                        Err(err) => LookupState::Error(err.to_string()),
                    };
                    AppMessage::LookupState(result)
                };

                // Perform the async lookup with the callback
                return Command::perform(try_update_host(target), post_lookup);
            }
            // Patching
            AppMessage::PatchGame => match try_patch_game() {
                // Game was patched
                Ok(true) => show_info("Game patched", "Sucessfully patched game"),
                // Patching was cancelled
                Ok(false) => {}
                // Error occurred
                Err(err) => show_error("Failed to patch game", &err.to_string()),
            },
            // Patch removal
            AppMessage::RemovePatch => match try_remove_patch() {
                // Patch was removed
                Ok(true) => show_info("Patch removed", "Sucessfully removed patch"),
                // Patch removal cancelled
                Ok(false) => {}
                // Error occurred
                Err(err) => show_error("Failed to remove patch", &err.to_string()),
            },
            // Lookup result changed
            AppMessage::LookupState(value) => {
                if let LookupState::Success(_) = &value {
                    self.state = AppState::Login;
                }
                self.lookup_state = value
            }
            AppMessage::SetState(state) => {
                self.state = state;
            }
            AppMessage::UsernameChanged(username) => self.username = username,
            AppMessage::PasswordChanged(password) => self.password = password,
            AppMessage::AttemptLogin => {
                self.auth_state = AuthState::Loading;
                // Handling for once the async lookup is complete
                let post_login = |result: Result<(), AuthError>| match result {
                    Ok(_) => AppMessage::SetState(AppState::Running),
                    Err(err) => AppMessage::AuthState(AuthState::Error(err.to_string())),
                };

                return Command::perform(
                    try_update_login(self.username.clone(), self.password.clone()),
                    post_login,
                );
            }
            AppMessage::AttemptCreate => {
                self.auth_state = AuthState::Loading;
                // Handling for once the async lookup is complete
                let post_create = |result: Result<(), AuthError>| match result {
                    Ok(_) => AppMessage::SetState(AppState::Running),
                    Err(err) => AppMessage::AuthState(AuthState::Error(err.to_string())),
                };

                return Command::perform(
                    try_update_create(self.username.clone(), self.password.clone()),
                    post_create,
                );
            }
            AppMessage::Disconnect => {
                self.state = AppState::Default;
                self.username.clear();
                self.password.clear();
                self.lookup_state = LookupState::None;
            }
            AppMessage::AuthState(state) => {
                self.auth_state = state;
            }
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        match self.state {
            AppState::Default => self.base_view(),
            AppState::Login => self.login_view(),
            AppState::Create => self.create_view(),
            AppState::Running => self.running_view(),
        }
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }
}

const DARK_TEXT: Color = Color::from_rgb(0.4, 0.4, 0.4);
const RED_TEXT: Color = Color::from_rgb(0.8, 0.4, 0.4);
const YELLOW_TEXT: Color = Color::from_rgb(0.8, 0.8, 0.4);
const ORANGE_TEXT: Color = Color::from_rgb(0.8, 0.6, 0.4);
const SPACING: u16 = 10;

impl App
where
    Self: Application,
{
    fn base_view(&self) -> iced::Element<'_, <Self as Application>::Message> {
        let target_input: TextInput<_> = text_input("Connection URL", &self.target)
            .padding(10)
            .on_input(AppMessage::TargetChanged)
            .on_submit(AppMessage::UpdateTarget);

        let target_text: Text =
            text("Please put the server Connection URL below and press 'Set'").style(DARK_TEXT);
        let target_button: Button<_> = button("Set").on_press(AppMessage::UpdateTarget).padding(10);

        let status_text: Text = match &self.lookup_state {
            LookupState::None => text("Not Connected.").style(ORANGE_TEXT),
            LookupState::Loading => text("Connecting...").style(YELLOW_TEXT),
            LookupState::Success(lookup_data) => text(format!(
                "Connected: {} {} version v{}",
                lookup_data.scheme, lookup_data.host, lookup_data.version
            ))
            .style(Palette::DARK.success),
            LookupState::Error(err) => text(err).style(Palette::DARK.danger),
        };

        let target_row: Row<_> = row![target_input, target_button].spacing(SPACING);

        // Keep running notice
        let notice = text(
            "You must keep this program running while playing. \
            Closing this program will cause you to connect to the official servers instead.",
        )
        .style(RED_TEXT);

        // Game patching buttons
        let patch_button: Button<_> = button("Patch Game")
            .on_press(AppMessage::PatchGame)
            .padding(5);
        let unpatch_button: Button<_> = button("Remove Patch")
            .on_press(AppMessage::RemovePatch)
            .padding(5);

        // Patching notice
        let patch_notice: Text = text(
            "You must patch your game in order to make it compatible with Pocket Relay. \
            This patch can be left applied and wont affect playing on official servers.",
        )
        .style(DARK_TEXT);

        let actions_row: Row<_> = row![patch_button, unpatch_button]
            .spacing(SPACING)
            .width(Length::Fill);

        let content: Column<_> = column![
            target_text,
            target_row,
            status_text,
            notice,
            patch_notice,
            actions_row
        ]
        .spacing(10);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(SPACING)
            .into()
    }

    fn login_view(&self) -> iced::Element<'_, <Self as Application>::Message> {
        let title = text("Login").style(DARK_TEXT);

        let status_text: Text = match &self.auth_state {
            AuthState::None => text("Enter your username and password").style(ORANGE_TEXT),
            AuthState::Loading => text("Authenticating...").style(YELLOW_TEXT),
            AuthState::Error(err) => text(err).style(Palette::DARK.danger),
        };

        let username_input: TextInput<_> = text_input("Username", &self.username)
            .padding(10)
            .on_input(AppMessage::UsernameChanged);
        let password_input: TextInput<_> = text_input("Password", &self.password)
            .padding(10)
            .password()
            .on_input(AppMessage::PasswordChanged);

        let submit_button: Button<_> = button("Login")
            .on_press(AppMessage::AttemptLogin)
            .padding(10)
            .width(Length::Fill);
        let switch_button: Button<_> = button("Don't have an account? Create")
            .on_press(AppMessage::SetState(AppState::Create))
            .padding(10)
            .width(Length::Fill);

        let content: Column<_> = column![
            title,
            status_text,
            username_input,
            password_input,
            submit_button,
            switch_button
        ]
        .spacing(10);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(SPACING)
            .into()
    }

    fn create_view(&self) -> iced::Element<'_, <Self as Application>::Message> {
        let title = text("Create Account").style(DARK_TEXT);

        let status_text: Text = match &self.auth_state {
            AuthState::None => text("Enter your desired username and password").style(ORANGE_TEXT),
            AuthState::Loading => text("Creating...").style(YELLOW_TEXT),
            AuthState::Error(err) => text(err).style(Palette::DARK.danger),
        };

        let username_input: TextInput<_> = text_input("Username", &self.username)
            .padding(10)
            .on_input(AppMessage::UsernameChanged);
        let password_input: TextInput<_> = text_input("Password", &self.password)
            .padding(10)
            .password()
            .on_input(AppMessage::PasswordChanged);

        let submit_button: Button<_> = button("Create")
            .on_press(AppMessage::AttemptCreate)
            .padding(10)
            .width(Length::Fill);
        let switch_button: Button<_> = button("Already have an account? Login")
            .on_press(AppMessage::SetState(AppState::Login))
            .padding(10)
            .width(Length::Fill);

        let content: Column<_> = column![
            title,
            status_text,
            username_input,
            password_input,
            submit_button,
            switch_button
        ]
        .spacing(10);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(SPACING)
            .into()
    }

    fn running_view(&self) -> iced::Element<'_, <Self as Application>::Message> {
        let status_text: Text = match &self.lookup_state {
            LookupState::None => text("Not Connected.").style(ORANGE_TEXT),
            LookupState::Loading => text("Connecting...").style(YELLOW_TEXT),
            LookupState::Success(lookup_data) => text(format!(
                "Connected: {} {} version v{}",
                lookup_data.scheme, lookup_data.host, lookup_data.version
            ))
            .style(Palette::DARK.success),
            LookupState::Error(err) => text(err).style(Palette::DARK.danger),
        };

        let disconnect_button: Button<_> = button("Disconnect")
            .on_press(AppMessage::Disconnect)
            .padding(5)
            .width(Length::Fill);

        let content: Column<_> = column![status_text, disconnect_button].spacing(10);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(SPACING)
            .into()
    }
}
