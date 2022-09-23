use super::acct::Accounts;
use super::consts::KV_BINDING;
use worker::*;

pub enum AuthState {
    Ok,
    Err(Result<Response>),
}

pub async fn auth(form: &FormData, ctx: &RouteContext<()>) -> Result<AuthState> {
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
