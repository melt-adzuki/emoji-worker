use super::consts::KV_BINDING;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use worker::*;

#[derive(Serialize, Deserialize)]
struct EmojiList {
    value: Vec<String>,
}

pub struct ListManager {
    pub kv_store: worker::kv::KvStore,
    pub mut_value: RefCell<Vec<String>>,
}

impl ListManager {
    pub async fn new(ctx: &RouteContext<()>) -> Result<Self> {
        let kv_store = ctx.kv(KV_BINDING)?;
        let list: EmojiList = kv_store.get("list").json().await?.ok_or("Couldn't fetch list")?;
        let mut_value = RefCell::new(list.value);

        Ok(Self { kv_store, mut_value })
    }

    pub async fn update(&self) -> Result<()> {
        let value_str = self.get_str()?;
        let _ = self.kv_store.put("list", value_str)?.execute().await?;

        Ok(())
    }

    pub fn end_with_response(&self) -> Result<Response> {
        let value_str = self.get_str()?;
        Response::ok(value_str)?.into_customized()
    }

    pub fn get_str(&self) -> Result<String> {
        let list = EmojiList { value: self.mut_value.borrow().to_vec() };
        let value_str = serde_json::to_string(&list)?;

        Ok(value_str)
    }
}

pub trait CustomizedResponse {
    fn into_customized(self) -> Result<Response>;
}

impl CustomizedResponse for Response {
    fn into_customized(self) -> Result<Response> {
        let cors = Cors::new()
            .with_origins(["*"])
            .with_methods([Method::Get, Method::Post, Method::Options])
            .with_max_age(86400)
            .with_allowed_headers(["Content-Type"]);

        self.with_cors(&cors)
    }
}