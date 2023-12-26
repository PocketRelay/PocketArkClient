use super::{ICON_BYTES, WINDOW_TITLE};
use crate::{
    config::{write_config_file, ClientConfig},
    patch::{try_patch_game, try_remove_patch},
    servers::start_all_servers,
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
use log::debug;
use pocket_ark_client_shared::{
    api::{
        create_user, login_user, lookup_server, AuthToken, CreateUserRequest, LoginUserRequest,
        LookupData, LookupError, ServerAuthError,
    },
    reqwest,
};

/// The window size
pub const WINDOW_SIZE: (u32, u32) = (500, 300);

pub fn init(config: Option<ClientConfig>, client: reqwest::Client) {
    App::run(Settings {
        window: window::Settings {
            icon: icon::from_file_data(ICON_BYTES, None).ok(),
            size: WINDOW_SIZE,
            resizable: false,

            ..window::Settings::default()
        },
        flags: (config, client),

        ..Settings::default()
    })
    .unwrap();
}

struct App {
    /// Result of a connection lookup
    lookup_result: LookupState,
    /// Whether to remember the connection URL
    remember: bool,
    /// The current connection URL
    target: String,
    /// Http client for sending requests
    http_client: reqwest::Client,
    /// Current authentication state
    auth_state: AuthState,
    /// App state
    state: AppState,
}

#[derive(Debug, Default, Clone)]
pub struct LoginState {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Default, Clone)]
pub struct CreateState {
    pub email: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Default, Clone)]
enum AppState {
    /// Default state
    #[default]
    Default,
    /// User is on login page
    Login(LoginState),
    /// User is on create account page
    Create(CreateState),
    /// User is logged in and running
    Running(AuthToken),
}

/// Messages used for updating the game state
#[derive(Debug, Clone)]
enum AppMessage {
    /// The redirector target address changed
    TargetChanged(String),
    /// Username field changed
    UsernameChanged(String),
    /// Email field changed
    EmailChanged(String),
    /// Password field changed
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
    /// Login should be attempted
    AttemptLogin,
    /// Account creation should be attempted
    AttemptCreate,
    /// App state should be changed
    SetState(AppState),
    /// Server should disconnect
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
    Error,
}

#[derive(Debug, Clone)]
enum AuthState {
    None,
    Loading,
    Error,
}

impl Application for App {
    type Message = AppMessage;
    type Executor = executor::Default;
    type Flags = (Option<ClientConfig>, reqwest::Client);
    type Theme = Theme;

    fn new(flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let (config, http_client) = flags;
        let (target, remember) = config
            .map(|value| (value.connection_url, true))
            .unwrap_or_default();
        (
            App {
                lookup_result: LookupState::None,
                auth_state: AuthState::None,
                state: AppState::Default,
                target,
                remember,
                http_client,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        WINDOW_TITLE.to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            // Update the stored target
            AppMessage::TargetChanged(value) => self.target = value,
            // Handle new target being set
            AppMessage::UpdateTarget => {
                // Don't try to lookup if already looking up
                if let LookupState::Loading = self.lookup_result {
                    return Command::none();
                }

                self.lookup_result = LookupState::Loading;

                let target = self.target.clone();

                // Handling for once the async lookup is complete
                let post_lookup = |result: Result<LookupData, LookupError>| {
                    let result = match result {
                        Ok(value) => LookupState::Success(value),
                        Err(err) => {
                            show_error("Failed to connect", &err.to_string());
                            LookupState::Error
                        }
                    };
                    AppMessage::LookupState(result)
                };

                // Perform the async lookup with the callback
                return Command::perform(
                    lookup_server(self.http_client.clone(), target),
                    post_lookup,
                );
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
                    self.state = AppState::Login(LoginState::default());
                }
                self.lookup_result = value
            }
            AppMessage::SetState(state) => {
                if let (AppState::Running(token), LookupState::Success(value)) =
                    (&state, &self.lookup_result)
                {
                    debug!("Starting servers");
                    // Start all the servers
                    start_all_servers(
                        self.http_client.clone(),
                        value.url.clone(),
                        value.association.clone(),
                        token.clone(),
                    );

                    // Save the connection URL
                    if self.remember {
                        let connection_url = value.url.to_string();

                        write_config_file(ClientConfig { connection_url });
                    }
                }

                self.state = state;
            }
            AppMessage::UsernameChanged(username) => {
                if let AppState::Create(state) = &mut self.state {
                    state.username = username
                }
            }
            AppMessage::PasswordChanged(password) => match &mut self.state {
                AppState::Login(state) => state.password = password,
                AppState::Create(state) => state.password = password,
                _ => {}
            },
            AppMessage::EmailChanged(email) => match &mut self.state {
                AppState::Login(state) => state.email = email,
                AppState::Create(state) => state.email = email,
                _ => {}
            },
            AppMessage::AttemptLogin => {
                self.auth_state = AuthState::Loading;
                // Handling for once the async lookup is complete
                let post_login = |result: Result<AuthToken, ServerAuthError>| match result {
                    Ok(token) => AppMessage::SetState(AppState::Running(token)),
                    Err(err) => {
                        show_error("Failed to login", &err.to_string());
                        AppMessage::AuthState(AuthState::Error)
                    }
                };

                let (state, data) = match (&self.state, &self.lookup_result) {
                    (AppState::Login(value), LookupState::Success(data)) => (value, data),
                    _ => return Command::none(),
                };

                return Command::perform(
                    login_user(
                        self.http_client.clone(),
                        data.url.as_ref().clone(),
                        LoginUserRequest {
                            email: state.email.clone(),
                            password: state.password.clone(),
                        },
                    ),
                    post_login,
                );
            }
            AppMessage::AttemptCreate => {
                self.auth_state = AuthState::Loading;

                // Handling for once the async lookup is complete
                let post_login = |result: Result<AuthToken, ServerAuthError>| match result {
                    Ok(token) => AppMessage::SetState(AppState::Running(token)),
                    Err(err) => {
                        show_error("Failed to create account", &err.to_string());
                        AppMessage::AuthState(AuthState::Error)
                    }
                };
                let (state, data) = match (&self.state, &self.lookup_result) {
                    (AppState::Create(value), LookupState::Success(data)) => (value, data),
                    _ => return Command::none(),
                };
                return Command::perform(
                    create_user(
                        self.http_client.clone(),
                        data.url.as_ref().clone(),
                        CreateUserRequest {
                            email: state.email.clone(),
                            username: state.username.clone(),
                            password: state.password.clone(),
                        },
                    ),
                    post_login,
                );
            }
            AppMessage::Disconnect => {
                self.state = AppState::Default;
                self.lookup_result = LookupState::None;
            }
            AppMessage::AuthState(state) => {
                self.auth_state = state;
            }
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        match &self.state {
            AppState::Default => self.base_view(),
            AppState::Login(state) => self.login_view(state),
            AppState::Create(state) => self.create_view(state),
            AppState::Running(_) => self.running_view(),
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
            "You must patch your game in order to make it compatible with Pocket Ark. \
            This patch can be left applied and wont affect playing on official servers.",
        )
        .style(DARK_TEXT);

        let actions_row: Row<_> = row![patch_button, unpatch_button]
            .spacing(SPACING)
            .width(Length::Fill);

        let content: Column<_> =
            column![target_text, target_row, notice, patch_notice, actions_row].spacing(10);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(SPACING)
            .into()
    }

    fn login_view(&self, state: &LoginState) -> iced::Element<'_, <Self as Application>::Message> {
        let title = text("Login").style(DARK_TEXT);

        let status_text: Text = match &self.auth_state {
            AuthState::None => text("Enter your email and password").style(ORANGE_TEXT),
            AuthState::Loading => text("Authenticating...").style(YELLOW_TEXT),
            AuthState::Error => text("Failed to login").style(Palette::DARK.danger),
        };

        let email_input: TextInput<_> = text_input("Email", &state.email)
            .padding(10)
            .on_input(AppMessage::EmailChanged);
        let password_input: TextInput<_> = text_input("Password", &state.password)
            .padding(10)
            .password()
            .on_input(AppMessage::PasswordChanged);

        let submit_button: Button<_> = button("Login")
            .on_press(AppMessage::AttemptLogin)
            .padding(10)
            .width(Length::Fill);
        let switch_button: Button<_> = button("Don't have an account? Create")
            .on_press(AppMessage::SetState(AppState::Create(CreateState {
                email: state.email.clone(),
                username: String::new(),
                password: String::new(),
            })))
            .padding(10)
            .width(Length::Fill);

        let content: Column<_> = column![
            title,
            status_text,
            email_input,
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

    fn create_view(
        &self,
        state: &CreateState,
    ) -> iced::Element<'_, <Self as Application>::Message> {
        let title = text("Create Account").style(DARK_TEXT);

        let status_text: Text = match &self.auth_state {
            AuthState::None => {
                text("Enter your desired email, username and password").style(ORANGE_TEXT)
            }
            AuthState::Loading => text("Creating...").style(YELLOW_TEXT),
            AuthState::Error => text("Failed to create account").style(Palette::DARK.danger),
        };

        let email_input: TextInput<_> = text_input("Email", &state.email)
            .padding(10)
            .on_input(AppMessage::EmailChanged);
        let username_input: TextInput<_> = text_input("Username", &state.username)
            .padding(10)
            .on_input(AppMessage::UsernameChanged);
        let password_input: TextInput<_> = text_input("Password", &state.password)
            .padding(10)
            .password()
            .on_input(AppMessage::PasswordChanged);

        let submit_button: Button<_> = button("Create")
            .on_press(AppMessage::AttemptCreate)
            .padding(10)
            .width(Length::Fill);
        let switch_button: Button<_> = button("Already have an account? Login")
            .on_press(AppMessage::SetState(AppState::Login(LoginState {
                email: state.email.clone(),
                password: String::new(),
            })))
            .padding(10)
            .width(Length::Fill);

        let content: Column<_> = column![
            title,
            status_text,
            email_input,
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
        let status_text: Text = match &self.lookup_result {
            LookupState::None => text("Not Connected.").style(ORANGE_TEXT),
            LookupState::Loading => text("Connecting...").style(YELLOW_TEXT),
            LookupState::Success(lookup_data) => text(format!(
                "Connected: {} {} version v{}",
                lookup_data.url.scheme(),
                lookup_data.url.authority(),
                lookup_data.version
            ))
            .style(Palette::DARK.success),
            LookupState::Error => text("Failed to connect").style(Palette::DARK.danger),
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

/// Shows a info message to the user.
///
/// ## Arguments
/// * `title` - The title for the dialog
/// * `text`  - The text for the dialog
pub fn show_info(title: &str, text: &str) {
    native_dialog::MessageDialog::new()
        .set_title(title)
        .set_text(text)
        .set_type(native_dialog::MessageType::Info)
        .show_alert()
        .unwrap()
}

/// Shows an error message to the user.
///
/// ## Arguments
/// * `title` - The title for the dialog
/// * `text`  - The text for the dialog
pub fn show_error(title: &str, text: &str) {
    native_dialog::MessageDialog::new()
        .set_title(title)
        .set_text(text)
        .set_type(native_dialog::MessageType::Error)
        .show_alert()
        .unwrap()
}

/// Shows an warning message to the user.
///
/// ## Arguments
/// * `title` - The title for the dialog
/// * `text`  - The text for the dialog
pub fn show_warning(title: &str, text: &str) {
    native_dialog::MessageDialog::new()
        .set_title(title)
        .set_text(text)
        .set_type(native_dialog::MessageType::Warning)
        .show_alert()
        .unwrap()
}

/// Shows a confirmation message to the user returning
/// the choice that the user made.
///
/// ## Arguments
/// * `title` - The title for the dialog
/// * `text`  - The text for the dialog
pub fn show_confirm(title: &str, text: &str) -> bool {
    native_dialog::MessageDialog::new()
        .set_title(title)
        .set_text(text)
        .set_type(native_dialog::MessageType::Info)
        .show_confirm()
        .unwrap()
}
