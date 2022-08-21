mod account;
mod upload;

use rocket::fairing::AdHoc;
use rocket::Request;

use crate::components::MoneyResult;
use crate::error::MoneyError;

#[catch(404)]
fn not_found(req: &Request) -> MoneyResult<()> {
    Err(MoneyError::MissingEndpoint(
        req.uri().path().as_str().to_string(),
    ))
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Money API", |rocket| async {
        rocket
            .register("/api/", catchers![not_found])
            .mount("/api/upload", upload::routes())
            .mount("/api/account", account::routes())
    })
}
