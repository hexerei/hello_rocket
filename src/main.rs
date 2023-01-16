#[macro_use]
extern crate rocket;

use rocket::{Build, Rocket, Request, State};
use rocket::form::Form;
use rocket::request::FromParam;
use rocket::http::{ContentType, Status};
use rocket::response::{self, Responder, Response, status};
use rocket::fs::{NamedFile, relative};

use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use lazy_static::lazy_static;
use hashbrown::HashMap;

//* --- templating -------------------------------------------------------------

fn default_response<'r>() -> Response<'r> {
    Response::build()
    .header(ContentType::Plain)
    .raw_header("X-CUSTOM-ID", "CUSTOM")
    .finalize()
}

struct VisitorCounter {
    visitor: AtomicU64,
}

impl VisitorCounter {
    fn increment(&self) {
        self.visitor.fetch_add(1, Ordering::Relaxed);
        println!(
            "Number of visitors: {}",
            self.visitor.load(Ordering::Relaxed)
        );
    }
}

//* --- user -------------------------------------------------------------------

#[derive(FromForm)]
struct Filters {
    age: u8,
    active: bool,
}

#[derive(Debug)]
struct User {
    uuid: String,
    name: String,
    age: u8,
    grade: u8,
    active: bool,
}

impl<'r> Responder<'r, 'r> for &'r User {
    fn respond_to(self, _request: &'r rocket::Request<'_>) -> response::Result<'r> {
        let base_response = default_response();
        let user = format!("Found user: {:?}", self);
        Response::build()
            .sized_body(user.len(), Cursor::new(user))
            .raw_header("X-USER-ID", self.uuid.to_string())
            .merge(base_response)
            .ok()
    }
}

#[derive(Debug)]
struct NewUser<'a>(Vec<&'a User>);

impl<'r> Responder<'r, 'r> for NewUser<'r> {
    fn respond_to(self, _request: &'r rocket::Request<'_>) -> response::Result<'r> {
        let base_response = default_response();
        let user = self.0.iter()
            .map(|u| format!("{:?}", u))
            .collect::<Vec<String>>()
            .join(",");
        Response::build()
            .sized_body(user.len(), Cursor::new(user))
            .raw_header("X-CUSTOM-ID", "USERS")
            .join(base_response)
            .ok()
    }
}

#[derive(Debug)]
struct NameGrade<'r> {
    name: &'r str,
    grade: u8,
}

impl<'r> FromParam<'r> for NameGrade<'r> {
    type Error = &'static str;
    fn from_param(param: &'r str) -> Result<Self, Self::Error> {
        const ERROR_MESSAGE: Result<NameGrade, &'static str> = Err("Error parsing user parameter");
        let name_grade_vec: Vec<&'r str> = param.split('_').collect();
        match name_grade_vec.len() {
            2 => match name_grade_vec[1].parse::<u8>() {
                Ok(n) => Ok(Self {
                    name: name_grade_vec[0],
                    grade: n,
                }),
                Err(_) => ERROR_MESSAGE,

            },
            _ => ERROR_MESSAGE
        }
    }
}

//* --- object store -----------------------------------------------------------

lazy_static! {
    static ref USERS: HashMap<&'static str, User> = {
        let mut map = HashMap::new();
        map.insert(
            "3e3dd4ae-3c37-40c6-aa64-7061f284ce28",
            User {
                uuid: String::from("3e3dd4ae-3c37-40c6-aa64-7061f284ce28"),
                name: String::from("Daniel"),
                age: 53,
                grade: 1,
                active: true,
            }
        );
        map.insert(
            "3e3dd4ae-3c37-40c6-aa64-7061f284ce29",
            User {
                uuid: String::from("3e3dd4ae-3c37-40c6-aa64-7061f284ce29"),
                name: String::from("John"),
                age: 57,
                grade: 1,
                active: true,
            }
        );
        map.insert(
            "3e3dd4ae-3c37-40c6-aa64-7061f284ce30",
            User {
                uuid: String::from("3e3dd4ae-3c37-40c6-aa64-7061f284ce30"),
                name: String::from("Arne"),
                age: 36,
                grade: 2,
                active: false,
            }
        );
        map
    };
}

//* --- routes -----------------------------------------------------------------

#[catch(403)]
fn forbidden(reqest: &Request) -> String {
    format!("Access forbidden {}.", reqest.uri())
}

#[catch(404)]
fn not_found(reqest: &Request) -> String {
    format!("We cannot find this page {}.", reqest.uri())
}

#[get("/favicon.png")]
async fn favicon() -> NamedFile {
    NamedFile::open(Path::new(relative!("static")).join("favicon.png")).await.unwrap()
}

#[post("/post", data="<data>")]
fn post(data: Form<Filters>) -> &'static str {
    "POST Request"
}

#[get("/user/<uuid>", rank=1, format="text/plain")]
fn user<'a>(counter: &State<VisitorCounter>, uuid: &'a str) -> Option<&'a User> {
    counter.increment();
    USERS.get(uuid)
}

/*#[get("/users/<grade>?<filters..>")]
fn users(grade: u8, filters: Filters) {
    unimplemented!()
}*/

#[get("/users/<name_grade>?<filters..>")]
fn users<'a>(counter: &State<VisitorCounter>, name_grade: NameGrade, filters: Option<Filters>) -> Result<NewUser<'a>, Status> {
    counter.increment();
    let users: Vec<&User> = USERS
        .values()
        .filter(|user| user.name.contains(&name_grade.name) 
            && user.grade == name_grade.grade)
        .filter(|user| {
            if let Some(fts) = &filters {
                user.age == fts.age
                && user.active == fts.active
            } else {
                true
            }
        })
        .collect();
    if users.is_empty() {
        Err(Status::Forbidden)
    } else {
        Ok(NewUser(users))
    }
}

#[get("/<rank>", rank=1)]
fn first(rank: u8) -> String {
    let result = rank + 10;
    format!("Your rank is, {}!", result)
}

#[get("/<name>", rank=2)]
fn second(name: &str) -> String {
    format!("Hello, {}!", name)
}

#[get("/")]
async fn index() -> &'static str {
    "Hello, Rocket!"
}

//* --- main -------------------------------------------------------------------

#[launch]
fn rocket() -> Rocket<Build> {
    let visitor_counter = VisitorCounter {
        visitor: AtomicU64::new(0),
    };
    rocket::build()
        .manage(visitor_counter)
        .mount("/", routes![user, users, favicon])
        .register("/", catchers![forbidden, not_found])
}

// #[rocket::main]
// async fn main() {
//     rocket::build()
//         .mount("/", routes![index])
//         .launch()
//         .await;
// }
