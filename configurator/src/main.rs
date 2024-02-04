#![windows_subsystem = "windows"]
mod i18n;
mod launcher;
use druid::widget::{
    Align, Button, Checkbox, CrossAxisAlignment, Flex, FlexParams, Label, LineBreaking, RadioGroup,
    TextBox,
};
use druid::{AppLauncher, Data, Lens, Widget, WidgetExt, WindowDesc};
use launcher::Launcher;
use njupt_wifi_login_configuration::{
    credential::{Credential, IspType},
    login_config::LoginConfig,
    password::{Password, PasswordScope},
};
use once_cell::sync::Lazy;
use std::env;
use std::error::Error;
use std::path::PathBuf;
const WINDOW_TITLE: &str = "NJUPT WiFi Login Configurator";
static LAUNCHERS: Lazy<Vec<Box<dyn Launcher + Send + Sync>>> = Lazy::new(|| {
    let mut launchers: Vec<Box<dyn Launcher + Send + Sync>> = Vec::new();
    if let Ok(launcher) = launcher::DesktopLauncher::new() {
        launchers.push(Box::new(launcher));
    }
    launchers
});
static CONFIG_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut path = env::current_exe().unwrap();
    path.pop();
    path.push("njupt_wifi.yml");
    path
});

#[derive(PartialEq, Eq, Debug, Clone, Copy, Data, Default)]
pub enum IspTypeState {
    #[default]
    EDU,
    CMCC,
    CT,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy, Data, Default)]
pub enum PasswordScopeState {
    Anywhere,
    LocalMachine,
    #[default]
    CurrentUser,
}

#[derive(Clone, Data, Lens, Default)]
struct ConfiguratorState {
    userid: String,
    password: String,
    isp: IspTypeState,
    password_scope: PasswordScopeState,
    launcher_index: usize,
    enabled: bool,
    message: String,
    running: bool,
}

fn read_my_config() -> Result<LoginConfig, Box<dyn Error>> {
    let f = std::fs::File::open(CONFIG_PATH.as_path())?;
    let config: LoginConfig = serde_yaml::from_reader(f)?;
    Ok(config)
}

fn write_my_config(d: &LoginConfig) -> Result<(), Box<dyn Error>> {
    let f = std::fs::File::create(CONFIG_PATH.as_path())?;
    serde_yaml::to_writer(f, d)?;
    Ok(())
}

fn main() {
    // describe the main window
    let main_window = WindowDesc::new(build_root_widget())
        .title(WINDOW_TITLE)
        .with_min_size((500.0, 400.0))
        .window_size((500.0, 400.0));

    // create the initial app state
    let mut initial_state = ConfiguratorState::default();

    if let Ok(config) = read_my_config() {
        let isp_state = match config.credential.isp() {
            IspType::EDU => IspTypeState::EDU,
            IspType::CMCC => IspTypeState::CMCC,
            IspType::CT => IspTypeState::CT,
        };
        initial_state.isp = isp_state;
        initial_state.userid = config.credential.userid().to_string();
        initial_state.password = config.credential.password().get().to_string();
        if let Password::Basic(_) = config.credential.password() {
            initial_state.password_scope = PasswordScopeState::Anywhere;
        }
    }
    initial_state.enabled = false;
    initial_state.running = false;
    initial_state.launcher_index = 0;
    for (index, launcher) in LAUNCHERS.iter().enumerate() {
        if launcher.is_enabled().unwrap_or(false) {
            initial_state.enabled = true;
            initial_state.running = launcher.is_running().unwrap_or(false);
            initial_state.launcher_index = index;
            if initial_state.password_scope == PasswordScopeState::CurrentUser
                && !launcher.is_password_scope_supported(&PasswordScope::CurrentUser)
            {
                if launcher.is_password_scope_supported(&PasswordScope::LocalMachine) {
                    initial_state.password_scope = PasswordScopeState::LocalMachine;
                } else {
                    initial_state.password_scope = PasswordScopeState::Anywhere;
                }
            }
            break;
        }
    }
    initial_state.message = fl!("tips-not-effective-until-rebooting");

    // start the application
    AppLauncher::with_window(main_window)
        .launch(initial_state)
        .expect("Failed to launch application");
}

fn build_root_widget() -> impl Widget<ConfiguratorState> {
    let isp_label = Label::new(fl!("isp")).fix_width(140.0);
    let isp_radio_group = RadioGroup::row(vec![
        (fl!("isp-edu"), IspTypeState::EDU),
        (fl!("isp-cmcc"), IspTypeState::CMCC),
        (fl!("isp-ct"), IspTypeState::CT),
    ])
    .lens(ConfiguratorState::isp)
    .expand_width();
    let isp_flex = Flex::row()
        .with_child(isp_label)
        .with_default_spacer()
        .with_flex_child(
            isp_radio_group,
            FlexParams::new(1.0, CrossAxisAlignment::End),
        );

    let userid_label = Label::new(fl!("user-id")).fix_width(140.0);
    let userid_text_box = TextBox::new()
        .expand_width()
        .lens(ConfiguratorState::userid);
    let userid_flex = Flex::row()
        .with_child(userid_label)
        .with_default_spacer()
        .with_flex_child(
            userid_text_box,
            FlexParams::new(1.0, CrossAxisAlignment::End),
        );

    let password_label = Label::new(fl!("password")).fix_width(140.0);
    let password_text_box = TextBox::new()
        .expand_width()
        .lens(ConfiguratorState::password);
    let password_flex = Flex::row()
        .with_child(password_label)
        .with_default_spacer()
        .with_flex_child(
            password_text_box,
            FlexParams::new(1.0, CrossAxisAlignment::End),
        );

    let password_scope_label = Label::new(fl!("password-scope"));
    let password_scope_tips_button =
        Button::new("?").on_click(|_ctx, data: &mut ConfiguratorState, _env| {
            data.message = fl!("tips-password-scope");
        });
    let password_scope_radio_group = RadioGroup::row(vec![
        (fl!("password-scope-anywhere"), PasswordScopeState::Anywhere),
        (
            fl!("password-scope-local-machine"),
            PasswordScopeState::LocalMachine,
        ),
        (
            fl!("password-scope-current-user"),
            PasswordScopeState::CurrentUser,
        ),
    ])
    .lens(ConfiguratorState::password_scope)
    .expand_width();
    let password_scope_flex = Flex::row()
        .with_child(
            Flex::row()
                .with_child(password_scope_label)
                .with_default_spacer()
                .with_child(password_scope_tips_button)
                .align_left()
                .fix_width(140.0),
        )
        .with_default_spacer()
        .with_flex_child(
            password_scope_radio_group,
            FlexParams::new(1.0, CrossAxisAlignment::End),
        );

    let launcher_label = Label::new(fl!("launcher")).fix_width(140.0);
    let launcher_radio_group = RadioGroup::row(
        LAUNCHERS
            .iter()
            .enumerate()
            .map(|(index, launcher)| (launcher.name(), index))
            .collect::<Vec<_>>(),
    )
    .lens(ConfiguratorState::launcher_index)
    .expand_width();
    let launcher_flex = Flex::row()
        .with_child(launcher_label)
        .with_default_spacer()
        .with_flex_child(
            launcher_radio_group,
            FlexParams::new(1.0, CrossAxisAlignment::End),
        );

    let enable_checkbox = Checkbox::new(fl!("enable"))
        .lens(ConfiguratorState::enabled)
        .align_left();

    let message_label = Label::new(|data: &ConfiguratorState, _env: &_| data.message.clone())
        .with_line_break_mode(LineBreaking::WordWrap)
        .align_left()
        .scroll()
        .vertical()
        .fix_height(100.0);

    let status_label = Label::new(|data: &ConfiguratorState, _env: &_| {
        if data.running {
            fl!("status-running")
        } else {
            fl!("status-stopped")
        }
    });

    let start_button = Button::new(fl!("start"))
        .on_click(|_ctx, data: &mut ConfiguratorState, _env| {
            let current_launcher = &LAUNCHERS[data.launcher_index];
            if current_launcher.is_running().unwrap_or(data.running) {
                data.running = true;
                data.message = fl!("error-already-running");
                return;
            }
            if let Err(e) = current_launcher.start() {
                data.message = fl!("error-failed-to-start", details = e.to_string());
                return;
            }
            data.running = true;
            data.message = fl!("info-started-successfully");
        })
        .fix_size(72.0, 36.0);

    let stop_button = Button::new(fl!("stop"))
        .on_click(|_ctx, data: &mut ConfiguratorState, _env| {
            let current_launcher = &LAUNCHERS[data.launcher_index];
            if !current_launcher.is_running().unwrap_or(data.running) {
                data.running = false;
                data.message = fl!("error-not-running");
                return;
            }
            if let Err(e) = current_launcher.stop() {
                data.message = fl!("error-failed-to-stop", details = e.to_string());
                return;
            }
            data.running = false;
            data.message = fl!("info-stopped-successfully");
        })
        .fix_size(72.0, 36.0);

    let save_button = Button::new(fl!("save"))
        .on_click(|_ctx, data: &mut ConfiguratorState, _env| {
            let current_launcher = &LAUNCHERS[data.launcher_index];
            let isp = match data.isp {
                IspTypeState::EDU => IspType::EDU,
                IspTypeState::CMCC => IspType::CMCC,
                IspTypeState::CT => IspType::CT,
            };
            let password_scope = match data.password_scope {
                PasswordScopeState::Anywhere => PasswordScope::Anywhere,
                PasswordScopeState::LocalMachine => PasswordScope::LocalMachine,
                PasswordScopeState::CurrentUser => PasswordScope::CurrentUser,
            };
            if !current_launcher.is_password_scope_supported(&password_scope) {
                data.message = fl!(
                    "error-selected-password-scope-not-supported-by-launcher",
                    launcher = current_launcher.name()
                );
                return;
            }
            let password = Password::try_new(data.password.clone(), password_scope);
            if let Err(e) = password {
                data.message = fl!("error-failed-to-encrypt-password", details = e.to_string());
                return;
            }
            let password = password.unwrap();
            let config = LoginConfig {
                credential: Credential::new(data.userid.clone(), password, isp),
            };
            if let Err(e) = write_my_config(&config) {
                data.message = fl!("error-failed-to-write-config", details = e.to_string());
                return;
            }
            for (index, launcher) in LAUNCHERS.iter().enumerate() {
                if index == data.launcher_index {
                    continue;
                }
                if launcher.is_enabled().unwrap_or(false) {
                    if let Err(e) = launcher.disable() {
                        data.message = fl!(
                            "error-failed-to-disable-other-launchers",
                            launcher = launcher.name(),
                            details = e.to_string()
                        );
                        return;
                    }
                }
            }
            let auto_launch_result = if data.enabled {
                current_launcher.enable()
            } else if current_launcher.is_enabled().unwrap_or(true) {
                current_launcher.disable()
            } else {
                Ok(())
            };
            if let Err(e) = auto_launch_result {
                data.message = fl!("error-failed-to-set-auto-launch", details = e.to_string());
                return;
            }
            data.message = fl!("info-applied-successfully");
        })
        .fix_size(72.0, 36.0);

    let control_area_flex = Flex::row()
        .with_child(status_label)
        .with_flex_spacer(1.0)
        .with_child(start_button)
        .with_default_spacer()
        .with_child(stop_button)
        .with_default_spacer()
        .with_child(save_button);

    let layout = Flex::column()
        .with_child(isp_flex)
        .with_default_spacer()
        .with_child(userid_flex)
        .with_default_spacer()
        .with_child(password_flex)
        .with_default_spacer()
        .with_child(password_scope_flex)
        .with_default_spacer()
        .with_child(launcher_flex)
        .with_default_spacer()
        .with_child(enable_checkbox)
        .with_default_spacer()
        .with_child(message_label)
        .with_default_spacer()
        .with_child(control_area_flex);

    // center the widgets in the available space
    Align::centered(layout)
}
