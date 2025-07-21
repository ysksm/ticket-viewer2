use dotenv::dotenv;
use std::env;

use jira_api::{
    Auth,
    JiraConfig
};

#[tokio::main]
async fn main() {
    dotenv().ok();
        // Get configuration from environment variables
    let base_url = env::var("JIRA_URL")
        .unwrap_or_else(|_| "https://your-domain.atlassian.net".to_string());
    let username = env::var("JIRA_USER")
        .unwrap_or_else(|_| "your-email@example.com".to_string());
    let api_token = env::var("JIRA_API_TOKEN")
        .unwrap_or_else(|_| "your-api-token".to_string());
    println!("Base URL: {}", base_url);
    println!("Username: {}", username);

    let config = JiraConfig::new(
        base_url,
        Auth::Basic {
            username,
            api_token,
        },
    );

    


}