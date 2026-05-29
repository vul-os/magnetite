// Email service — transactional emails via Resend or SMTP; platform surface, not yet wired.
#![allow(dead_code)]

pub struct EmailService {
    from_address: String,
    from_name: String,
    smtp_host: String,
    smtp_port: u16,
    username: String,
    password: String,
}

impl EmailService {
    pub fn new(
        from_address: String,
        from_name: String,
        smtp_host: String,
        smtp_port: u16,
        username: String,
        password: String,
    ) -> Self {
        Self {
            from_address,
            from_name,
            smtp_host,
            smtp_port,
            username,
            password,
        }
    }

    pub fn mock() -> Self {
        Self {
            from_address: "test@example.com".to_string(),
            from_name: "Test User".to_string(),
            smtp_host: "localhost".to_string(),
            smtp_port: 587,
            username: "test".to_string(),
            password: "test".to_string(),
        }
    }

    pub async fn send_email(
        &self,
        to: &str,
        subject: &str,
        _text: &str,
        _html: &str,
    ) -> Result<(), crate::error::AppError> {
        tracing::info!("Email would be sent to {} with subject: {}", to, subject);
        tracing::debug!("From: {} <{}>", self.from_name, self.from_address);
        Ok(())
    }
}
