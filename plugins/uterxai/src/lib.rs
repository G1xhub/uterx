//! UterxAI plugin scaffold.
//!
//! This crate intentionally starts as a minimal plugin surface so runtime
//! integration can be added incrementally.

pub mod config;

use serde_json::Value;
use uterx_fs::read_file;

const AI_STREAM_ID: i64 = 1;
const CHUNK_SIZE: usize = 64;
const RESPONSE_BUF_SIZE: usize = 128 * 1024;
const KEYRING_BUF_SIZE: usize = 1024;
const DEFAULT_ZAI_URL: &str = "https://api.z.ai/v1/chat/completions";
const DEFAULT_ZAI_MODEL: &str = "glm-4.5-air";
const PROVIDER_SETTINGS_PATH: &str = "uterxai/provider.toml";

#[link(wasm_import_module = "uterx_io")]
unsafe extern "C" {
	fn read(stream_id: i64, buf_ptr: i32, buf_len: i32) -> i32;
	fn write(stream_id: i64, data_ptr: i32, data_len: i32) -> i32;
}

#[link(wasm_import_module = "uterx_net")]
unsafe extern "C" {
	fn http_post(
		url_ptr: i32,
		url_len: i32,
		headers_ptr: i32,
		headers_len: i32,
		body_ptr: i32,
		body_len: i32,
		response_ptr: i32,
		response_len: i32,
	) -> i32;
}

#[link(wasm_import_module = "uterx_platform")]
unsafe extern "C" {
	fn keyring_get(key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32) -> i32;
}

mod uterx_fs {
	#[link(wasm_import_module = "uterx_fs")]
	unsafe extern "C" {
		fn host_read_file(
			path_ptr: i32,
			path_len: i32,
			buf_ptr: i32,
			buf_len: i32,
		) -> i32;
	}

	pub fn read_file(path: &str) -> Option<Vec<u8>> {
		let mut out = vec![0_u8; 16 * 1024];
		let read_len = unsafe {
			host_read_file(
				path.as_ptr() as i32,
				path.len() as i32,
				out.as_mut_ptr() as i32,
				out.len() as i32,
			)
		};
		if read_len <= 0 {
			return None;
		}
		out.truncate(read_len as usize);
		Some(out)
	}
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() {}

#[unsafe(no_mangle)]
pub extern "C" fn uterxai_poll() -> i32 {
	let mut input_buf = [0_u8; 4096];
	let read_len = unsafe {
		read(
			AI_STREAM_ID,
			input_buf.as_mut_ptr() as i32,
			input_buf.len() as i32,
		)
	};

	if read_len <= 0 {
		return read_len;
	}

	let prompt = String::from_utf8_lossy(&input_buf[..read_len as usize]).trim().to_string();
	if prompt.is_empty() {
		return 0;
	}

	match zai_chat_completion(&prompt) {
		Ok(answer) => write_stream_chunks(answer.as_bytes()),
		Err(message) => write_stream_chunks(format!("UterxAI error: {message}\n").as_bytes()),
	}
}

fn zai_chat_completion(prompt: &str) -> Result<String, String> {
	let settings = read_provider_settings();
	let api_key = settings
		.as_ref()
		.and_then(|s| s.api_key.clone())
		.filter(|s| !s.trim().is_empty())
		.or_else(read_api_key)
		.ok_or_else(|| {
		"missing API key. set ZAI_API_KEY or keyring entry 'zai_api_key'".to_string()
	})?;

	let url = settings
		.as_ref()
		.and_then(|s| s.api_url.clone())
		.filter(|s| !s.trim().is_empty())
		.or_else(|| std::env::var("UTERXAI_ZAI_URL").ok())
		.unwrap_or_else(|| DEFAULT_ZAI_URL.to_string());
	let model = settings
		.as_ref()
		.and_then(|s| s.api_model.clone())
		.filter(|s| !s.trim().is_empty())
		.or_else(|| std::env::var("UTERXAI_ZAI_MODEL").ok())
		.unwrap_or_else(|| DEFAULT_ZAI_MODEL.to_string());

	let body = serde_json::json!({
		"model": model,
		"messages": [
			{
				"role": "user",
				"content": prompt,
			}
		],
		"stream": false,
	});
	let body_str = serde_json::to_string(&body).map_err(|e| format!("serialize request: {e}"))?;

	let headers = format!(
		"Authorization: Bearer {}\nContent-Type: application/json\n",
		api_key
	);

	let mut response_buf = vec![0_u8; RESPONSE_BUF_SIZE];
	let status = unsafe {
		http_post(
			url.as_ptr() as i32,
			url.len() as i32,
			headers.as_ptr() as i32,
			headers.len() as i32,
			body_str.as_ptr() as i32,
			body_str.len() as i32,
			response_buf.as_mut_ptr() as i32,
			response_buf.len() as i32,
		)
	};

	if status < 200 || status >= 300 {
		let snippet = String::from_utf8_lossy(&response_buf)
			.trim_matches(char::from(0))
			.chars()
			.take(240)
			.collect::<String>();
		return Err(format!("z.ai returned HTTP {status}: {snippet}"));
	}

	let response_text = String::from_utf8_lossy(&response_buf)
		.trim_matches(char::from(0))
		.to_string();
	let json: Value = serde_json::from_str(&response_text)
		.map_err(|e| format!("parse provider response: {e}"))?;

	let content = extract_chat_content(&json)
		.ok_or_else(|| "provider response missing content".to_string())?;
	Ok(format!("z.ai> {content}\n"))
}

fn read_api_key() -> Option<String> {
	if let Ok(value) = std::env::var("ZAI_API_KEY") {
		let trimmed = value.trim();
		if !trimmed.is_empty() {
			return Some(trimmed.to_string());
		}
	}

	let key_name = "zai_api_key";
	let mut out = [0_u8; KEYRING_BUF_SIZE];
	let len = unsafe {
		keyring_get(
			key_name.as_ptr() as i32,
			key_name.len() as i32,
			out.as_mut_ptr() as i32,
			out.len() as i32,
		)
	};
	if len <= 0 {
		return None;
	}

	let bytes = &out[..len as usize];
	let value = String::from_utf8_lossy(bytes).trim().to_string();
	if value.is_empty() {
		None
	} else {
		Some(value)
	}
}

#[derive(Debug, serde::Deserialize)]
struct ProviderSettings {
	api_key: Option<String>,
	api_url: Option<String>,
	api_model: Option<String>,
}

fn read_provider_settings() -> Option<ProviderSettings> {
	let bytes = read_file(PROVIDER_SETTINGS_PATH)?;
	let text = String::from_utf8(bytes).ok()?;
	toml::from_str::<ProviderSettings>(&text).ok()
}

fn write_stream_chunks(bytes: &[u8]) -> i32 {
	let mut total_written = 0_i32;
	for chunk in bytes.chunks(CHUNK_SIZE) {
		let wrote = unsafe { write(AI_STREAM_ID, chunk.as_ptr() as i32, chunk.len() as i32) };
		if wrote <= 0 {
			break;
		}
		total_written += wrote;
		if wrote as usize != chunk.len() {
			break;
		}
	}
	total_written
}

fn extract_chat_content(json: &Value) -> Option<String> {
	let choice = json.get("choices")?.as_array()?.first()?;
	let message = choice.get("message")?;
	let content = message.get("content")?;

	if let Some(text) = content.as_str() {
		return Some(text.to_string());
	}

	if let Some(parts) = content.as_array() {
		let merged = parts
			.iter()
			.filter_map(|part| part.get("text").and_then(Value::as_str))
			.collect::<Vec<_>>()
			.join("");
		if !merged.is_empty() {
			return Some(merged);
		}
	}

	None
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_string_content() {
		let payload = serde_json::json!({
			"choices": [
				{
					"message": {
						"content": "hello from zai"
					}
				}
			]
		});

		assert_eq!(extract_chat_content(&payload).as_deref(), Some("hello from zai"));
	}
}
