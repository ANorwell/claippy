use std::{error::Error, fmt::Debug, time::Instant};

use aws_sdk_bedrockruntime::{
    error::SdkError,
    primitives::Blob,
    types::{error::ResponseStreamError, PayloadPart, ResponseStream},
    Client,
};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;

use crate::model::{Message, MessageRefs, Result, ResultIterator};

#[derive(Serialize)]
struct ReqBody<'a> {
    anthropic_version: &'static str,
    max_tokens: i32,
    temperature: f32,
    system: &'static str,
    messages: Vec<&'a Message>,
}

/// Queryable provides the interface that any LLM being queried should implement.
pub trait Queryable {
    fn generate(self, query: MessageRefs) -> ResultIterator<Result<String>>;
}

pub struct BedrockConfig {
    pub model_id: &'static str,
    pub system_prompt: &'static str,
    pub temperature: f32,
    pub region: &'static str,
    pub aws_profile_name: &'static str,
}

/// Bedrock implementation of Queryable.
/// The aws client uses async/tokio, and so the associated runtime is for use (`block_on`) with the client.
///
pub struct Bedrock {
    pub model_config: BedrockConfig,
    pub runtime: Runtime,
    pub client: Client,
}

impl Bedrock {
    pub fn create(model_config: BedrockConfig) -> Result<Self> {
        let runtime = Runtime::new()?;
        let start = Instant::now();
        let config = runtime.block_on(
            aws_config::from_env()
                .region(model_config.region)
                .profile_name(model_config.aws_profile_name)
                .load(),
        );
        log::info!("Load aws cfg: {:?}ms", (Instant::now() - start).as_millis());
        let client = aws_sdk_bedrockruntime::Client::new(&config);
        Ok(Bedrock {
            model_config,
            runtime,
            client,
        })
    }
}

impl Queryable for Bedrock {
    fn generate(self, query: MessageRefs) -> ResultIterator<Result<String>> {
        let body_str = serde_json::to_string(&ReqBody {
            anthropic_version: "bedrock-2023-05-31",
            max_tokens: 4096, // the maximum
            temperature: self.model_config.temperature,
            system: self.model_config.system_prompt,
            messages: query.messages,
        })?;

        log::info!("Request Body: {body_str:?}");

        let body = body_str.into_bytes();

        let async_request = self
            .client
            .invoke_model_with_response_stream()
            .model_id(self.model_config.model_id)
            .body(Blob::new(body))
            .send();

        log::info!("Starting request:");
        let response = self.runtime.block_on(async_request)?;
        log::info!("Response: {:?}", response.content_type);
        let mut event_receiver = response.body;
        let iter = std::iter::from_fn(move || {
            convert_to_option(self.runtime.block_on(event_receiver.recv()))
        })
        .map(|item| item.and_then(|chunk| parse_claude_api_text(chunk)))
        .filter_map(|result| match result {
            Ok(None) => None,
            Ok(Some(string)) => Some(Ok(string)),
            Err(e) => Some(Err(e)),
        });

        Ok(Box::new(iter))
    }
}

#[derive(Deserialize)]
struct RspText {
    text: Option<String>,
}

#[derive(Deserialize)]
struct RspChunk {
    r#type: String,
    delta: Option<RspText>,
}

// Parse the response chunks and extract the text. Ensure we don't fail on parsing, but discard chunks that don't have text
/// e.g.s:
/// Ok("{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}")
/// Ok("{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}")
/// Ok("{\"type\":\"content_block_stop\",\"index\":0}")
/// Ok("{\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":12}}")
/// Ok("{\"type\":\"message_stop\",\"amazon-bedrock-invocationMetrics\":{ ... }})
fn parse_claude_api_text(chunk_text: String) -> Result<Option<String>> {
    // This copies the part of the response chunks that we want to extract. Can we avoid this copy?

    log::debug!("Input: {chunk_text:?}");

    match serde_json::from_str(&chunk_text)? {
        RspChunk {
            r#type,
            delta: Some(RspText { text: Some(text) }),
        } if r#type == "content_block_delta" => Ok(Some(text)),
        _ => Ok(None),
    }
}

fn convert_to_option<T>(
    recv: core::result::Result<Option<ResponseStream>, SdkError<ResponseStreamError, T>>,
) -> Option<Result<String>>
where
    T: Send + Sync + Debug + 'static,
{
    match recv {
        Err(e) => Some(Err(Box::new(e.into_service_error()))),
        Ok(Some(ResponseStream::Chunk(PayloadPart {
            bytes: Some(bytes), ..
        }))) => {
            Some(String::from_utf8(bytes.into_inner()).map_err(|e| Box::new(e) as Box<dyn Error>))
        }
        Ok(Some(_)) => Some(Ok(String::new())), //ResponseStream::Unknown
        Ok(None) => None,
    }
}
