//! @agents-index Gets the login of the token user from GET /user, once per fetch; 401 and 403 give the cause unauthorized.

use super::client::{Cause, Client};

/// The login stays in memory for the fetch only.
pub fn login(c: &Client) -> Result<String, Cause> {
    let v = c.get("/user", &[]).map_err(|e| match e {
        Cause::Http(401 | 403) => Cause::Unauthorized,
        other => other,
    })?;
    v["login"]
        .as_str()
        .map(str::to_string)
        .ok_or(Cause::Http(200))
}
