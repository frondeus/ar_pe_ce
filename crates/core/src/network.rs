use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Serialize, Debug)]
pub struct Headers<E> {
    pub method: E,
    pub headers: HashMap<String, Vec<u8>>,
}
