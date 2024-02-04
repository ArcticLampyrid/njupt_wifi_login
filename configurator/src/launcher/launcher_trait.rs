use njupt_wifi_login_configuration::password::PasswordScope;

pub trait Launcher {
    fn name(&self) -> String;
    fn enable(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>>;
    fn disable(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>>;
    fn start(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>>;
    fn stop(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>>;
    fn is_enabled(&self) -> Result<bool, Box<dyn std::error::Error + Sync + Send>>;
    fn is_running(&self) -> Result<bool, Box<dyn std::error::Error + Sync + Send>>;
    fn is_password_scope_supported(&self, scope: &PasswordScope) -> bool;
}
