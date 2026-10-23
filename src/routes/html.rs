use crate::auth::OptionalUser;
use crate::blocks;
use crate::pages;
use crate::state::AppState;
use crate::util::{escape, RESERVED_USERNAMES};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use serde_json::Value;

pub async fn home(State(state): State<AppState>) -> impl IntoResponse {
    template(&state, "home.html")
}

pub async fn login(State(state): State<AppState>, OptionalUser(user): OptionalUser) -> Response {
    if user.is_some() {
        return redirect("/dashboard");
    }
    template(&state, "login.html")
}

pub async fn register(State(state): State<AppState>, OptionalUser(user): OptionalUser) -> Response {
    if user.is_some() {
        return redirect("/dashboard");
    }
    template(&state, "register.html")
}

pub async fn dashboard(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> Response {
    if user.is_none() {
        return redirect("/login");
    }
    template(&state, "dashboard.html")
}

pub async fn editor(State(state): State<AppState>, OptionalUser(user): OptionalUser) -> Response {
    if user.is_none() {
        return redirect("/login");
    }
    template(&state, "editor.html")
}

pub async fn admin(State(state): State<AppState>, OptionalUser(user): OptionalUser) -> Response {
    match user {
        Some(user) if user.is_admin() => template(&state, "admin.html"),
        Some(_) => error_page(StatusCode::FORBIDDEN, "دسترسی مدیریت لازم است"),
        None => redirect("/login"),
    }
}

pub async fn preview(State(state): State<AppState>, OptionalUser(user): OptionalUser) -> Response {
    let Some(user) = user else {
        return redirect("/login");
    };
    match pages::find_by_user_id(&state.pool, &user.id).await {
        Ok(page) => match blocks::list_visible(&state.pool, &page.id).await {
            Ok(blocks) => {
                let html = render_public_page(&page, &blocks, true);
                Html(html).into_response()
            }
            Err(_) => error_page(StatusCode::INTERNAL_SERVER_ERROR, "خطای داخلی"),
        },
        Err(_) => error_page(StatusCode::NOT_FOUND, "صفحه پیدا نشد"),
    }
}

pub async fn public_page(State(state): State<AppState>, Path(username): Path<String>) -> Response {
    let slug = username.to_ascii_lowercase();
    if RESERVED_USERNAMES.contains(&slug.as_str()) {
        return error_page(StatusCode::NOT_FOUND, "صفحه پیدا نشد");
    }
    match pages::find_public_by_slug(&state.pool, &slug).await {
        Ok(Some(page)) => match blocks::list_visible(&state.pool, &page.id).await {
            Ok(blocks) => Html(render_public_page(&page, &blocks, false)).into_response(),
            Err(_) => error_page(StatusCode::INTERNAL_SERVER_ERROR, "خطای داخلی"),
        },
        Ok(None) => error_page(StatusCode::NOT_FOUND, "این صفحه منتشر نشده یا وجود ندارد"),
        Err(_) => error_page(StatusCode::INTERNAL_SERVER_ERROR, "خطای داخلی"),
    }
}

pub async fn not_found() -> Response {
    error_page(StatusCode::NOT_FOUND, "صفحه پیدا نشد")
}

pub async fn robots() -> &'static str {
    "User-agent: *\nDisallow: /dashboard\nDisallow: /editor\nDisallow: /admin\nDisallow: /api/\n"
}

fn redirect(to: &str) -> Response {
    axum::response::Redirect::to(to).into_response()
}

fn template(state: &AppState, name: &str) -> Response {
    match load_template(&state.config.asset_root, name) {
        Ok(body) => Html(body).into_response(),
        Err(err) => {
            tracing::error!(error = %err, template = name, "missing template");
            error_page(StatusCode::INTERNAL_SERVER_ERROR, "قالب پیدا نشد")
        }
    }
}

fn load_template(root: &std::path::Path, name: &str) -> Result<String, String> {
    let path = root.join("templates").join(name);
    if path.is_file() {
        return std::fs::read_to_string(path).map_err(|e| e.to_string());
    }
    match name {
        "home.html" => Ok(include_str!("../../templates/home.html").to_string()),
        "login.html" => Ok(include_str!("../../templates/login.html").to_string()),
        "register.html" => Ok(include_str!("../../templates/register.html").to_string()),
        "dashboard.html" => Ok(include_str!("../../templates/dashboard.html").to_string()),
        "editor.html" => Ok(include_str!("../../templates/editor.html").to_string()),
        "admin.html" => Ok(include_str!("../../templates/admin.html").to_string()),
        "public_page.html" => Ok(include_str!("../../templates/public_page.html").to_string()),
        _ => Err(format!("unknown template {name}")),
    }
}

fn error_page(status: StatusCode, message: &str) -> Response {
    let body = format!(
        r#"<!DOCTYPE html><html lang="fa" dir="rtl"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{msg}</title><link rel="icon" type="image/png" href="/static/images/linko-icon.png"><link rel="stylesheet" href="/static/css/app.css"></head><body class="error-body"><main class="error-card"><a class="brand" href="/" aria-label="Linko"><img class="brand-logo" src="/static/images/linko-logo-light.png" alt="Linko" width="150" height="46"></a><h1>{msg}</h1><p>می‌توانید به <a href="/">صفحهٔ اصلی</a> برگردید.</p></main></body></html>"#,
        msg = escape(message)
    );
    (status, Html(body)).into_response()
}

fn render_public_page(page: &pages::Page, blocks: &[blocks::Block], is_preview: bool) -> String {
    let title = page
        .seo_title
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(&page.display_name);
    let desc = page.seo_description.clone().unwrap_or_default();
    let robots = if is_preview {
        "noindex,nofollow"
    } else {
        "index,follow"
    };
    let preview_banner = if is_preview {
        r#"<div class="preview-banner">پیش‌نمایش خصوصی — این صفحه برای بازدیدکنندگان عمومی نیست مگر اینکه منتشر شده باشد. <a href="/editor">بازگشت به ویرایشگر</a></div>"#
    } else {
        ""
    };
    let avatar = page
        .avatar_url
        .as_deref()
        .map(|url| {
            format!(
                r#"<img class="avatar" src="{}" alt="{}">"#,
                escape(url),
                escape(&page.display_name)
            )
        })
        .unwrap_or_else(|| {
            let initial = page.display_name.chars().next().unwrap_or('L');
            format!(
                r#"<span class="avatar avatar-fallback" aria-hidden="true">{}</span>"#,
                escape(&initial.to_string())
            )
        });
    let cover = page
        .cover_url
        .as_deref()
        .map(|url| {
            format!(
                r#"<div class="public-cover"><img src="{}" alt="" aria-hidden="true"></div>"#,
                escape(url)
            )
        })
        .unwrap_or_default();
    let mut body_blocks = String::new();
    if blocks.is_empty() {
        body_blocks
            .push_str(r#"<div class="empty-public"><p>هنوز محتوایی منتشر نشده است.</p></div>"#);
    }
    for block in blocks {
        body_blocks.push_str(&render_block(block));
    }
    format!(
        r#"<!DOCTYPE html>
<html lang="fa" dir="rtl" data-theme="{theme}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <meta name="description" content="{desc}">
  <meta name="robots" content="{robots}">
  <link rel="icon" type="image/png" href="/static/images/linko-icon.png">
  <link rel="stylesheet" href="/static/css/app.css">
  <link rel="stylesheet" href="/static/css/themes.css">
</head>
<body class="public-body theme-{theme}" style="--accent:{accent}">
{preview}
<main class="public-page">
  <header class="public-hero">
    {cover}
    {avatar}
    <div class="public-identity">
      <p class="public-slug"><span aria-hidden="true">@</span>{slug}</p>
      <h1>{name}</h1>
      <p class="public-bio">{bio}</p>
    </div>
  </header>
  <section class="public-blocks" aria-label="محتوای صفحه">{blocks}</section>
  <footer class="public-foot">
    <a class="linko-signature" href="/" aria-label="ساخته‌شده با Linko">
      <img class="signature-logo signature-logo-light" src="/static/images/linko-logo-light.png" alt="Linko" width="100" height="31">
      <img class="signature-logo signature-logo-dark" src="/static/images/linko-logo.png" alt="Linko" width="100" height="31">
    </a>
  </footer>
</main>
</body>
</html>"#,
        theme = escape(&page.theme),
        title = escape(title),
        desc = escape(&desc),
        robots = robots,
        accent = escape(&page.accent_color),
        cover = cover,
        preview = preview_banner,
        avatar = avatar,
        slug = escape(&page.slug),
        name = escape(&page.display_name),
        bio = escape(&page.bio).replace('\n', "<br>"),
        blocks = body_blocks,
    )
}

fn render_block(block: &blocks::Block) -> String {
    let content = block.content();
    match block.block_type.as_str() {
        "heading" => {
            let level = content
                .get("level")
                .and_then(Value::as_u64)
                .unwrap_or(2)
                .clamp(1, 3);
            let text = escape(content.get("text").and_then(Value::as_str).unwrap_or(""));
            format!("<h{level} class=\"block heading\">{text}</h{level}>")
        }
        "paragraph" => format!(
            "<p class=\"block paragraph\">{}</p>",
            escape(content.get("text").and_then(Value::as_str).unwrap_or("")).replace('\n', "<br>")
        ),
        "link" => safe_link(
            content.get("url").and_then(Value::as_str).unwrap_or("#"),
            content
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("پیوند"),
            "block link-chip",
        ),
        "social_links" => {
            let mut html = String::from("<ul class=\"block social-list\">");
            if let Some(items) = content.get("items").and_then(Value::as_array) {
                for item in items {
                    let label = item
                        .get("label")
                        .and_then(Value::as_str)
                        .unwrap_or("social");
                    let url = item.get("url").and_then(Value::as_str).unwrap_or("#");
                    html.push_str("<li>");
                    html.push_str(&safe_link(url, label, "social-chip"));
                    html.push_str("</li>");
                }
            }
            html.push_str("</ul>");
            html
        }
        "divider" => "<hr class=\"block divider\">".into(),
        "image" => {
            let url = escape(content.get("url").and_then(Value::as_str).unwrap_or(""));
            let alt = escape(content.get("alt").and_then(Value::as_str).unwrap_or(""));
            format!(r#"<figure class="block image"><img src="{url}" alt="{alt}"></figure>"#)
        }
        "cta" => safe_link(
            content.get("url").and_then(Value::as_str).unwrap_or("#"),
            content
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("ادامه"),
            "block cta",
        ),
        "project_card" => {
            let title = escape(content.get("title").and_then(Value::as_str).unwrap_or(""));
            let desc = escape(
                content
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            );
            let img = content
                .get("image_url")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(|u| format!(r#"<img src="{}" alt="">"#, escape(u)))
                .unwrap_or_default();
            let link = content
                .get("url")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(|u| safe_link(u, "مشاهده پروژه", "text-link"))
                .unwrap_or_default();
            format!(
                r#"<article class="block project-card">{img}<div><h3>{title}</h3><p>{desc}</p>{link}</div></article>"#
            )
        }
        "skills" => {
            let mut html = String::from("<ul class=\"block skills\">");
            if let Some(items) = content.get("items").and_then(Value::as_array) {
                for item in items {
                    html.push_str(&format!("<li>{}</li>", escape(item.as_str().unwrap_or(""))));
                }
            }
            html.push_str("</ul>");
            html
        }
        "contact" => {
            let email = escape(content.get("email").and_then(Value::as_str).unwrap_or(""));
            let phone = escape(content.get("phone").and_then(Value::as_str).unwrap_or(""));
            let location = escape(
                content
                    .get("location")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            );
            format!(
                r#"<address class="block contact"><p>{email}</p><p>{phone}</p><p>{location}</p></address>"#
            )
        }
        _ => String::new(),
    }
}

fn safe_link(url: &str, label: &str, class: &str) -> String {
    if crate::util::validate_http_url(url).is_err() {
        return format!("<span class=\"{class}\">{}</span>", escape(label));
    }
    format!(
        r#"<a class="{class}" href="{url}" target="_blank" rel="noopener noreferrer nofollow">{label}</a>"#,
        url = escape(url),
        label = escape(label)
    )
}
