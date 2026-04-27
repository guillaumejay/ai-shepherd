use crate::terminal::ShellContext;
use std::path::PathBuf;

pub struct PathResolver;

impl PathResolver {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir().expect("config dir").join("ai-shepherd")
    }
    pub fn data_dir() -> PathBuf {
        dirs::data_local_dir()
            .expect("data dir")
            .join("ai-shepherd")
    }
    pub fn resolve_home(relative: &str, context: &ShellContext) -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        if let ShellContext::Wsl { distro } = context {
            return Self::wsl_home(distro).map(|home| home.join(relative));
        }
        dirs::home_dir().map(|home| home.join(relative))
    }
    pub fn all_candidates(relative: &str) -> Vec<PathBuf> {
        let mut paths = dirs::home_dir()
            .map(|home| vec![home.join(relative)])
            .unwrap_or_default();
        #[cfg(target_os = "windows")]
        {
            paths.extend(Self::wsl_candidates(relative));
        }
        paths
    }
    #[cfg(target_os = "windows")]
    fn wsl_candidates(_relative: &str) -> Vec<PathBuf> {
        Vec::new()
    }
    #[cfg(target_os = "windows")]
    fn wsl_home(_distro: &str) -> Option<PathBuf> {
        None
    }
}
