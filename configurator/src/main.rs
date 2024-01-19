#![windows_subsystem = "windows"]
mod i18n;
use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use druid::widget::{
    Align, Button, Checkbox, CrossAxisAlignment, Flex, FlexParams, Label, RadioGroup, TextBox,
};
use druid::{AppLauncher, Data, Lens, Widget, WidgetExt, WindowDesc};
use njupt_wifi_login_configuration::{
    credential::{Credential, IspType},
    login_config::LoginConfig,
    password::{Password, PasswordScope},
};
use once_cell::sync::Lazy;
use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;
const WINDOW_TITLE: &str = "NJUPT WiFi Login Configurator";
static AUTO_LAUNCH: Lazy<AutoLaunch> = Lazy::new(|| {
    let mut path = env::current_exe().unwrap();
    match path.extension() {
        Some(ext) => {
            let mut file_name = OsString::new();
            file_name.push("njupt_wifi_login.");
            file_name.push(ext);
            path.pop();
            path.push(file_name)
        }
        None => {
            path.pop();
            path.push("njupt_wifi_login")
        }
    }
    AutoLaunchBuilder::new()
        .set_app_name("njupt_wifi_login")
        .set_app_path(path.to_string_lossy().as_ref())
        .set_use_launch_agent(true)
        .build()
        .unwrap()
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
    enabled: bool,
    message: String,
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
        .with_min_size((460.0, 320.0))
        .window_size((460.0, 320.0));

    // create the initial app state
    let mut initial_state = ConfiguratorState::default();

    match read_my_config() {
        Ok(config) => {
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
        Err(_) => {}
    }
    initial_state.enabled = AUTO_LAUNCH.is_enabled().unwrap_or(false);
    initial_state.message = fl!("tips-not-effective-until-rebooting");

    // start the application
    AppLauncher::with_window(main_window)
        .launch(initial_state)
        .expect("Failed to launch application");
}

fn build_root_widget() -> impl Widget<ConfiguratorState> {
    let isp_label = Label::new(fl!("isp")).fix_width(100.0);
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

    let userid_label = Label::new(fl!("user-id")).fix_width(100.0);
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

    let password_label = Label::new(fl!("password")).fix_width(100.0);
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

    let password_scope_label = Label::new(fl!("password-scope")).fix_width(100.0);
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
        .with_child(password_scope_label)
        .with_default_spacer()
        .with_flex_child(
            password_scope_radio_group,
            FlexParams::new(1.0, CrossAxisAlignment::End),
        );

    let enable_checkbox = Checkbox::new(fl!("enable"))
        .lens(ConfiguratorState::enabled)
        .align_left();

    let message_label =
        Label::new(|data: &ConfiguratorState, _env: &_| data.message.clone()).align_left();

    let save_button = Button::new(fl!("save"))
        .on_click(|_ctx, data: &mut ConfiguratorState, _env| {
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
            let auto_launch_result = if data.enabled {
                AUTO_LAUNCH.enable()
            } else {
                AUTO_LAUNCH.disable()
            };
            if let Err(e) = auto_launch_result {
                data.message = fl!("error-failed-to-set-auto-launch", details = e.to_string());
                return;
            }
            data.message = fl!("info-applied-successfully");
        })
        .fix_size(72.0, 36.0)
        .align_right();

    let layout = Flex::column()
        .with_child(isp_flex)
        .with_default_spacer()
        .with_child(userid_flex)
        .with_default_spacer()
        .with_child(password_flex)
        .with_default_spacer()
        .with_child(password_scope_flex)
        .with_default_spacer()
        .with_child(enable_checkbox)
        .with_default_spacer()
        .with_child(message_label)
        .with_default_spacer()
        .with_child(save_button);

    // center the two widgets in the available space
    Align::centered(layout)
}
