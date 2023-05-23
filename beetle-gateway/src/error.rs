use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

use axum::{
    body::BoxBody,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use beetle_metrics::{core::MRecorder, gateway::GatewayMetrics, get_current_trace_id, inc};
use http::{HeaderMap, HeaderValue};
use opentelemetry::trace::TraceId;
use serde_json::json;

use crate::constants::HEADER_X_TRACE_ID;

#[derive(Debug, Clone)]
pub struct GatewayError {
    pub status_code: StatusCode,
    pub message: String,
    pub trace_id: TraceId,
    pub method: Option<http::Method>,
    pub accept_html: bool,
}

impl GatewayError {
    #[tracing::instrument()]
    pub fn new(status_code: StatusCode, message: &str) -> GatewayError {
        inc!(GatewayMetrics::ErrorCount);
        GatewayError {
            status_code,
            message: message.to_string(),
            trace_id: get_current_trace_id(),
            method: None,
            accept_html: false,
        }
    }

    pub fn with_method(self, method: http::Method) -> Self {
        Self {
            method: Some(method),
            ..self
        }
    }

    pub fn with_html(self) -> Self {
        Self {
            accept_html: true,
            ..self
        }
    }
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let mut headers = HeaderMap::new();
        if self.trace_id != TraceId::INVALID {
            headers.insert(
                &HEADER_X_TRACE_ID,
                HeaderValue::from_str(&self.trace_id.to_string()).unwrap(),
            );
        }
        match self.method {
            Some(http::Method::HEAD) => {
                let mut rb = Response::builder().status(self.status_code);
                let rh = rb.headers_mut().unwrap();
                rh.extend(headers);
                rb.body(BoxBody::default()).unwrap()
            }
            _ => {
                let body = if self.trace_id != TraceId::INVALID {
                    axum::Json(json!({
                        "code": self.status_code.as_u16(),
                        "success": false,
                        "message": self.message,
                        "trace_id": self.trace_id.to_string(),
                    }))
                } else {
                    axum::Json(json!({
                        "code": self.status_code.as_u16(),
                        "success": false,
                        "message": self.message,
                    }))
                };
                let mut res = body.into_response();
                if self.accept_html && self.status_code == StatusCode::NOT_FOUND {
                    let body = crate::templates::NOT_FOUND_TEMPLATE;
                    res = body.into_response();
                    res.headers_mut().insert(
                        http::header::CONTENT_TYPE,
                        http::header::HeaderValue::from_static("text/html"),
                    );
                }
                res.headers_mut().extend(headers);
                let status = res.status_mut();
                *status = self.status_code;
                res
            }
        }
    }
}

impl Display for GatewayError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "gateway_error({}): {})",
            &self.status_code, &self.message
        )
    }
}

impl Error for GatewayError {}
