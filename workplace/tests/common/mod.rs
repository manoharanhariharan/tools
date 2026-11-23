#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header};
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

use daybook::config::Config;
use daybook::db;
use daybook::{AppState, build_router};

pub const TEST_PASSWORD: &str = "test-password-123";

/// Seeded earning types from migration 0003.
pub const SALES_TYPE: &str = "00000000-0000-4000-8000-000000000101";
pub const SERVICE_TYPE: &str = "00000000-0000-4000-8000-000000000102";
pub const TIPS_TYPE: &str = "00000000-0000-4000-8000-000000000103";

/// Build a `lines` array for an entry request.
pub fn lines(rows: &[(&str, &str, i64)]) -> Value {
    Value::Array(
        rows.iter()
            .map(|(type_id, description, amount_minor)| {
                serde_json::json!({
                    "type_id": type_id,
                    "description": description,
                    "amount_minor": amount_minor,
                })
            })
            .collect(),
    )
}

pub struct TestServer {
    pub app: Router,
    pub state: AppState,
    _dir: TempDir,
}

pub struct TestResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl TestResponse {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|err| {
            panic!(
                "response is not JSON ({err}): {}",
                String::from_utf8_lossy(&self.body)
            )
        })
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn error_code(&self) -> Option<String> {
        self.json()
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(Value::as_str)
            .map(str::to_string)
    }
}

impl TestServer {
    pub async fn start() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = Arc::new(Config {
            bind: "127.0.0.1:0".parse().unwrap(),
            data_dir: dir.path().to_path_buf(),
            tls: None,
            session_ttl_secs: 3_600,
            pdf_font: None,
            web_dir: None,
        });

        let init = db::open(&config.data_dir).await.expect("open database");
        db::set_admin_password(&init.pool, TEST_PASSWORD)
            .await
            .expect("set admin password");

        let state = AppState::new(init.pool.clone(), config);
        let app = build_router(state.clone());

        Self {
            app,
            state,
            _dir: dir,
        }
    }

    pub async fn call(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        cookie: Option<&str>,
    ) -> TestResponse {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("x-daybook-client", "daybook-test");

        if body.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, cookie);
        }

        let request = builder
            .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40_000))))
            .body(match body {
                Some(value) => Body::from(value.to_string()),
                None => Body::empty(),
            })
            .expect("build request");

        let response = self
            .app
            .clone()
            .oneshot(request)
            .await
            .expect("router response");
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .expect("read body");
        TestResponse {
            status,
            headers,
            body: bytes.to_vec(),
        }
    }

    /// Call a mutating endpoint without the CSRF client header.
    pub async fn call_without_client_header(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        cookie: Option<&str>,
    ) -> TestResponse {
        let mut builder = Request::builder().method(method).uri(uri);
        if body.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, cookie);
        }
        let request = builder
            .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40_000))))
            .body(match body {
                Some(value) => Body::from(value.to_string()),
                None => Body::empty(),
            })
            .expect("build request");
        let response = self.app.clone().oneshot(request).await.expect("response");
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .expect("read body");
        TestResponse {
            status,
            headers,
            body: bytes.to_vec(),
        }
    }

    pub async fn login(&self) -> String {
        let response = self
            .call(
                Method::POST,
                "/api/auth/login",
                Some(serde_json::json!({ "password": TEST_PASSWORD })),
                None,
            )
            .await;
        assert_eq!(
            response.status,
            StatusCode::OK,
            "login failed: {}",
            response.text()
        );
        response
            .headers
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .map(str::to_string)
            .expect("session cookie")
    }

    pub async fn employee(&self, name: &str) -> (String, Value) {
        self.employee_with_code(name, None).await
    }

    pub async fn employee_with_code(&self, name: &str, code: Option<&str>) -> (String, Value) {
        let cookie = self.login().await;
        let response = self
            .call(
                Method::POST,
                "/api/employees",
                Some(serde_json::json!({ "name": name, "code": code })),
                Some(&cookie),
            )
            .await;
        assert_eq!(
            response.status,
            StatusCode::CREATED,
            "employee create failed: {}",
            response.text()
        );
        (cookie, response.json())
    }
}

pub fn cookie_header(value: &str) -> HeaderValue {
    HeaderValue::from_str(value).expect("cookie value")
}
