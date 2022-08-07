use std::cell::RefCell;
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

const KV_BINDING: &str = "EMOJIS";

trait CustomizedResponse {
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

#[derive(Serialize, Deserialize)]
struct EmojiList {
    value: Vec<String>,
}

struct ListManager {
    kv_store: worker::kv::KvStore,
    mut_value: RefCell<Vec<String>>,
}

impl ListManager {
    async fn new(ctx: &RouteContext<()>) -> Result<Self> {
        let kv_store = ctx.kv(KV_BINDING)?;
        let list: EmojiList = kv_store.get("list").json().await?.ok_or("Couldn't fetch list")?;
        let mut_value = RefCell::new(list.value);

        Ok(Self { kv_store, mut_value })
    }

    async fn update(&self) -> Result<()> {
        let value_str = self.get_str()?;
        let _ = self.kv_store.put("list", value_str)?.execute().await?;

        Ok(())
    }

    fn end_with_response(&self) -> Result<Response> {
        let value_str = self.get_str()?;
        Response::ok(value_str)?.into_customized()
    }

    fn get_str(&self) -> Result<String> {
        let list = EmojiList { value: self.mut_value.borrow().to_vec() };
        let value_str = serde_json::to_string(&list)?;

        Ok(value_str)
    }
}

#[derive(Serialize, Deserialize)]
struct Accounts {
    value: Vec<Account>,
}

#[derive(Serialize, Deserialize)]
struct Account {
    username: String,
    password: String,
}

enum AuthState {
    Ok,
    Err(Result<Response>),
}

async fn auth(form: &FormData, ctx: &RouteContext<()>) -> Result<AuthState> {
    match form.get("username").zip(form.get("password")) {
        Some((FormEntry::Field(username), FormEntry::Field(password))) => {
            let kv_store = ctx.kv(KV_BINDING)?;
            let accounts: Accounts = kv_store.get("accounts").json().await?.ok_or("Couldn't fetch accounts")?;

            match accounts.value.iter().find(|acct| acct.username == username) {
                Some(acct) => Ok(
                    if acct.password == password { AuthState::Ok }
                    else { AuthState::Err(Response::error("Invalid password", 401)) }
                ),
                None => Ok(AuthState::Err(Response::error("Invalid username", 401))),
            }
        }
        _ => Ok(AuthState::Err(Response::error("Invalid parameter", 401)))
    }
}

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
        .get_async("/list", |_req, ctx| async move {
            let manager = ListManager::new(&ctx).await?;
            manager.end_with_response()
        })
        .post_async("/auth", |mut req, ctx| async move {
            let form = req.form_data().await?;
            
            match auth(&form, &ctx).await? {
                AuthState::Ok => Response::empty()?.into_customized(),
                AuthState::Err(err) => err,
            }
        })
        .post_async("/add", |mut req, ctx| async move {
            let form = req.form_data().await?;

            match auth(&form, &ctx).await? {
                AuthState::Ok => (),
                AuthState::Err(err) => return err
            }
            
            match form.get("content") {
                Some(FormEntry::Field(content)) => {
                    let manager = ListManager::new(&ctx).await?;

                    manager.mut_value.borrow_mut().push(content);
                    manager.update().await?;

                    manager.end_with_response()
                }
                _ => Response::error("Couldn't add content", 400)
            }
        })
        .post_async("/move", |mut req, ctx| async move {
            let form = req.form_data().await?;

            match auth(&form, &ctx).await? {
                AuthState::Ok => (),
                AuthState::Err(err) => return err
            }

            match form.get("content").zip(form.get("index")) {
                Some((FormEntry::Field(content), FormEntry::Field(index))) => {
                    let manager = ListManager::new(&ctx).await?;
                    
                    manager.mut_value.borrow_mut().insert(index.parse().or(Err("Failed to parse"))?, content);
                    manager.update().await?;

                    manager.end_with_response()
                }
                _ => Response::error("Couldn't move content", 400)
            }
        })
        .post_async("/delete", |mut req, ctx| async move {
            let form = req.form_data().await?;

            match auth(&form, &ctx).await? {
                AuthState::Ok => (),
                AuthState::Err(err) => return err
            }

            match form.get("index") {
                Some(FormEntry::Field(index)) => {
                    let manager = ListManager::new(&ctx).await?;

                    manager.mut_value.borrow_mut().remove(index.parse().or(Err("Failed to parse"))?);
                    manager.update().await?;

                    manager.end_with_response()
                }
                _ => Response::error("Couldn't delete content", 400)
            }
        })
        .run(req, env)
        .await
}
