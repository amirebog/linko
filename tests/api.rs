use axum::body::Body;
use axum::http::{header, HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use linko::config::Config;
use linko::db;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tower::ServiceExt;

struct TestApp {
    router: axum::Router,
    cookies: HashMap<String, String>,
    pool: sqlx::SqlitePool,
    _dir: tempfile::TempDir,
}

impl TestApp {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("linko.db");
        let config = Config::for_tests(db_path, Duration::from_secs(3600));
        let state = linko::build_state(config).await.unwrap();
        let pool = state.pool.clone();
        let router = linko::router(state);
        let mut app = Self {
            router,
            cookies: HashMap::new(),
            pool,
            _dir: dir,
        };
        let _ = app.get("/api/auth/csrf").await;
        app
    }

    fn cookie_header(&self) -> String {
        self.cookies
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn store_cookies(&mut self, headers: &HeaderMap) {
        for value in headers.get_all(header::SET_COOKIE) {
            let Ok(raw) = value.to_str() else { continue };
            let Some(pair) = raw.split(';').next() else {
                continue;
            };
            let Some((name, val)) = pair.split_once('=') else {
                continue;
            };
            let expired = raw.to_ascii_lowercase().contains("max-age=0");
            if expired || val.is_empty() {
                self.cookies.remove(name);
            } else {
                self.cookies.insert(name.to_string(), val.to_string());
            }
        }
    }

    async fn send(
        &mut self,
        mut builder: axum::http::request::Builder,
        body: Body,
    ) -> (StatusCode, Value, String) {
        builder = builder.header(header::COOKIE, self.cookie_header());
        if let Some(csrf) = self.cookies.get("linko_csrf").cloned() {
            builder = builder.header("x-csrf-token", csrf);
        }
        let request = builder.body(body).unwrap();
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        self.store_cookies(response.headers());
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let json = serde_json::from_str(&text).unwrap_or(Value::Null);
        (status, json, text)
    }

    async fn get(&mut self, uri: &str) -> (StatusCode, Value, String) {
        self.send(Request::builder().method("GET").uri(uri), Body::empty())
            .await
    }

    async fn post(&mut self, uri: &str, body: Value) -> (StatusCode, Value, String) {
        self.send(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json"),
            Body::from(body.to_string()),
        )
        .await
    }

    async fn patch(&mut self, uri: &str, body: Value) -> (StatusCode, Value, String) {
        self.send(
            Request::builder()
                .method("PATCH")
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json"),
            Body::from(body.to_string()),
        )
        .await
    }

    async fn delete(&mut self, uri: &str) -> (StatusCode, Value, String) {
        self.send(Request::builder().method("DELETE").uri(uri), Body::empty())
            .await
    }

    async fn register(&mut self, username: &str, email: &str) -> StatusCode {
        let (status, _, _) = self
            .post(
                "/api/auth/register",
                json!({
                    "username": username,
                    "email": email,
                    "password": "correct-horse",
                    "password_confirmation": "correct-horse"
                }),
            )
            .await;
        status
    }
}

#[tokio::test]
async fn database_initializes_from_empty_directory() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("nested");
    let db = nested.join("linko.db");
    assert!(!nested.exists());
    let pool = db::init_pool(&db).await.unwrap();
    let tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('users','pages','blocks','sessions','audit_logs')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tables, 5);
    let version: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(version >= 1);
}

#[tokio::test]
async fn registration_creates_user_and_page() {
    let mut app = TestApp::new().await;
    let status = app.register("builder_one", "one@example.com").await;
    assert_eq!(status, StatusCode::OK);
    let (status, me, _) = app.get("/api/auth/me").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["username"], "builder_one");
    let (status, page, _) = app.get("/api/pages/me").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["page"]["slug"], "builder_one");
    assert_eq!(page["page"]["status"], "draft");
}

#[tokio::test]
async fn registration_normalizes_inputs_hashes_password_and_rejects_invalid_data() {
    let mut app = TestApp::new().await;
    let (status, _, _) = app
        .post(
            "/api/auth/register",
            json!({
                "username": "Mixed_Case",
                "email": "Mixed@Example.test",
                "password": "correct-horse",
                "password_confirmation": "correct-horse"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (username, email, password_hash): (String, String, String) =
        sqlx::query_as("SELECT username, email, password_hash FROM users")
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(username, "mixed_case");
    assert_eq!(email, "mixed@example.test");
    assert!(password_hash.starts_with("$argon2"));
    assert_ne!(password_hash, "correct-horse");

    let page_slug: String =
        sqlx::query_scalar("SELECT slug FROM pages WHERE user_id = (SELECT id FROM users LIMIT 1)")
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(page_slug, "mixed_case");

    let (status, body, _) = app
        .post(
            "/api/auth/register",
            json!({
                "username": "invalid!",
                "email": "invalid@example.test",
                "password": "correct-horse",
                "password_confirmation": "correct-horse"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");

    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    let pages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pages")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(users, 1);
    assert_eq!(pages, 1);
}

#[tokio::test]
async fn authentication_forms_do_not_use_csp_blocked_inline_handlers() {
    let mut app = TestApp::new().await;
    let (_, _, register) = app.get("/register").await;
    let (_, _, login) = app.get("/login").await;
    let (_, _, auth_js) = app.get("/static/js/auth.js").await;

    assert!(register.contains("id=\"register-form\""));
    assert!(register.contains("method=\"post\""));
    assert!(!register.contains("onsubmit="));
    assert!(login.contains("id=\"login-form\""));
    assert!(login.contains("method=\"post\""));
    assert!(!login.contains("onsubmit="));
    assert!(auth_js.contains("addEventListener(\"submit\", onRegister)"));
    assert!(auth_js.contains("addEventListener(\"submit\", onLogin)"));
}

#[tokio::test]
async fn csrf_allows_same_origin_and_rejects_cross_origin_auth_mutations() {
    let mut app = TestApp::new().await;
    let body = json!({
        "username": "origin_user",
        "email": "origin@example.test",
        "password": "correct-horse",
        "password_confirmation": "correct-horse"
    });

    let (status, _, _) = app
        .send(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::HOST, "127.0.0.1:3000")
                .header(header::ORIGIN, "http://attacker.example"),
            Body::from(body.to_string()),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _, _) = app
        .send(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::HOST, "127.0.0.1:3000")
                .header(header::ORIGIN, "http://127.0.0.1"),
            Body::from(body.to_string()),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn duplicate_username_and_email_are_rejected() {
    let mut app = TestApp::new().await;
    assert_eq!(
        app.register("same_user", "a@example.com").await,
        StatusCode::OK
    );
    app.cookies.remove("linko_session");
    let (status, body, _) = app
        .post(
            "/api/auth/register",
            json!({
                "username": "same_user",
                "email": "b@example.com",
                "password": "correct-horse",
                "password_confirmation": "correct-horse"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "DUPLICATE_USERNAME");
    let (status, body, _) = app
        .post(
            "/api/auth/register",
            json!({
                "username": "other_user",
                "email": "a@example.com",
                "password": "correct-horse",
                "password_confirmation": "correct-horse"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "DUPLICATE_EMAIL");
}

#[tokio::test]
async fn login_logout_and_expired_sessions() {
    let mut app = TestApp::new().await;
    app.register("sessioned", "sess@example.com").await;
    let (status, _, _) = app.post("/api/auth/logout", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = app.get("/api/auth/me").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _, _) = app
        .post(
            "/api/auth/login",
            json!({ "identifier": "sessioned", "password": "correct-horse" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(app.cookies.contains_key("linko_session"));

    let user_id: String = sqlx::query_scalar("SELECT id FROM users WHERE username = 'sessioned'")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    let raw = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    linko::auth::session::create_expired_session_for_tests(&app.pool, &user_id, raw)
        .await
        .unwrap();
    app.cookies.insert("linko_session".into(), raw.into());
    let (status, _, _) = app.get("/api/auth/me").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unauthenticated_users_cannot_access_private_routes() {
    let mut app = TestApp::new().await;
    let (status, _, _) = app.get("/api/pages/me").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = app.get("/api/admin/stats").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn users_cannot_edit_other_users_blocks() {
    let mut app = TestApp::new().await;
    app.register("owner_user", "owner@example.com").await;
    let (status, block, _) = app
        .post(
            "/api/pages/me/blocks",
            json!({ "type": "heading", "content": { "text": "Hello", "level": 2 } }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let block_id = block["id"].as_str().unwrap().to_string();

    let mut other = TestApp {
        router: app.router.clone(),
        cookies: HashMap::new(),
        pool: app.pool.clone(),
        _dir: tempfile::tempdir().unwrap(),
    };
    let _ = other.get("/api/auth/csrf").await;
    other.register("intruder", "intruder@example.com").await;
    let (status, _, _) = other
        .patch(
            &format!("/api/pages/me/blocks/{block_id}"),
            json!({ "content": { "text": "Hacked", "level": 2 } }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = other
        .delete(&format!("/api/pages/me/blocks/{block_id}"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn blocks_persist_and_reorder() {
    let mut app = TestApp::new().await;
    app.register("blocky", "blocky@example.com").await;
    let (_, a, _) = app
        .post(
            "/api/pages/me/blocks",
            json!({ "type": "heading", "content": { "text": "One", "level": 2 } }),
        )
        .await;
    let (_, b, _) = app
        .post(
            "/api/pages/me/blocks",
            json!({ "type": "paragraph", "content": { "text": "Two" } }),
        )
        .await;
    let id_a = a["id"].as_str().unwrap();
    let id_b = b["id"].as_str().unwrap();
    let (status, ordered, _) = app
        .post(
            "/api/pages/me/blocks/reorder",
            json!({ "ids": [id_b, id_a] }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ordered[0]["id"], id_b);
    assert_eq!(ordered[1]["id"], id_a);
    let (_, listed, _) = app.get("/api/pages/me/blocks").await;
    assert_eq!(listed[0]["id"], id_b);
}

#[tokio::test]
async fn invalid_block_types_and_malformed_json_are_rejected() {
    let mut app = TestApp::new().await;
    app.register("strict", "strict@example.com").await;
    let (status, body, _) = app
        .post(
            "/api/pages/me/blocks",
            json!({ "type": "script", "content": { "text": "nope" } }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");

    let (status, body, _) = app
        .send(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json"),
            Body::from("{not-json"),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "BAD_REQUEST");
}

#[tokio::test]
async fn draft_and_unpublished_pages_are_hidden() {
    let mut app = TestApp::new().await;
    app.register("publicish", "pub@example.com").await;
    let (status, _, _) = app.get("/api/public/pages/publicish").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, _, html) = app.get("/publicish").await;
    assert!(html.contains("منتشر نشده") || html.contains("وجود ندارد"));

    let (status, _, _) = app.post("/api/pages/me/publish", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, page, _) = app.get("/api/public/pages/publicish").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["username"], "publicish");
    assert!(page.get("email").is_none());
    assert!(page.get("password_hash").is_none());

    let (status, _, _) = app.post("/api/pages/me/unpublish", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = app.get("/api/public/pages/publicish").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn published_profile_renders_each_theme_and_profile_media() {
    let mut app = TestApp::new().await;
    assert_eq!(
        app.register("profile_theme", "theme@example.com").await,
        StatusCode::OK
    );

    let (status, _, _) = app
        .patch(
            "/api/pages/me",
            json!({
                "cover_url": "https://example.com/cover.jpg",
                "accent_color": "#6B9B78"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = app.post("/api/pages/me/publish", json!({})).await;
    assert_eq!(status, StatusCode::OK);

    for theme in ["minimal", "developer", "creator"] {
        let (status, _, _) = app.patch("/api/pages/me", json!({ "theme": theme })).await;
        assert_eq!(status, StatusCode::OK);

        let (status, _, html) = app.get("/profile_theme").await;
        assert_eq!(status, StatusCode::OK);
        assert!(html.contains(&format!("class=\"public-body theme-{theme}\"")));
        assert!(html.contains("class=\"public-cover\""));
        assert!(
            html.contains("cover.jpg"),
            "Cover image markup missing from profile response: {html}"
        );
        assert!(html.contains("class=\"avatar avatar-fallback\""));
        assert!(html.contains("class=\"linko-signature\""));
        assert!(html.contains("/static/images/linko-icon.png"));
        assert!(html.contains("/static/images/linko-logo-light.png"));
        assert!(html.contains("/static/images/linko-logo.png"));
    }
}

#[tokio::test]
async fn brand_logo_assets_are_served() {
    let mut app = TestApp::new().await;
    for asset in [
        "/static/images/linko-icon.png",
        "/static/images/linko-logo.png",
        "/static/images/linko-logo-light.png",
    ] {
        let (status, _, body) = app.get(asset).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!body.is_empty());
    }
}

#[tokio::test]
async fn non_admin_cannot_access_admin_and_admin_writes_audit() {
    let mut app = TestApp::new().await;
    app.register("normal_user", "normal@example.com").await;
    let (status, _, _) = app.get("/api/admin/stats").await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    sqlx::query("UPDATE users SET role = 'admin' WHERE username = 'normal_user'")
        .execute(&app.pool)
        .await
        .unwrap();
    app.register("target_user", "target@example.com").await;
    // switch back to admin session: login as normal_user again
    let (status, _, _) = app
        .post(
            "/api/auth/login",
            json!({ "identifier": "normal_user", "password": "correct-horse" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let target_id: String =
        sqlx::query_scalar("SELECT id FROM users WHERE username = 'target_user'")
            .fetch_one(&app.pool)
            .await
            .unwrap();
    let (status, _, _) = app
        .patch(
            &format!("/api/admin/users/{target_id}/status"),
            json!({ "status": "disabled" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE action = 'user.status'")
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert!(count >= 1);

    let mut victim = TestApp {
        router: app.router.clone(),
        cookies: HashMap::new(),
        pool: app.pool.clone(),
        _dir: tempfile::tempdir().unwrap(),
    };
    let _ = victim.get("/api/auth/csrf").await;
    let (status, _, _) = victim
        .post(
            "/api/auth/login",
            json!({ "identifier": "target_user", "password": "correct-horse" }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn invalid_login_is_generic() {
    let mut app = TestApp::new().await;
    app.register("real_user", "real@example.com").await;
    app.post("/api/auth/logout", json!({})).await;
    let (status, body, _) = app
        .post(
            "/api/auth/login",
            json!({ "identifier": "real_user", "password": "wrong-password-value" }),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body["error"]["message"],
        "Invalid username, email, or password."
    );
}
