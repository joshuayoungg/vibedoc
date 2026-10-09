//! Which AI services to use. Saved as TOML so people can also edit it by hand:
//! `~/.config/vibedoc/config.toml` on Linux, the platform equivalent elsewhere.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub agent: AgentConfig,
    pub images: ImageConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentProvider {
    Anthropic,
    /// Any service that speaks OpenAI's chat API: OpenAI, Gemini, DeepSeek,
    /// OpenRouter, Groq, or a local model through Ollama or LM Studio.
    OpenaiCompatible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageProvider {
    Gemini,
    Openai,
}

/// The model that reads requests and edits the document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub provider: AgentProvider,
    pub model: String,
    /// Leave empty to read the key from the `api_key_env` environment variable.
    pub api_key: String,
    pub api_key_env: String,
    pub base_url: String,
    /// Extra instructions added to the assistant's system prompt.
    pub instructions: String,
}

/// The model that draws pictures.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageConfig {
    pub provider: ImageProvider,
    pub model: String,
    pub api_key: String,
    pub api_key_env: String,
    pub base_url: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        AgentConfig::for_provider(AgentProvider::Anthropic)
    }
}

impl Default for ImageConfig {
    fn default() -> Self {
        ImageConfig::for_provider(ImageProvider::Gemini)
    }
}

impl AgentConfig {
    /// Sensible settings for a provider, used as defaults and when switching.
    pub fn for_provider(provider: AgentProvider) -> Self {
        let (model, api_key_env, base_url) = match provider {
            AgentProvider::Anthropic => {
                ("claude-sonnet-5-5", "ANTHROPIC_API_KEY", "https://api.anthropic.com")
            }
            AgentProvider::OpenaiCompatible => ("gpt-5", "OPENAI_API_KEY", "https://api.openai.com/v1"),
        };
        AgentConfig {
            provider,
            model: model.into(),
            api_key: String::new(),
            api_key_env: api_key_env.into(),
            base_url: base_url.into(),
            instructions: String::new(),
        }
    }

    pub fn api_key(&self) -> Result<String, String> {
        resolve_api_key(&self.api_key, &self.api_key_env)
    }
}

impl ImageConfig {
    pub fn for_provider(provider: ImageProvider) -> Self {
        let (model, api_key_env, base_url) = match provider {
            ImageProvider::Gemini => {
                ("gemini-nano-banana-2.1", "GEMINI_API_KEY", "https://generativelanguage.googleapis.com")
            }
            ImageProvider::Openai => ("gpt-image-1", "OPENAI_API_KEY", "https://api.openai.com/v1"),
        };
        ImageConfig {
            provider,
            model: model.into(),
            api_key: String::new(),
            api_key_env: api_key_env.into(),
            base_url: base_url.into(),
        }
    }

    pub fn api_key(&self) -> Result<String, String> {
        resolve_api_key(&self.api_key, &self.api_key_env)
    }
}

fn resolve_api_key(api_key: &str, env_var: &str) -> Result<String, String> {
    if !api_key.trim().is_empty() {
        return Ok(api_key.trim().to_string());
    }
    match std::env::var(env_var) {
        Ok(key) if !key.trim().is_empty() => Ok(key.trim().to_string()),
        _ => Err(format!("No API key. Set the {env_var} environment variable, or add a key in Settings.")),
    }
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("vibedoc").join("config.toml"))
    }

    /// Loads the saved config, or the defaults if there isn't one yet.
    pub fn load() -> Result<Config, String> {
        let Some(path) = Config::path() else { return Ok(Config::default()) };
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Config::path().ok_or("Couldn't find a config folder on this computer")?;
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let config: Config = toml::from_str("[agent]\nmodel = \"claude-opus-5-5\"").unwrap();
        assert_eq!(config.agent.model, "claude-opus-5-5");
        assert_eq!(config.agent.provider, AgentProvider::Anthropic);
        assert_eq!(config.images, ImageConfig::default());
    }

    #[test]
    fn config_round_trips_through_toml() {
        let agent = AgentConfig {
            instructions: "Write like a pirate.".into(),
            ..AgentConfig::for_provider(AgentProvider::OpenaiCompatible)
        };
        let config = Config { agent, ..Default::default() };
        let text = toml::to_string_pretty(&config).unwrap();
        assert_eq!(toml::from_str::<Config>(&text).unwrap(), config);
    }

    #[test]
    fn a_key_in_the_config_wins_over_the_environment() {
        assert_eq!(resolve_api_key(" abc ", "VIBEDOC_TEST_UNSET_VAR"), Ok("abc".into()));
        assert!(resolve_api_key("", "VIBEDOC_TEST_UNSET_VAR").is_err());
    }
}
