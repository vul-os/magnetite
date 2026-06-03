// Server-rendered HTML for the super-admin panel.
//
// Everything here is self-contained: no external fonts, scripts, or stylesheets,
// so the panel can run under a strict `default-src 'none'` CSP. Styling is a
// single inline stylesheet keyed to the "Industrial Magnetite" palette. All
// dynamic text MUST be passed through `esc()` before interpolation.

/// HTML-escape untrusted text for safe interpolation into element bodies and
/// double-quoted attribute values.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            '/' => out.push_str("&#x2f;"),
            _ => out.push(c),
        }
    }
    out
}

/// A navigation entry in the panel chrome. `(href, label, is_active)`.
pub struct Nav<'a> {
    pub href: &'a str,
    pub label: &'a str,
    pub active: bool,
}

const STYLE: &str = r#"
:root{
  --bg:#0a0c0f; --panel:#11151b; --panel-2:#161b22; --line:#222b35;
  --ink:#e6edf3; --muted:#8b98a5; --accent:#38e1c8; --amber:#f5a524;
  --success:#3ddc84; --error:#ff5c7a; --mono:ui-monospace,'SF Mono',Menlo,Consolas,monospace;
}
*{box-sizing:border-box}
html,body{margin:0;padding:0}
body{
  background:var(--bg);color:var(--ink);
  font-family:var(--mono);font-size:14px;line-height:1.5;
  -webkit-font-smoothing:antialiased;
}
a{color:var(--accent);text-decoration:none}
a:hover{text-decoration:underline}
.wrap{display:flex;min-height:100vh}
.side{
  width:212px;flex:0 0 212px;background:var(--panel);border-right:1px solid var(--line);
  padding:20px 0;position:sticky;top:0;height:100vh;
}
.brand{padding:0 20px 18px;font-weight:700;letter-spacing:.14em;text-transform:uppercase;font-size:12px}
.brand b{color:var(--accent)}
.brand .tag{display:block;color:var(--muted);font-size:10px;letter-spacing:.18em;margin-top:4px}
.nav a{display:block;padding:9px 20px;color:var(--muted);border-left:2px solid transparent;font-size:13px}
.nav a:hover{color:var(--ink);background:var(--panel-2);text-decoration:none}
.nav a.on{color:var(--accent);border-left-color:var(--accent);background:var(--panel-2)}
.nav .sep{height:1px;background:var(--line);margin:12px 20px}
.main{flex:1;min-width:0;padding:28px 34px 60px}
h1{font-size:19px;letter-spacing:.04em;margin:0 0 4px}
h2{font-size:13px;letter-spacing:.14em;text-transform:uppercase;color:var(--muted);margin:30px 0 12px;font-weight:600}
.sub{color:var(--muted);font-size:12px;margin:0 0 22px}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(190px,1fr));gap:14px}
.card{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:16px 18px}
.card .k{color:var(--muted);font-size:11px;letter-spacing:.12em;text-transform:uppercase}
.card .v{font-size:26px;font-weight:700;margin-top:8px;letter-spacing:.02em}
.card .v.sm{font-size:18px}
.card .v.accent{color:var(--accent)} .card .v.amber{color:var(--amber)}
.card .v.ok{color:var(--success)} .card .v.bad{color:var(--error)}
table{width:100%;border-collapse:collapse;font-size:12.5px;background:var(--panel);border:1px solid var(--line);border-radius:10px;overflow:hidden}
th,td{text-align:left;padding:9px 12px;border-bottom:1px solid var(--line);white-space:nowrap}
th{color:var(--muted);font-size:10.5px;letter-spacing:.12em;text-transform:uppercase;font-weight:600;background:var(--panel-2)}
tr:last-child td{border-bottom:none}
td.wrap{white-space:normal;max-width:340px}
.pill{display:inline-block;padding:2px 8px;border-radius:999px;font-size:10.5px;letter-spacing:.06em;text-transform:uppercase;border:1px solid var(--line)}
.pill.ok{color:var(--success);border-color:#1f4a33} .pill.bad{color:var(--error);border-color:#5a2230}
.pill.warn{color:var(--amber);border-color:#5a4420} .pill.mute{color:var(--muted)}
.btn{display:inline-block;padding:6px 12px;border-radius:7px;border:1px solid var(--line);background:var(--panel-2);color:var(--ink);font-family:var(--mono);font-size:12px;cursor:pointer}
.btn:hover{border-color:var(--accent);text-decoration:none}
.btn.danger:hover{border-color:var(--error);color:var(--error)}
.btn.go:hover{border-color:var(--success);color:var(--success)}
form.inline{display:inline}
input,select{background:var(--bg);border:1px solid var(--line);color:var(--ink);font-family:var(--mono);font-size:13px;padding:8px 10px;border-radius:7px;width:100%}
input:focus,select:focus{outline:none;border-color:var(--accent)}
.flash{padding:11px 14px;border-radius:8px;margin-bottom:18px;font-size:12.5px;border:1px solid}
.flash.ok{border-color:#1f4a33;background:#0e1c14;color:var(--success)}
.flash.err{border-color:#5a2230;background:#1c0e12;color:var(--error)}
.muted{color:var(--muted)} .accent{color:var(--accent)} .amber{color:var(--amber)}
.ok{color:var(--success)} .bad{color:var(--error)}
.row{display:flex;gap:10px;align-items:center;flex-wrap:wrap}
.bar{height:7px;border-radius:4px;background:var(--panel-2);overflow:hidden;min-width:80px;flex:1}
.bar>span{display:block;height:100%;background:var(--accent)}
.foot{color:var(--muted);font-size:11px;margin-top:40px;border-top:1px solid var(--line);padding-top:14px}
.login{max-width:360px;margin:14vh auto;padding:30px;background:var(--panel);border:1px solid var(--line);border-radius:12px}
.login h1{text-align:center;letter-spacing:.18em;text-transform:uppercase;font-size:15px}
.login .field{margin:16px 0}
.login label{display:block;color:var(--muted);font-size:11px;letter-spacing:.1em;text-transform:uppercase;margin-bottom:6px}
.login button{width:100%;margin-top:8px;padding:11px;background:var(--accent);color:#04100d;border:none;border-radius:8px;font-weight:700;font-family:var(--mono);cursor:pointer}
.login button:hover{filter:brightness(1.08)}
"#;

/// Render a full page with the standard chrome (sidebar + main column).
pub fn page(title: &str, nav: &[Nav], body: &str) -> String {
    let mut nav_html = String::new();
    for n in nav {
        if n.href.is_empty() {
            nav_html.push_str("<div class=\"sep\"></div>");
            continue;
        }
        nav_html.push_str(&format!(
            "<a href=\"{href}\"{on}>{label}</a>",
            href = esc(n.href),
            on = if n.active { " class=\"on\"" } else { "" },
            label = esc(n.label),
        ));
    }
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<meta name=\"robots\" content=\"noindex,nofollow\">\
<title>{title} · Magnetite Control</title><style>{style}</style></head>\
<body><div class=\"wrap\"><nav class=\"side\">\
<div class=\"brand\"><b>MAGNETITE</b><span class=\"tag\">Super Admin</span></div>\
<div class=\"nav\">{nav}</div></nav>\
<main class=\"main\">{body}\
<div class=\"foot\">Hardened control surface · all actions are audited · \
no external resources loaded</div></main></div></body></html>",
        title = esc(title),
        style = STYLE,
        nav = nav_html,
        body = body,
    )
}

/// Render the standalone login page (no sidebar chrome).
pub fn login_page(error: Option<&str>, csrf: &str) -> String {
    let flash = match error {
        Some(e) => format!("<div class=\"flash err\">{}</div>", esc(e)),
        None => String::new(),
    };
    let body = format!(
        "<div class=\"login\"><h1>Magnetite Control</h1>{flash}\
<form method=\"post\" action=\"/superadmin/login\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\">\
<div class=\"field\"><label>Email</label>\
<input type=\"email\" name=\"email\" autocomplete=\"username\" required autofocus></div>\
<div class=\"field\"><label>Password</label>\
<input type=\"password\" name=\"password\" autocomplete=\"current-password\" required></div>\
<button type=\"submit\">Authenticate</button></form></div>",
        flash = flash,
        csrf = esc(csrf),
    );
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<meta name=\"robots\" content=\"noindex,nofollow\">\
<title>Sign in · Magnetite Control</title><style>{style}</style></head>\
<body>{body}</body></html>",
        style = STYLE,
        body = body,
    )
}

/// A status pill (`ok`/`bad`/`warn`/`mute`) with the given label.
pub fn pill(kind: &str, label: &str) -> String {
    format!("<span class=\"pill {}\">{}</span>", kind, esc(label))
}
