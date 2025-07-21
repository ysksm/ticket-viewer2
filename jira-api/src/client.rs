use crate::error::Result;
use url::Url;

#[derive(Debug, Clone)]
pub enum Auth {
    Basic { username: String, api_token: String },
    Bearer { token: String },
}

#[derive(Debug, Clone)]
pub struct JiraConfig {
    pub base_url: String,
    pub auth: Auth,
}

impl JiraConfig {
    pub fn new(base_url: impl Into<String>, auth: Auth) -> Result<Self> {
        let base_url = base_url.into();
        
        // Validate URL
        let _ = Url::parse(&base_url)
            .map_err(|_| crate::error::Error::InvalidConfiguration("Invalid base URL".to_string()))?;
        
        Ok(Self {
            base_url,
            auth,
        })
    }
}

#[derive(Debug, Clone)]
pub struct JiraClient {
    // pub(crate) client: Client,
    // pub(crate) config: Arc<JiraConfig>,
}