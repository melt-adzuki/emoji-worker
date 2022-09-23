use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Accounts {
    pub value: Vec<Account>,
}

#[derive(Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub password: String,
}
