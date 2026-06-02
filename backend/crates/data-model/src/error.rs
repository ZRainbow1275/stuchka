//! API envelope, error type, and the error-code table (backend/01 §1.1.2 + §1.12).
//!
//! `crates/types` was merged into data-model (D6), so the IPC contract types and the
//! error-code constants live here. Each code carries a Chinese user-facing message template.
//!
//! Code count: backend/01 §1.12 enumerates exactly **16** authoritative codes. `E_NOT_IMPLEMENTED`
//! (HTTP 501) is a **skeleton-phase authorized extension** beyond those 16 — it is required by the
//! R-phase route skeleton (handlers whose contract-accurate signature is registered but whose
//! module logic is not yet implemented). It is intentional and sanctioned, not a spec violation;
//! `ALL_ERROR_CODES` therefore contains 16 spec codes + this one = 17. (This resolves the earlier
//! self-contradictory "17 spec codes" wording.)

use serde::{Deserialize, Serialize};

/// Uniform IPC response envelope (backend/01 §1.1.2).
///
/// Success → `{ data: Some(_), error: None }`; failure → `{ data: None, error: Some(_) }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEnvelope<T> {
    pub data: Option<T>,
    pub error: Option<ApiError>,
    /// Injected by tracing.
    pub trace_id: String,
}

impl<T> ApiEnvelope<T> {
    /// Build a success envelope.
    pub fn ok(data: T, trace_id: impl Into<String>) -> Self {
        Self {
            data: Some(data),
            error: None,
            trace_id: trace_id.into(),
        }
    }

    /// Build a failure envelope (data is dropped to `None`).
    pub fn err(error: ApiError, trace_id: impl Into<String>) -> Self {
        Self {
            data: None,
            error: Some(error),
            trace_id: trace_id.into(),
        }
    }
}

/// Structured API error (backend/01 §1.1.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    /// One of the §1.12 codes (see the `code` consts on [`ErrorCode`]).
    pub code: String,
    /// User-facing Chinese message.
    pub message: String,
    pub detail: serde_json::Value,
    pub hint: Option<String>,
}

impl ApiError {
    /// Build an `ApiError` from a known [`ErrorCode`] using its default Chinese message.
    pub fn from_code(code: ErrorCode) -> Self {
        Self {
            code: code.code().to_string(),
            message: code.message().to_string(),
            detail: serde_json::Value::Null,
            hint: None,
        }
    }

    /// Attach a structured detail payload.
    pub fn with_detail(mut self, detail: serde_json::Value) -> Self {
        self.detail = detail;
        self
    }

    /// Attach a user-facing hint.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

/// The error-code enum: backend/01 §1.12's 16 codes + `E_NOT_IMPLEMENTED` (skeleton-phase
/// authorized extension; see the module doc). Each variant maps to a stable `code` string, an HTTP
/// status, and a Chinese message template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ErrorCode {
    #[error("E_INVALID_TOKEN")]
    InvalidToken,
    #[error("E_BAD_REQUEST")]
    BadRequest,
    #[error("E_NOT_FOUND")]
    NotFound,
    #[error("E_INVALID_TRANSITION")]
    InvalidTransition,
    #[error("E_CONCURRENT_MOD")]
    ConcurrentMod,
    #[error("E_PII_BLOCKED")]
    PiiBlocked,
    #[error("E_KB_OUTDATED")]
    KbOutdated,
    #[error("E_LLM_ABSTENTION")]
    LlmAbstention,
    #[error("E_LLM_PROVIDER_DOWN")]
    LlmProviderDown,
    #[error("E_RULE_NO_COVERAGE")]
    RuleNoCoverage,
    #[error("E_FS_UNENCRYPTED")]
    FsUnencrypted,
    #[error("E_PATCH_INVALID")]
    PatchInvalid,
    #[error("E_PATCH_AUTHORIZATION")]
    PatchAuthorization,
    #[error("E_OTS_UPSTREAM")]
    OtsUpstream,
    #[error("E_AUDIT_CHAIN_BROKEN")]
    AuditChainBroken,
    /// Route is registered with a contract-accurate signature but its module logic is
    /// not yet implemented (R-phase skeleton); handler returns HTTP 501.
    #[error("E_NOT_IMPLEMENTED")]
    NotImplemented,
    #[error("E_INTERNAL")]
    Internal,
}

impl ErrorCode {
    /// Stable code string written to `ApiError.code` (backend/01 §1.12).
    pub const fn code(self) -> &'static str {
        match self {
            ErrorCode::InvalidToken => "E_INVALID_TOKEN",
            ErrorCode::BadRequest => "E_BAD_REQUEST",
            ErrorCode::NotFound => "E_NOT_FOUND",
            ErrorCode::InvalidTransition => "E_INVALID_TRANSITION",
            ErrorCode::ConcurrentMod => "E_CONCURRENT_MOD",
            ErrorCode::PiiBlocked => "E_PII_BLOCKED",
            ErrorCode::KbOutdated => "E_KB_OUTDATED",
            ErrorCode::LlmAbstention => "E_LLM_ABSTENTION",
            ErrorCode::LlmProviderDown => "E_LLM_PROVIDER_DOWN",
            ErrorCode::RuleNoCoverage => "E_RULE_NO_COVERAGE",
            ErrorCode::FsUnencrypted => "E_FS_UNENCRYPTED",
            ErrorCode::PatchInvalid => "E_PATCH_INVALID",
            ErrorCode::PatchAuthorization => "E_PATCH_AUTHORIZATION",
            ErrorCode::OtsUpstream => "E_OTS_UPSTREAM",
            ErrorCode::AuditChainBroken => "E_AUDIT_CHAIN_BROKEN",
            ErrorCode::NotImplemented => "E_NOT_IMPLEMENTED",
            ErrorCode::Internal => "E_INTERNAL",
        }
    }

    /// Suggested HTTP status (backend/01 §1.12).
    pub const fn http_status(self) -> u16 {
        match self {
            ErrorCode::InvalidToken => 401,
            ErrorCode::BadRequest => 400,
            ErrorCode::NotFound => 404,
            ErrorCode::InvalidTransition => 409,
            ErrorCode::ConcurrentMod => 409,
            ErrorCode::PiiBlocked => 422,
            ErrorCode::KbOutdated => 422,
            ErrorCode::LlmAbstention => 422,
            ErrorCode::LlmProviderDown => 503,
            ErrorCode::RuleNoCoverage => 422,
            ErrorCode::FsUnencrypted => 412,
            ErrorCode::PatchInvalid => 400,
            ErrorCode::PatchAuthorization => 403,
            ErrorCode::OtsUpstream => 502,
            ErrorCode::AuditChainBroken => 500,
            ErrorCode::NotImplemented => 501,
            ErrorCode::Internal => 500,
        }
    }

    /// Chinese user-facing message template (backend/01 §1.12 触发场景).
    pub const fn message(self) -> &'static str {
        match self {
            ErrorCode::InvalidToken => "鉴权失败：本地通信令牌无效或缺失",
            ErrorCode::BadRequest => "请求参数格式错误，无法解析",
            ErrorCode::NotFound => "未找到对应记录",
            ErrorCode::InvalidTransition => "非法的状态迁移",
            ErrorCode::ConcurrentMod => "并发修改冲突，请刷新后重试",
            ErrorCode::PiiBlocked => "检测到高敏信息，且未确认本地处理，已拦截",
            ErrorCode::KbOutdated => "知识库已超过 30 天，请先更新知识库",
            ErrorCode::LlmAbstention => "无足够法源支撑，系统选择拒答（覆盖度未知）",
            ErrorCode::LlmProviderDown => "所有 AI 服务提供方均不可用",
            ErrorCode::RuleNoCoverage => "规则引擎无对应覆盖，无法计算",
            ErrorCode::FsUnencrypted => "文件系统未启用加密，且未声明知悉风险",
            ErrorCode::PatchInvalid => "补丁文件解析失败",
            ErrorCode::PatchAuthorization => "授权链不完整，拒绝导入",
            ErrorCode::OtsUpstream => "时间戳锚定上游失败，已降级写入待重试队列",
            ErrorCode::AuditChainBroken => "审计日志哈希链校验失败",
            ErrorCode::NotImplemented => "该功能尚未实装（路由已就绪，模块逻辑待补完）",
            ErrorCode::Internal => "系统内部错误，请凭 trace_id 联系排查",
        }
    }
}

impl From<ErrorCode> for ApiError {
    fn from(code: ErrorCode) -> Self {
        ApiError::from_code(code)
    }
}

/// All codes for exhaustiveness assertions / iteration.
///
/// backend/01 §1.12 lists exactly **16** authoritative codes; `E_NOT_IMPLEMENTED` (HTTP 501) is the
/// skeleton-phase authorized extension (see module doc) for the R-phase route skeleton, so the
/// array length is 16 + 1 = 17.
pub const ALL_ERROR_CODES: [ErrorCode; 17] = [
    ErrorCode::InvalidToken,
    ErrorCode::BadRequest,
    ErrorCode::NotFound,
    ErrorCode::InvalidTransition,
    ErrorCode::ConcurrentMod,
    ErrorCode::PiiBlocked,
    ErrorCode::KbOutdated,
    ErrorCode::LlmAbstention,
    ErrorCode::LlmProviderDown,
    ErrorCode::RuleNoCoverage,
    ErrorCode::FsUnencrypted,
    ErrorCode::PatchInvalid,
    ErrorCode::PatchAuthorization,
    ErrorCode::OtsUpstream,
    ErrorCode::AuditChainBroken,
    ErrorCode::NotImplemented,
    ErrorCode::Internal,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_codes_present() {
        // backend/01 §1.12 lists 16 authoritative codes; +1 E_NOT_IMPLEMENTED (skeleton-phase
        // authorized extension) for the R-phase route skeleton.
        assert_eq!(
            ALL_ERROR_CODES.len(),
            17,
            "16 spec codes + E_NOT_IMPLEMENTED (authorized R-phase extension)"
        );
    }

    #[test]
    fn not_implemented_is_501() {
        assert_eq!(ErrorCode::NotImplemented.code(), "E_NOT_IMPLEMENTED");
        assert_eq!(ErrorCode::NotImplemented.http_status(), 501);
    }

    #[test]
    fn every_code_has_code_string_and_chinese_message() {
        for c in ALL_ERROR_CODES {
            assert!(
                c.code().starts_with("E_"),
                "code must start with E_: {}",
                c.code()
            );
            assert!(!c.message().is_empty(), "{} must have a message", c.code());
            // message template must contain CJK characters (Chinese)
            assert!(
                c.message()
                    .chars()
                    .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch)),
                "{} message must be Chinese",
                c.code()
            );
            assert!(
                (400..=599).contains(&c.http_status()),
                "{} http status range",
                c.code()
            );
        }
    }

    #[test]
    fn codes_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for c in ALL_ERROR_CODES {
            assert!(seen.insert(c.code()), "duplicate code {}", c.code());
        }
    }

    #[test]
    fn known_http_statuses_match_spec() {
        assert_eq!(ErrorCode::InvalidToken.http_status(), 401);
        assert_eq!(ErrorCode::BadRequest.http_status(), 400);
        assert_eq!(ErrorCode::InvalidTransition.http_status(), 409);
        assert_eq!(ErrorCode::LlmProviderDown.http_status(), 503);
        assert_eq!(ErrorCode::FsUnencrypted.http_status(), 412);
        assert_eq!(ErrorCode::OtsUpstream.http_status(), 502);
        assert_eq!(ErrorCode::AuditChainBroken.http_status(), 500);
    }

    #[test]
    fn api_envelope_ok_and_err_shapes() {
        let ok: ApiEnvelope<u32> = ApiEnvelope::ok(7, "trace-1");
        let j = serde_json::to_string(&ok).unwrap();
        assert!(j.contains("\"data\":7"), "got {j}");
        assert!(j.contains("\"error\":null"), "got {j}");
        assert!(j.contains("\"traceId\":\"trace-1\""), "got {j}");

        let err: ApiEnvelope<u32> =
            ApiEnvelope::err(ApiError::from_code(ErrorCode::NotFound), "trace-2");
        let je = serde_json::to_string(&err).unwrap();
        assert!(je.contains("\"data\":null"), "got {je}");
        assert!(je.contains("\"code\":\"E_NOT_FOUND\""), "got {je}");
    }

    #[test]
    fn api_error_builder() {
        let e = ApiError::from_code(ErrorCode::InvalidTransition)
            .with_detail(serde_json::json!({"from": "draft", "to": "frozen"}))
            .with_hint("先 diagnose 再 confirm");
        assert_eq!(e.code, "E_INVALID_TRANSITION");
        assert_eq!(e.hint.as_deref(), Some("先 diagnose 再 confirm"));
        assert_eq!(e.detail["from"], "draft");
    }
}
