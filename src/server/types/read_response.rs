use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadResponse<W, R> {
    pub workspaces: W,
    pub registers: R,
}
