// Email service — EmailProvider trait with ResendProvider (reqwest HTTPS) and SesProvider (lettre SMTP).
// Provider selected by EMAIL_PROVIDER env var (default: resend). EMAIL_FROM sets the sender address.
// If the selected provider is unconfigured, send_email logs and returns a clear Err (no silent success).

use std::collections::HashMap;

use lettre::{
    message::{header, MultiPart, SinglePart},
    transport::smtp::{
        authentication::{Credentials, Mechanism},
        PoolConfig,
    },
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use serde_json::json;

use crate::error::{AppError, Result};

// ---------------------------------------------------------------------------
// EmailProvider trait
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
pub trait EmailProvider: Send + Sync {
    /// Send a single transactional email. Returns Err if the provider is unconfigured or the send fails.
    async fn send(&self, from: &str, to: &str, subject: &str, text: &str, html: &str)
        -> Result<()>;
}

// ---------------------------------------------------------------------------
// ResendProvider — HTTPS POST to https://api.resend.com/emails
// ---------------------------------------------------------------------------

pub struct ResendProvider {
    api_key: String,
    client: reqwest::Client,
}

impl ResendProvider {
    /// Construct from env. Returns None if RESEND_API_KEY is absent/empty.
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("RESEND_API_KEY")
            .ok()
            .filter(|k| !k.is_empty())?;
        Some(Self {
            api_key,
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait::async_trait]
impl EmailProvider for ResendProvider {
    async fn send(
        &self,
        from: &str,
        to: &str,
        subject: &str,
        text: &str,
        html: &str,
    ) -> Result<()> {
        let body = json!({
            "from": from,
            "to": [to],
            "subject": subject,
            "text": text,
            "html": html,
        });

        let resp = self
            .client
            .post("https://api.resend.com/emails")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Resend HTTP error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Resend returned {status}: {text}"
            )));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SesProvider — lettre SMTP to SES endpoint (email-smtp.<region>.amazonaws.com)
//
// Decision (§4c): lettre is already a Cargo.toml dependency (SMTP transport + builder features
// present). We implement SES via lettre's AsyncSmtpTransport over TLS to the SES SMTP endpoint,
// using SES SMTP credentials (AWS_SES_SMTP_USER / AWS_SES_SMTP_PASSWORD / AWS_SES_REGION).
// This avoids the heavy aws-sdk-sesv2/aws-config crate entirely (aws-config is a dep for S3 only;
// we deliberately do NOT pull sesv2 for email). The transport is STARTTLS on port 587.
// ---------------------------------------------------------------------------

pub struct SesProvider {
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl SesProvider {
    /// Construct from env. Returns None if SES SMTP credentials are absent/empty.
    /// Required env vars:
    ///   AWS_SES_SMTP_USER     — SES SMTP username
    ///   AWS_SES_SMTP_PASSWORD — SES SMTP password
    ///   AWS_SES_REGION        — AWS region (default: us-east-1)
    pub fn from_env() -> Option<Self> {
        let user = std::env::var("AWS_SES_SMTP_USER")
            .ok()
            .filter(|s| !s.is_empty())?;
        let pass = std::env::var("AWS_SES_SMTP_PASSWORD")
            .ok()
            .filter(|s| !s.is_empty())?;
        let region = std::env::var("AWS_SES_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let host = format!("email-smtp.{}.amazonaws.com", region);

        let creds = Credentials::new(user, pass);
        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
            .ok()?
            .credentials(creds)
            .authentication(vec![Mechanism::Login, Mechanism::Plain])
            .pool_config(PoolConfig::new().max_size(5))
            .build();

        Some(Self { transport })
    }
}

#[async_trait::async_trait]
impl EmailProvider for SesProvider {
    async fn send(
        &self,
        from: &str,
        to: &str,
        subject: &str,
        text: &str,
        html: &str,
    ) -> Result<()> {
        let email = Message::builder()
            .from(
                from.parse()
                    .map_err(|e| AppError::Internal(format!("Invalid from address: {e}")))?,
            )
            .to(to
                .parse()
                .map_err(|e| AppError::Internal(format!("Invalid to address: {e}")))?)
            .subject(subject)
            .header(header::ContentType::TEXT_PLAIN)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(header::ContentType::TEXT_PLAIN)
                            .body(text.to_string()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(header::ContentType::TEXT_HTML)
                            .body(html.to_string()),
                    ),
            )
            .map_err(|e| AppError::Internal(format!("Failed to build email: {e}")))?;

        self.transport
            .send(email)
            .await
            .map_err(|e| AppError::Internal(format!("SES SMTP send error: {e}")))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// EmailService — selects provider from env; renders templates
// ---------------------------------------------------------------------------

pub struct EmailService {
    from_address: String,
    base_url: String,
    provider: Box<dyn EmailProvider>,
}

impl EmailService {
    /// Construct from environment variables. Returns Err if the selected provider is unconfigured.
    ///
    /// EMAIL_PROVIDER = "resend" (default) | "ses"
    /// EMAIL_FROM     = full sender address, e.g. "Magnetite <noreply@magnetite.gg>"
    /// APP_BASE_URL   = base URL for email links (default: http://localhost:5173)
    pub fn from_env() -> Result<Self> {
        let provider_name =
            std::env::var("EMAIL_PROVIDER").unwrap_or_else(|_| "resend".to_string());
        let from_address = std::env::var("EMAIL_FROM")
            .unwrap_or_else(|_| "Magnetite <noreply@magnetite.gg>".to_string());
        let base_url =
            std::env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());

        let provider: Box<dyn EmailProvider> = match provider_name.to_lowercase().as_str() {
            "ses" => {
                let p = SesProvider::from_env().ok_or_else(|| {
                    tracing::error!(
                        "EMAIL_PROVIDER=ses but SES SMTP credentials are not configured \
                         (AWS_SES_SMTP_USER / AWS_SES_SMTP_PASSWORD required)"
                    );
                    AppError::Internal(
                        "Email provider 'ses' is not configured: \
                         AWS_SES_SMTP_USER and AWS_SES_SMTP_PASSWORD must be set"
                            .to_string(),
                    )
                })?;
                Box::new(p)
            }
            _ => {
                // default: resend
                let p = ResendProvider::from_env().ok_or_else(|| {
                    tracing::error!(
                        "EMAIL_PROVIDER=resend (default) but RESEND_API_KEY is not configured"
                    );
                    AppError::Internal(
                        "Email provider 'resend' is not configured: RESEND_API_KEY must be set"
                            .to_string(),
                    )
                })?;
                Box::new(p)
            }
        };

        Ok(Self {
            from_address,
            base_url,
            provider,
        })
    }

    /// Low-level send — delegates to the selected provider.
    pub async fn send_email(&self, to: &str, subject: &str, text: &str, html: &str) -> Result<()> {
        tracing::info!(to = %to, subject = %subject, "Sending email via provider");
        self.provider
            .send(&self.from_address, to, subject, text, html)
            .await
    }

    // ------------------------------------------------------------------
    // Rendered template helpers
    // ------------------------------------------------------------------

    /// Send the email-verification email. token = raw token, link auto-generated.
    pub async fn send_verification_email(
        &self,
        to: &str,
        username: &str,
        token: &str,
    ) -> Result<()> {
        let link = format!("{}/verify-email?token={}", self.base_url, token);
        let subject = "Verify your Magnetite email address";
        let text = format!(
            "Hi {username},\n\nVerify your email address:\n{link}\n\nToken: {token}\n\nExpires in 24 hours."
        );
        let html = render_verify_email(username, token, &link, &self.base_url);
        self.send_email(to, subject, &text, &html).await
    }

    /// Send the password-reset email.
    pub async fn send_password_reset_email(
        &self,
        to: &str,
        username: &str,
        token: &str,
    ) -> Result<()> {
        let link = format!("{}/reset-password?token={}", self.base_url, token);
        let subject = "Reset your Magnetite password";
        let text = format!(
            "Hi {username},\n\nReset your password:\n{link}\n\nReset code: {token}\n\nExpires in 1 hour.\n\n\
             If you didn't request this, please ignore this email."
        );
        let html = render_password_reset(username, token, &link, &self.base_url);
        self.send_email(to, subject, &text, &html).await
    }

    /// Send the welcome email (after email is verified).
    pub async fn send_welcome_email(&self, to: &str, username: &str) -> Result<()> {
        let subject = "Welcome to Magnetite!";
        let text = format!(
            "Hi {username},\n\nWelcome to Magnetite — the open-source platform for Rust games.\n\n\
             Start exploring: {base}/games\n\nCheers, the Magnetite team",
            base = self.base_url
        );
        let html = render_welcome(username, &self.base_url);
        self.send_email(to, subject, &text, &html).await
    }
}

// ---------------------------------------------------------------------------
// Template renderers — inline HTML (avoids Tera/Handlebars engine complexity;
// templates use Jinja2 syntax which is not compatible with handlebars; we emit
// self-contained HTML using the same design as templates/emails/ but without the
// inheritance/block system that requires a full Tera engine).
// ---------------------------------------------------------------------------

fn email_wrapper(subject: &str, content: &str, base_url: &str) -> String {
    let year = chrono::Utc::now().format("%Y");
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{subject}</title>
</head>
<body style="margin:0;padding:0;background-color:#0a0a0f;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">
<center style="width:100%;background-color:#0a0a0f;">
<table role="presentation" align="center" border="0" cellpadding="0" cellspacing="0" width="100%" style="max-width:600px;margin:auto;background-color:#12121a;">
  <tr><td style="padding:40px 30px 30px 30px;text-align:center;">
    <a href="{base_url}" style="text-decoration:none;color:#38e1c8;font-size:22px;font-weight:700;letter-spacing:-0.5px;">Magnetite</a>
  </td></tr>
  {content}
  <tr><td style="padding:40px 30px;background-color:#0a0a0f;text-align:center;">
    <p style="margin:0 0 16px 0;font-size:14px;color:#6b7280;">&copy; {year} Magnetite. All rights reserved.</p>
    <p style="margin:0;font-size:12px;color:#4b5563;">
      <a href="{base_url}" style="color:#8b5cf6;text-decoration:none;">Website</a>
      &nbsp;&bull;&nbsp;
      <a href="{base_url}/privacy" style="color:#8b5cf6;text-decoration:none;">Privacy Policy</a>
      &nbsp;&bull;&nbsp;
      <a href="mailto:support@magnetite.gg" style="color:#8b5cf6;text-decoration:none;">Support</a>
    </p>
  </td></tr>
</table>
</center>
</body>
</html>"#
    )
}

fn render_verify_email(username: &str, token: &str, link: &str, base_url: &str) -> String {
    let vars: HashMap<&str, &str> = [("username", username), ("token", token), ("link", link)]
        .into_iter()
        .collect();
    let content = format!(
        r#"
  <tr><td style="padding:50px 30px 40px 30px;text-align:center;">
    <h1 style="margin:0 0 20px 0;font-size:28px;font-weight:700;color:#ffffff;line-height:1.2;">Verify your email</h1>
    <p style="margin:0 0 30px 0;font-size:16px;color:#9ca3af;line-height:1.6;">Hi {username}, enter the code below or click the button to verify your email address.</p>
  </td></tr>
  <tr><td style="padding:0 30px 30px 30px;text-align:center;">
    <table role="presentation" align="center" border="0" cellpadding="0" cellspacing="0" style="margin:auto;">
      <tr><td style="padding:24px 40px;background-color:#1a1a26;border-radius:12px;border:2px solid #38e1c8;">
        <span style="font-size:28px;font-weight:700;color:#ffffff;letter-spacing:6px;">{token}</span>
      </td></tr>
    </table>
  </td></tr>
  <tr><td style="padding:0 30px 30px 30px;text-align:center;">
    <a href="{link}" style="display:inline-block;padding:14px 32px;background-color:#38e1c8;color:#07070b;font-size:15px;font-weight:600;text-decoration:none;border-radius:8px;">Verify Email</a>
  </td></tr>
  <tr><td style="padding:0 30px 40px 30px;text-align:center;">
    <p style="margin:0;font-size:14px;color:#6b7280;">This code expires in <strong style="color:#9ca3af;">24 hours</strong>.</p>
  </td></tr>
"#,
        username = esc(vars["username"]),
        token = esc(vars["token"]),
        link = esc(vars["link"]),
    );
    email_wrapper("Verify your Magnetite email address", &content, base_url)
}

fn render_password_reset(username: &str, token: &str, link: &str, base_url: &str) -> String {
    let content = format!(
        r#"
  <tr><td style="padding:50px 30px 40px 30px;text-align:center;">
    <h1 style="margin:0 0 20px 0;font-size:28px;font-weight:700;color:#ffffff;line-height:1.2;">Reset your password</h1>
    <p style="margin:0 0 30px 0;font-size:16px;color:#9ca3af;line-height:1.6;">Hi {username}, use the code below to reset your Magnetite password.</p>
  </td></tr>
  <tr><td style="padding:0 30px 30px 30px;text-align:center;">
    <table role="presentation" align="center" border="0" cellpadding="0" cellspacing="0" style="margin:auto;">
      <tr><td style="padding:24px 40px;background-color:#1a1a26;border-radius:12px;border:2px solid #8b5cf6;">
        <span style="font-size:28px;font-weight:700;color:#ffffff;letter-spacing:6px;">{token}</span>
      </td></tr>
    </table>
  </td></tr>
  <tr><td style="padding:0 30px 30px 30px;text-align:center;">
    <a href="{link}" style="display:inline-block;padding:14px 32px;background-color:#8b5cf6;color:#ffffff;font-size:15px;font-weight:600;text-decoration:none;border-radius:8px;">Reset Password</a>
  </td></tr>
  <tr><td style="padding:0 30px 30px 30px;text-align:center;">
    <p style="margin:0;font-size:14px;color:#f59e0b;">This code expires in <strong>1 hour</strong>.</p>
  </td></tr>
  <tr><td style="padding:0 30px 40px 30px;text-align:center;">
    <p style="margin:0;font-size:13px;color:#6b7280;">If you didn't request this password reset, you can safely ignore this email.</p>
  </td></tr>
"#,
        username = esc(username),
        token = esc(token),
        link = esc(link),
    );
    email_wrapper("Reset your Magnetite password", &content, base_url)
}

fn render_welcome(username: &str, base_url: &str) -> String {
    let content = format!(
        r#"
  <tr><td style="padding:50px 30px 40px 30px;text-align:center;">
    <h1 style="margin:0 0 20px 0;font-size:32px;font-weight:700;color:#ffffff;line-height:1.2;">Welcome to Magnetite, {username}!</h1>
    <p style="margin:0 0 30px 0;font-size:18px;color:#9ca3af;line-height:1.6;">Your email is verified. You're ready to build, distribute, and play Rust games at any scale.</p>
    <a href="{base_url}/games" style="display:inline-block;padding:16px 40px;background-color:#38e1c8;color:#07070b;font-size:16px;font-weight:600;text-decoration:none;border-radius:8px;">Explore Games</a>
  </td></tr>
  <tr><td style="padding:0 30px 50px 30px;text-align:center;">
    <p style="margin:0 0 24px 0;font-size:15px;color:#9ca3af;line-height:1.6;">
      1. Complete your profile<br>
      2. Browse and play games<br>
      3. Build your own game with the Magnetite SDK
    </p>
    <a href="{base_url}/profile" style="color:#8b5cf6;text-decoration:none;font-size:15px;">Complete Your Profile &rarr;</a>
  </td></tr>
"#,
        username = esc(username),
        base_url = base_url,
    );
    email_wrapper("Welcome to Magnetite!", &content, base_url)
}

/// Minimal HTML-escape to prevent injection in rendered templates.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_esc_html() {
        assert_eq!(esc("<script>"), "&lt;script&gt;");
        assert_eq!(esc("a & b"), "a &amp; b");
        assert_eq!(esc("\"hi\""), "&quot;hi&quot;");
    }

    #[test]
    fn test_render_verify_email_contains_token() {
        let html = render_verify_email(
            "alice",
            "TOK123",
            "http://localhost/verify?token=TOK123",
            "http://localhost",
        );
        assert!(html.contains("TOK123"));
        assert!(html.contains("alice"));
        assert!(html.contains("http://localhost/verify?token=TOK123"));
    }

    #[test]
    fn test_render_password_reset_contains_token() {
        let html = render_password_reset(
            "bob",
            "RST456",
            "http://localhost/reset?token=RST456",
            "http://localhost",
        );
        assert!(html.contains("RST456"));
        assert!(html.contains("bob"));
    }

    #[test]
    fn test_render_welcome_contains_username() {
        let html = render_welcome("carol", "http://localhost");
        assert!(html.contains("carol"));
        assert!(html.contains("http://localhost/games"));
    }

    #[test]
    fn test_email_wrapper_contains_year() {
        let html = email_wrapper(
            "Test Subject",
            "<tr><td>content</td></tr>",
            "http://localhost",
        );
        let year = chrono::Utc::now().format("%Y").to_string();
        assert!(html.contains(&year));
        assert!(html.contains("content"));
    }
}
