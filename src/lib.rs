mod utils;
mod list;
mod acct;
mod consts;
mod auth;

use worker::*;
use list::{ListManager, CustomizedResponse};
use auth::{Auth, AuthState};
use consts::msgs;

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or("unknown region".into())
    );
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
            
            match form.auth(&ctx).await? {
                AuthState::Ok => Response::empty()?.into_customized(),
                AuthState::Err(err) => err,
            }
        })
        .post_async("/add", |mut req, ctx| async move {
            let form = req.form_data().await?;

            match form.auth(&ctx).await? {
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
                _ => Response::error(msgs::ERR_ADD, 400)
            }
        })
        .post_async("/move", |mut req, ctx| async move {
            let form = req.form_data().await?;

            match form.auth(&ctx).await? {
                AuthState::Ok => (),
                AuthState::Err(err) => return err
            }

            match form.get("from").zip(form.get("to")) {
                Some((FormEntry::Field(from), FormEntry::Field(to))) => {
                    let manager = ListManager::new(&ctx).await?;
                    let indexes: (usize, usize) = from.parse().ok().zip(to.parse().ok()).ok_or(msgs::ERR_PARSE)?;

                    if indexes.0.max(indexes.1) + 1 > manager.mut_value.borrow_mut().len() {
                        return Response::error(msgs::ERR_OUT_OF_INDEX, 400);
                    }
                
                    let element = (&manager.mut_value.borrow_mut()[indexes.0]).to_string();

                    manager.mut_value.borrow_mut().remove(indexes.0);
                    manager.mut_value.borrow_mut().insert(indexes.1, element);
                    manager.update().await?;

                    manager.end_with_response()
                }
                _ => Response::error(msgs::ERR_MOVE, 400)
            }
        })
        .post_async("/delete", |mut req, ctx| async move {
            let form = req.form_data().await?;

            match form.auth(&ctx).await? {
                AuthState::Ok => (),
                AuthState::Err(err) => return err
            }

            match form.get("index") {
                Some(FormEntry::Field(index)) => {
                    let manager = ListManager::new(&ctx).await?;

                    manager.mut_value.borrow_mut().remove(index.parse().or(Err(msgs::ERR_PARSE))?);
                    manager.update().await?;

                    manager.end_with_response()
                }
                _ => Response::error(msgs::ERR_DELETE, 400)
            }
        })
        .run(req, env)
        .await
}
