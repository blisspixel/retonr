use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct VersionResponse {
    pub(crate) version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct TagsResponse {
    pub(crate) models: Vec<TagModel>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct TagModel {
    pub(crate) name: String,
    pub(crate) model: String,
    pub(crate) size: u64,
    pub(crate) digest: String,
    #[serde(default)]
    pub(crate) remote_model: String,
    #[serde(default)]
    pub(crate) remote_host: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PsResponse {
    pub(crate) models: Vec<RunningModel>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RunningModel {
    pub(crate) name: String,
    pub(crate) model: String,
    pub(crate) size: u64,
    pub(crate) digest: String,
    pub(crate) size_vram: u64,
    pub(crate) context_length: u32,
    #[serde(default)]
    pub(crate) remote_model: String,
    #[serde(default)]
    pub(crate) remote_host: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ShowRequest<'a> {
    pub(crate) model: &'a str,
    pub(crate) verbose: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ShowResponse {
    #[serde(default)]
    pub(crate) remote_model: String,
    #[serde(default)]
    pub(crate) remote_host: String,
    #[serde(default)]
    pub(crate) license: String,
    #[serde(default)]
    pub(crate) template: String,
    #[serde(default)]
    pub(crate) capabilities: Vec<String>,
    pub(crate) details: ShowDetails,
    #[serde(default)]
    pub(crate) model_info: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ShowDetails {
    pub(crate) format: String,
    pub(crate) family: String,
    #[serde(default)]
    pub(crate) quantization_level: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct GenerateRequest<'a> {
    pub(crate) model: &'a str,
    pub(crate) prompt: &'a str,
    pub(crate) stream: bool,
    pub(crate) format: serde_json::Value,
    pub(crate) think: bool,
    pub(crate) raw: bool,
    pub(crate) options: GenerateOptions,
}

#[derive(Debug, Serialize)]
pub(crate) struct GenerateOptions {
    pub(crate) temperature: f32,
    pub(crate) top_p: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) seed: Option<u64>,
    pub(crate) num_ctx: u32,
    pub(crate) num_predict: u32,
    pub(crate) stop: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GenerateResponse {
    pub(crate) model: String,
    #[serde(default)]
    pub(crate) remote_model: String,
    #[serde(default)]
    pub(crate) remote_host: String,
    pub(crate) response: String,
    #[serde(default)]
    pub(crate) thinking: String,
    pub(crate) done: bool,
    #[serde(default)]
    pub(crate) done_reason: String,
    pub(crate) prompt_eval_count: Option<u64>,
    pub(crate) eval_count: Option<u64>,
    pub(crate) eval_duration: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateEnvelope {
    pub(crate) candidates: Vec<CandidateItem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateItem {
    pub(crate) text: String,
}
