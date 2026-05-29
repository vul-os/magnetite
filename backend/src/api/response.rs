use axum::Json;
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ApiError>,
}

#[derive(Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub details: Option<Value>,
}

#[derive(Serialize)]
pub struct PaginatedResponse<T> {
    pub success: bool,
    pub data: Vec<T>,
    pub pagination: PaginationInfo,
}

#[derive(Serialize)]
pub struct PaginationInfo {
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub total_pages: u32,
}

pub fn success_response<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        success: true,
        data: Some(data),
        error: None,
    })
}

pub fn error_response(code: &str, message: &str) -> Json<ApiResponse<()>> {
    Json(ApiResponse {
        success: false,
        data: None,
        error: Some(ApiError {
            code: code.to_string(),
            message: message.to_string(),
            details: None,
        }),
    })
}

pub fn paginated<T: Serialize>(data: Vec<T>, page: u32, per_page: u32, total: u64) -> Json<PaginatedResponse<T>> {
    let total_pages = if per_page > 0 {
        ((total as f64) / (per_page as f64)).ceil() as u32
    } else {
        0
    };

    Json(PaginatedResponse {
        success: true,
        data,
        pagination: PaginationInfo {
            page,
            per_page,
            total,
            total_pages,
        },
    })
}
