#![windows_subsystem = "windows"]

use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use druid::widget::{
    Align, Button, Checkbox, CrossAxisAlignment, Flex, FlexParams, Label, RadioGroup, TextBox,
};
use druid::{AppLauncher, Data, Lens, Widget, WidgetExt, WindowDesc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
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

#[derive(Serialize, Deserialize, Debug)]
pub struct MyConfig {
    isp: IspType,
    userid: String,
    password: String,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy, Serialize, Deserialize, Data, Default)]
pub enum IspType {
    #[default]
    EDU,
    CMCC,
    CT,
}

#[derive(Clone, Data, Lens, Default)]
struct ConfiguratorState {
    userid: String,
    password: String,
    isp: IspType,
    enabled: bool,
}

fn read_my_config() -> Result<MyConfig, Box<dyn Error>> {
    let f = std::fs::File::open(CONFIG_PATH.as_path())?;
    let config: MyConfig = serde_yaml::from_reader(f)?;
    Ok(config)
}

fn write_my_config(d: &MyConfig) -> Result<(), Box<dyn Error>> {
    let f = std::fs::File::create(CONFIG_PATH.as_path())?;
    serde_yaml::to_writer(f, d)?;
    Ok(())
}

fn main() {
    // describe the main window
    let main_window = WindowDesc::new(build_root_widget())
        .title(WINDOW_TITLE)
        .with_min_size((400.0, 320.0))
        .window_size((400.0, 320.0));

    // create the initial app state
    let mut initial_state = ConfiguratorState::default();

    match read_my_config() {
        Ok(config) => {
            initial_state.isp = config.isp;
            initial_state.userid = config.userid;
            initial_state.password = config.password;
        }
        Err(_) => {}
    }
    initial_state.enabled = AUTO_LAUNCH.is_enabled().unwrap_or(false);

    // start the application
    AppLauncher::with_window(main_window)
        .launch(initial_state)
        .expect("Failed to launch application");
}

fn build_root_widget() -> impl Widget<ConfiguratorState> {
    let isp_label = Label::new("ISP").fix_width(100.0);
    let isp_radio_group = RadioGroup::column(vec![
        ("EDU", IspType::EDU),
        ("CMCC", IspType::CMCC),
        ("CT", IspType::CT),
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

    let userid_label = Label::new("UserID").fix_width(100.0);
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

    let password_label = Label::new("Password").fix_width(100.0);
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

    let enable_checkbox = Checkbox::new("Enable")
        .lens(ConfiguratorState::enabled)
        .align_left();

    let note_label =
        Label::new("Note: The configuration won't take effect until rebooting.").align_left();

    let save_button = Button::new("Save")
        .on_click(|_ctx, data: &mut ConfiguratorState, _env| {
            let config = MyConfig {
                userid: data.userid.to_owned(),
                password: data.password.to_owned(),
                isp: data.isp,
            };
            let _ = write_my_config(&config);
            let _ = if data.enabled {
                AUTO_LAUNCH.enable()
            } else {
                AUTO_LAUNCH.disable()
            };
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
        .with_child(enable_checkbox)
        .with_default_spacer()
        .with_child(note_label)
        .with_default_spacer()
        .with_child(save_button);

    // center the two widgets in the available space
    Align::centered(layout)
}
