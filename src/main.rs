#[macro_use]
extern crate rocket;

use rocket::{Build, Rocket};

#[derive(FromForm)]
struct Filters {
    age: u8,
    active: bool,
}

#[get("/user/<uuid>", rank=1, format="text/plain")]
fn user(uuid: &str) {
    unimplemented!()
}

#[get("/users/<grade>?<filters..>")]
fn users(grade: u8, filters: Filters) {
    unimplemented!()
}

// #[get("/")]
// async fn index() -> &'static str {
//     "Hello, Rocket!"
// }

#[launch]
fn rocket() -> Rocket<Build> {
    rocket::build()
        .mount("/", routes![user, users])
}

// #[rocket::main]
// async fn main() {
//     rocket::build()
//         .mount("/", routes![index])
//         .launch()
//         .await;
// }
