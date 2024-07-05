use std::{error::Error, fmt::Debug, time::Instant};

use aws_sdk_bedrockruntime::{error::SdkError, primitives::Blob, types::{error::ResponseStreamError, PayloadPart, ResponseStream}, Client};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;


#[derive(Serialize)]
pub struct ReqMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct ReqMessages {
    pub messages: Vec<ReqMessage>,
}

impl ReqMessages {
    pub fn new(messages: Vec<ReqMessage>) -> ReqMessages {
        ReqMessages { messages }
    }
}

#[derive(Serialize)]
struct ReqBody {
    anthropic_version: String,
    max_tokens: i32,
    temperature: f32,
    messages: Vec<ReqMessage>
}

pub type Result<T> = core::result::Result<T, Box<dyn Error>>;
pub type ResultIterator<T> = Result<Box<dyn Iterator<Item = T>>>;

pub struct Test {
    pub variable: String
}

impl Test {
    pub fn generate(self) -> ResultIterator<Result<String>> {
        let iter = std::iter::from_fn(move || Some(Ok(self.variable.clone())));
        Ok(Box::new(iter))
    }
}

/// Model provides the interface that any LLM being queried should implement.
pub trait Model {
    fn generate(self, query: ReqMessages) -> ResultIterator<Result<String>>;
}

/// Bedrock implementation of model.
/// The aws client uses async/tokio, and so the associated runtime is for use (`block_on`) with the client.
///
pub struct Bedrock {
    pub model_id: String,
    pub runtime: Runtime,
    pub client: Client,
}

impl Bedrock {
    pub fn create(model_id: String) -> Result<Self> {
        let runtime = Runtime::new()?;
        let start = Instant::now();
        let config = runtime.block_on(aws_config::from_env().region(Self::region()).load());
        log::info!("Load aws cfg: {:?}ms", (Instant::now() - start).as_millis());
        let client = aws_sdk_bedrockruntime::Client::new(&config);
        Ok(Bedrock { model_id, runtime, client })
    }

    /// ::from_env is very slow when no region is specified. specifying explicitly is a big speed up, but maybe there's a better
    /// way
    fn region() -> &'static str {
        "us-east-1"
    }
}

impl Model for Bedrock {
    fn generate(self, query: ReqMessages) -> ResultIterator<Result<String>> {

        let body_str = serde_json::to_string(&ReqBody {
            anthropic_version: "bedrock-2023-05-31".to_string(),
            max_tokens: 10240,
            temperature: 0.5,
            messages: query.messages
        })?;

        log::info!("Request Body: {body_str:?}");

        let body = body_str.into_bytes();

        let async_request = self.client.invoke_model_with_response_stream()
            .model_id(self.model_id.clone())
            .body(Blob::new(body))
            .send();

        log::info!("Starting request:");
        let response = self.runtime.block_on(async_request)?;
        log::info!("Response: {:?}", response.content_type);
        let mut event_receiver = response.body;
        let iter =
            std::iter::from_fn(move || convert_to_option(self.runtime.block_on(event_receiver.recv())))
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
/// e.g.s
/// Ok("{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}")
/// Ok("{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}")
/// Ok("{\"type\":\"content_block_stop\",\"index\":0}")
/// Ok("{\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":12}}")
/// Ok("{\"type\":\"message_stop\",\"amazon-bedrock-invocationMetrics\":{ ... }})
fn parse_claude_api_text(chunk_text: String) -> Result<Option<String>> {
    // This copies the part of the response chunks that we want to extract. Can we avoid this copy?

    log::info!("Input: {chunk_text:?}");

    match serde_json::from_str(&chunk_text)? {
        RspChunk { r#type, delta: Some(RspText { text: Some(text) }) } if r#type == "content_block_delta" => Ok(Some(text)),
        _ => Ok(None),
    }
}

fn convert_to_option<T>(recv: core::result::Result<Option<ResponseStream>, SdkError<ResponseStreamError, T>>) -> Option<Result<String>>
where
    T: Send + Sync + Debug + 'static,
{
    match recv {
        Err(e) => Some(Err(Box::new(e.into_service_error()))),
        Ok(Some(ResponseStream::Chunk(PayloadPart { bytes: Some(bytes), .. }))) =>
            Some(String::from_utf8(bytes.into_inner())
                .map_err(|e| Box::new(e) as Box<dyn Error>)),
        Ok(Some(_)) => Some(Ok(String::new())), //ResponseStream::Unknown
        Ok(None) => None,
    }
}
