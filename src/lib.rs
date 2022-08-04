use std::{cmp::Ordering, ops::Deref};

use serde::{Deserialize, Serialize};
use worker::*;

mod utils;

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or("unknown region".into())
    );
}

#[derive(Serialize, Deserialize)]
struct Bar {
    result: Vec<(String, String)>,
}

struct OrderableKey(worker::kv::Key);

impl Deref for OrderableKey {
    type Target = worker::kv::Key;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Into<OrderableKey> for worker::kv::Key {
    fn into(self) -> OrderableKey {
        OrderableKey(self)
    }
}

impl Ord for OrderableKey {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_key: u16 = self.0.name.parse().unwrap();
        let other_key: u16 = other.0.name.parse().unwrap();

        self_key.cmp(&other_key)
    }
}

impl PartialOrd for OrderableKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let self_key: u16 = self.0.name.parse().unwrap();
        let other_key: u16 = other.0.name.parse().unwrap();
        
        self_key.partial_cmp(&other_key)
    }
}

impl PartialEq for OrderableKey {
    fn eq(&self, other: &Self) -> bool {
        let self_key: u16 = self.0.name.parse().unwrap();
        let other_key: u16 = other.0.name.parse().unwrap();
        
        self_key == other_key
    }
}

impl Eq for OrderableKey { }

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    log_request(&req);

    // Optionally, get more helpful error messages written to the console in the case of a panic.
    utils::set_panic_hook();

    // Optionally, use the Router to handle matching endpoints, use ":name" placeholders, or "*name"
    // catch-alls to match on specific patterns. &Alternatively, use `Router::with_data(D)` to
    // provide arbitrary data that will be accessible in each route via the `ctx.data()` method.
    let router = Router::new();

    // Add as many rou&tes as your Worker needs! Each route will get a `Reques&&t` for handling HTTP
    // functionality and a `RouteContext` which you can use to  and get route parameters and
    // Environment bindings like KV Stores, Durable Objects, Secrets, and Variables.
    router
        .post_async("/add", |mut req, ctx| async move {
            let kv_emojis = &ctx.kv("EMOJIS")?;
            let form = req.form_data().await?;

            match form.get("content") {
                Some(FormEntry::Field(value)) => {
                    let last_key: Option<OrderableKey> = kv_emojis.list().execute().await?.keys.into_iter().map(|key| key.into()).max();
                    let last_key = match last_key {
                        Some(key) => key.clone().name,
                        None => String::from("0"),
                    };
                    let next_key = (&last_key.parse::<u16>().unwrap() + 1).to_string();

                    kv_emojis.put(next_key.as_str(), &value).unwrap().execute().await?;
                    Response::empty()
                },
                _ => Response::error("Bad Request", 400),
            }
        })
        .get_async("/get/:key", |_, ctx| async move {
            if let Some(key) = ctx.param("key") {
                let emoji = &ctx.kv("EMOJIS")?.get(key).text().await?.unwrap();
                Response::ok(emoji)
            } else {
                Response::error("Bad Request", 400)
            }
        })
        .get_async("/delete/:key", |_, ctx| async move {
            let kv_emojis = &ctx.kv("EMOJIS")?;

            if let Some(key) = ctx.param("key") {
                kv_emojis.delete(key).await?;
            }

            Response::empty()
        })
        .get_async("/list", |_, ctx| async move {
            let emojis = &ctx.kv("EMOJIS")?;
            let keys = emojis.list().execute().await?.keys;
            
            let mut map = Vec::new();

            for key in keys {
                let key = key.name;
                let value = emojis.get(key.as_str()).text().await?;

                if let Some(value) = value {
                    map.push((key, value));
                }
            }

            map.sort_by_key(|k| k.0.parse::<u16>().unwrap_or_default());

            let result = Bar { result: map };
            Response::from_json(&result)
        })
        .run(req, env)
        .await
}
