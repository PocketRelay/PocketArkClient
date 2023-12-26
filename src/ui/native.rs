use super::{show_error, show_info, ICON_BYTES, WINDOW_TITLE};
use crate::{
    config::{write_config_file, ClientConfig},
    core::{
        api::{lookup_server, LookupData},
        reqwest::Client,
        servers::stop_server_tasks,
    },
    patch::{try_patch_game, try_remove_patch},
    servers::start_all_servers,
    update,
};
use native_windows_derive::{NwgPartial, NwgUi};
use native_windows_gui::{init as nwg_init, *};
use parking_lot::Mutex;
use pocket_ark_client_shared::{
    api::{create_user, login_user, AuthToken, CreateUserRequest, LoginUserRequest},
    Url,
};
use std::{cell::RefCell, sync::Arc};

/// Size of the created window
pub const WINDOW_SIZE: (i32, i32) = (500, 400);

/// Partial UI for the connect screen
#[derive(NwgPartial, Default)]
pub struct ConnectPartial {
    /// Grid layout for all the content
    #[nwg_layout]
    grid: GridLayout,

    /// Label for the connection URL input
    #[nwg_control(text: "Please put the server Connection URL below and press 'Set'")]
    #[nwg_layout_item(layout: grid, row: 0, col_span: 2)]
    target_url_label: Label,

    /// Input for the connection URL
    #[nwg_control(focus: true)]
    #[nwg_layout_item(layout: grid, row: 1, col_span: 2)]
    target_url_input: TextInput,

    /// Button for connecting
    #[nwg_control(text: "Connect")]
    #[nwg_layout_item(layout: grid,  row: 2, col_span: 2)]
    connect_button: Button,

    /// Checkbox for whether to remember the connection URL
    #[nwg_control(text: "Save connection URL")]
    #[nwg_layout_item(layout: grid,row: 3, col_span: 2)]
    remember_checkbox: CheckBox,

    /// Label for the state
    #[nwg_control(text: "Disconnected")]
    #[nwg_layout_item(layout: grid, row: 4, col_span: 2)]
    state_label: Label,

    /// Label for patching the game
    #[nwg_control(text:
        "You must patch your game in order to make it compatible with\n\
        Pocket Ark.",
    )]
    #[nwg_layout_item(layout: grid, row: 5, col_span: 2)]
    patch_label: Label,

    /// Button for connecting
    #[nwg_control(text: "Patch")]
    #[nwg_layout_item(layout: grid, col: 0, row: 6, col_span: 1)]
    patch_button: Button,

    /// Button for connecting
    #[nwg_control(text: "Remove Patch")]
    #[nwg_layout_item(layout: grid, col: 1, row: 6, col_span: 1)]
    remove_patch_button: Button,
}

/// Partial UI for the login screen
#[derive(NwgPartial, Default)]
pub struct LoginPartial {
    /// Grid layout for all the content
    #[nwg_layout]
    grid: GridLayout,

    /// Label for the email
    #[nwg_control(text: "Email")]
    #[nwg_layout_item(layout: grid, row: 0)]
    email_label: Label,

    /// Input for the email
    #[nwg_control(focus: true)]
    #[nwg_layout_item(layout: grid, row: 1)]
    email_input: TextInput,

    /// Label for the password
    #[nwg_control(text: "Password")]
    #[nwg_layout_item(layout: grid, row: 2)]
    password_label: Label,

    /// Input for the password
    #[nwg_control(password: Some('*'))]
    #[nwg_layout_item(layout: grid, row: 3)]
    password_input: TextInput,

    /// Button for logging in
    #[nwg_control(text: "Login")]
    #[nwg_layout_item(layout: grid, row: 4)]
    login_button: Button,

    /// Label for the state
    #[nwg_control(text: "Not authenticated")]
    #[nwg_layout_item(layout: grid, row: 5)]
    state_label: Label,

    /// Button for logging instead
    #[nwg_control(text: "Don't have an account? Create")]
    #[nwg_layout_item(layout: grid, row: 6)]
    swap_button: Button,

    /// Button for disconnecting
    #[nwg_control(text: "Disconnect")]
    #[nwg_layout_item(layout: grid, row: 7)]
    disconnect_button: Button,
}

/// Partial UI for the create account screen
#[derive(NwgPartial, Default)]
pub struct CreatePartial {
    /// Grid layout for all the content
    #[nwg_layout]
    grid: GridLayout,

    /// Label for the email
    #[nwg_control(text: "Email")]
    #[nwg_layout_item(layout: grid, row: 0)]
    email_label: Label,

    /// Input for the email
    #[nwg_control(focus: true)]
    #[nwg_layout_item(layout: grid, row: 1)]
    email_input: TextInput,

    /// Label for the username
    #[nwg_control(text: "Username")]
    #[nwg_layout_item(layout: grid, row: 2)]
    username_label: Label,

    /// Input for the username
    #[nwg_control(limit: 16)]
    #[nwg_layout_item(layout: grid, row: 3)]
    username_input: TextInput,

    /// Label for the password
    #[nwg_control(text: "Password")]
    #[nwg_layout_item(layout: grid, row: 4)]
    password_label: Label,

    /// Input for the password
    #[nwg_control(limit: 99, password: Some('*'))]
    #[nwg_layout_item(layout: grid, row: 5)]
    password_input: TextInput,

    /// Button for logging in
    #[nwg_control(text: "Create")]
    #[nwg_layout_item(layout: grid, row: 6)]
    create_button: Button,

    /// Label for the state
    #[nwg_control(text: "Not authenticated")]
    #[nwg_layout_item(layout: grid, row: 7)]
    state_label: Label,

    /// Button for logging instead
    #[nwg_control(text: "Already have an account? Login")]
    #[nwg_layout_item(layout: grid, row: 8)]
    swap_button: Button,

    /// Button for disconnecting
    #[nwg_control(text: "Disconnect")]
    #[nwg_layout_item(layout: grid, row: 9)]
    disconnect_button: Button,
}

/// Partial UI for the running state
#[derive(NwgPartial, Default)]
pub struct RunningPartial {
    /// Grid layout for all the content
    #[nwg_layout]
    grid: GridLayout,

    /// Connection state label
    #[nwg_control(text: "Connected")]
    #[nwg_layout_item(layout: grid, row: 0)]
    state_label: Label,

    /// Label for keeping the program running
    #[nwg_control(text: "You must keep this program running while playing.")]
    #[nwg_layout_item(layout: grid, row: 1)]
    keep_alive_label: Label,

    /// Button for disconnecting
    #[nwg_control(text: "Disconnect")]
    #[nwg_layout_item(layout: grid, row: 2)]
    disconnect_button: Button,
}

/// Native GUI app
#[derive(NwgUi, Default)]
pub struct App {
    /// Window Icon
    #[nwg_resource(source_bin: Some(ICON_BYTES))]
    icon: Icon,

    /// App window
    #[nwg_control(
        size: WINDOW_SIZE,
        position: (5, 5),
        icon: Some(&data.icon),
        title: WINDOW_TITLE,
        flags: "WINDOW|VISIBLE|MINIMIZE_BOX"
    )]
    #[nwg_events(OnWindowClose: [stop_thread_dispatch()])]
    window: Window,

    /// Grid layout for all the content
    #[nwg_layout(parent: window)]
    grid: GridLayout,

    /// Frame for the connect UI
    #[nwg_control]
    #[nwg_layout_item(layout: grid)]
    connect_frame: Frame,

    /// Connection UI
    #[nwg_partial(parent: connect_frame)]
    #[nwg_events(
        (connect_button, OnButtonClick): [App::handle_connect],
        (patch_button, OnButtonClick): [App::handle_patch],
        (remove_patch_button, OnButtonClick): [App::handle_remove_patch],
    )]
    connect_ui: ConnectPartial,

    /// Frame for the login UI
    #[nwg_control]
    #[nwg_layout_item(layout: grid)]
    login_frame: Frame,

    /// Login UI
    #[nwg_partial(parent: login_frame)]
    #[nwg_events(
        (login_button, OnButtonClick): [App::handle_login],
        (swap_button, OnButtonClick): [App::swap_auth_state],
        (disconnect_button, OnButtonClick): [App::handle_disconnect],
    )]
    login_ui: LoginPartial,

    /// Frame for the create UI
    #[nwg_control]
    #[nwg_layout_item(layout: grid)]
    create_frame: Frame,

    /// Create UI
    #[nwg_partial(parent: create_frame)]
    #[nwg_events(
        (create_button, OnButtonClick): [App::handle_create],
        (swap_button, OnButtonClick): [App::swap_auth_state],
        (disconnect_button, OnButtonClick): [App::handle_disconnect],
    )]
    create_ui: CreatePartial,

    /// Frame for the running UI
    #[nwg_control]
    #[nwg_layout_item(layout: grid)]
    running_frame: Frame,

    /// Running UI
    #[nwg_partial(parent: running_frame)]
    #[nwg_events((disconnect_button, OnButtonClick): [App::handle_disconnect])]
    running_ui: RunningPartial,

    /// Current state of the app
    app_state: RefCell<AppState>,

    /// Shared reference for an optional next state decided by
    /// some other thread or callback
    next_state: Arc<Mutex<Option<NextState>>>,

    /// Notice for when [App::next_state] is changed
    #[nwg_control]
    #[nwg_events(OnNotice: [App::handle_next_state])]
    next_state_notice: Notice,

    /// Http client for sending requests
    http_client: Client,
}

enum NextState {
    /// Don't change the state just update the screen
    /// state label to show an error occured
    Error,
    /// App State to show
    State(AppState),
}

#[derive(Default)]
enum AppState {
    /// Connecting state
    #[default]
    Connect,
    /// Logging in state
    Login {
        /// Current lookup data
        lookup_data: LookupData,
    },
    /// Creating account state
    Create {
        /// Current lookup data
        lookup_data: LookupData,
    },
    /// Running state
    Running {
        /// Current lookup data
        lookup_data: LookupData,
        /// Current authentication token
        auth_token: AuthToken,
    },
}

impl App {
    fn handle_patch(&self) {
        match try_patch_game() {
            // Game was patched
            Ok(true) => show_info("Game patched", "Sucessfully patched game"),
            // Patching was cancelled
            Ok(false) => {}
            // Error occurred
            Err(err) => show_error("Failed to patch game", &err.to_string()),
        }
    }

    fn handle_remove_patch(&self) {
        match try_remove_patch() {
            // Patch was removed
            Ok(true) => show_info("Patch removed", "Sucessfully removed patch"),
            // Patch removal cancelled
            Ok(false) => {}
            // Error occurred
            Err(err) => show_error("Failed to remove patch", &err.to_string()),
        }
    }

    /// Handles changing to a new state provided by an
    /// external thread
    fn handle_next_state(&self) {
        let Some(next_state) = self.next_state.lock().take() else {
            return;
        };

        match next_state {
            // Set error labels
            NextState::Error => match &*self.app_state.borrow() {
                AppState::Connect => {
                    self.connect_ui.state_label.set_text("Failed to connect");
                }
                AppState::Login { .. } => {
                    self.login_ui.state_label.set_text("Failed to login");
                }
                AppState::Create { .. } => {
                    self.create_ui.state_label.set_text("Failed to create");
                }
                _ => {}
            },

            // Handle changing state
            NextState::State(next_state) => {
                // Handle setting up the next state
                match &next_state {
                    AppState::Connect => {}
                    AppState::Login { .. } => {}
                    AppState::Create { .. } => {}
                    AppState::Running {
                        lookup_data,
                        auth_token,
                    } => {
                        // TODO: Update connection state

                        // Start all the servers
                        start_all_servers(
                            self.http_client.clone(),
                            lookup_data.url.clone(),
                            Arc::new(None),
                            auth_token.clone(),
                        );
                    }
                }

                self.set_app_state(next_state);
            }
        }
    }

    /// Swaps the current authentication state to the opposite
    /// (i.e Login -> Create, Create -> Login)
    fn swap_auth_state(&self) {
        let next_state = match &*self.app_state.borrow() {
            AppState::Login { lookup_data } => AppState::Create {
                lookup_data: lookup_data.clone(),
            },
            AppState::Create { lookup_data } => AppState::Login {
                lookup_data: lookup_data.clone(),
            },
            // Do nothing for other states
            _ => return,
        };
        self.set_app_state(next_state)
    }

    /// Sets the app state to the provided `state` then
    /// triggers a UI update
    fn set_app_state(&self, state: AppState) {
        // Swap the state so the old state can be accessed
        let mut old_state = state;
        std::mem::swap(&mut *self.app_state.borrow_mut(), &mut old_state);

        // Stop the server tasks if we were running
        if let AppState::Running { .. } = old_state {
            stop_server_tasks();
        }

        // Update the current UI
        self.update_visible_frame();
    }

    /// Collection of available frames
    fn all_frames(&self) -> [&Frame; 4] {
        [
            &self.connect_frame,
            &self.login_frame,
            &self.create_frame,
            &self.running_frame,
        ]
    }

    /// Sets the current visible frame to `frame` hiding
    /// and disabling all the other frames.
    fn set_visible_frame(&self, frame: &Frame) {
        // Access all frames
        self.all_frames()
            .into_iter()
            // Hide all the frames
            .for_each(|frame| {
                frame.set_visible(false);
                frame.set_enabled(false);
                self.grid.remove_child(frame);
            });

        // Show the specific frame
        frame.set_visible(true);
        frame.set_enabled(true);
        self.grid.add_child(0, 0, frame);
    }

    /// Updates the currently visible frame to match the UI
    /// and resizes the window to fit accordingly
    fn update_visible_frame(&self) {
        match &*self.app_state.borrow() {
            AppState::Connect => {
                self.set_visible_frame(&self.connect_frame);
                self.window.set_size(500, 340);

                self.connect_ui.state_label.set_text("Not connected");
            }
            AppState::Login { .. } => {
                self.set_visible_frame(&self.login_frame);
                self.window.set_size(500, 320);

                self.login_ui.state_label.set_text("Not authenticated");
            }
            AppState::Create { .. } => {
                self.set_visible_frame(&self.create_frame);
                self.window.set_size(500, 400);

                self.create_ui.state_label.set_text("Not authenticated");
            }
            AppState::Running { lookup_data, .. } => {
                self.set_visible_frame(&self.running_frame);
                self.window.set_size(500, 160);

                let text = format!(
                    "Connected: {} {} version v{}",
                    lookup_data.url.scheme(),
                    lookup_data.url.authority(),
                    lookup_data.version
                );
                self.running_ui.state_label.set_text(&text)
            }
        }
    }

    /// Handles the "Set" button being pressed, dispatches a connect task
    /// that will wake up the App with `App::handle_connect_notice` to
    /// handle the connection result.
    fn handle_connect(&self) {
        self.connect_ui.state_label.set_text("Connecting...");

        let target = self.connect_ui.target_url_input.text();

        let http_client = self.http_client.clone();
        let sender = self.next_state_notice.sender();
        let next_state = self.next_state.clone();

        let remember = self.connect_ui.remember_checkbox.check_state() == CheckBoxState::Checked;

        // Save the connection URL
        if remember {
            let connection_url = target.to_string();
            write_config_file(ClientConfig { connection_url });
        }

        tokio::spawn(async move {
            let state = match lookup_server(http_client, target).await {
                Ok(lookup_data) => NextState::State(AppState::Login { lookup_data }),
                Err(err) => {
                    show_error("Failed to lookup server", &err.to_string());
                    NextState::Error
                }
            };

            let next_state = &mut *next_state.lock();
            *next_state = Some(state);
            sender.notice();
        });
    }

    fn handle_login(&self) {
        let AppState::Login { lookup_data } = &*self.app_state.borrow() else {
            return;
        };

        self.login_ui.state_label.set_text("Authenticating...");

        let email = self.login_ui.email_input.text();
        let password = self.login_ui.password_input.text();

        let request = LoginUserRequest { email, password };

        let http_client = self.http_client.clone();
        let lookup_data = lookup_data.clone();

        let sender = self.next_state_notice.sender();
        let next_state = self.next_state.clone();

        tokio::spawn(async move {
            let base_url: Url = lookup_data.url.as_ref().clone();

            let state = match login_user(http_client, base_url, request).await {
                Ok(auth_token) => NextState::State(AppState::Running {
                    lookup_data,
                    auth_token,
                }),
                Err(err) => {
                    show_error("Failed to login", &err.to_string());
                    NextState::Error
                }
            };

            let next_state = &mut *next_state.lock();
            *next_state = Some(state);
            sender.notice();
        });
    }

    fn handle_create(&self) {
        let AppState::Create { lookup_data } = &*self.app_state.borrow() else {
            return;
        };

        self.create_ui.state_label.set_text("Creating account...");

        let email = self.create_ui.email_input.text();
        let username = self.create_ui.username_input.text();
        let password = self.create_ui.password_input.text();

        let request = CreateUserRequest {
            email,
            username,
            password,
        };

        let http_client = self.http_client.clone();
        let lookup_data = lookup_data.clone();

        let sender = self.next_state_notice.sender();
        let next_state = self.next_state.clone();

        tokio::spawn(async move {
            let base_url: Url = lookup_data.url.as_ref().clone();

            let state = match create_user(http_client, base_url, request).await {
                Ok(auth_token) => NextState::State(AppState::Running {
                    lookup_data,
                    auth_token,
                }),
                Err(err) => {
                    show_error("Failed to create account", &err.to_string());
                    NextState::Error
                }
            };

            let next_state = &mut *next_state.lock();
            *next_state = Some(state);
            sender.notice();
        });
    }

    fn handle_disconnect(&self) {
        self.set_app_state(AppState::Connect);
    }
}

/// Initializes the user interface
///
/// ## Arguments
/// * `config` - The client config to use
/// * `client` - The HTTP client to use
pub fn init(config: Option<ClientConfig>, client: Client) {
    // Create tokio async runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building tokio runtime");

    // Enter the tokio runtime
    let _enter = runtime.enter();

    // Spawn the updating task
    tokio::spawn(update::update(client.clone()));

    // Initialize nwg
    nwg_init().expect("Failed to initialize native UI");

    // Set the default font family
    Font::set_global_family("Segoe UI").expect("Failed to set default font");

    // Build the app UI
    let app = App::build_ui(App {
        http_client: client,
        ..Default::default()
    })
    .expect("Failed to build native UI");

    let (target, remember) = config
        .map(|value| (value.connection_url, true))
        .unwrap_or_default();

    app.connect_ui.target_url_input.set_text(&target);

    if remember {
        app.connect_ui
            .remember_checkbox
            .set_check_state(CheckBoxState::Checked);
    }

    app.set_app_state(AppState::Connect);

    dispatch_thread_events();
}
