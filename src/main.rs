#[macro_use]
extern crate rocket;

use rocket::{Build, Data, Orbit, Rocket, State};
use rocket::fairing::{self, Fairing, Info, Kind};
use rocket::form::Form;
use rocket::fs::{NamedFile, relative};
use rocket::http::{ContentType, Header, Status};
use rocket::request::{FromParam, Request};
use rocket::response::{self, Responder, Response};

use serde::Deserialize;
use sqlx::{FromRow, PgPool};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Deserialize)]
struct Config {
    database_url: String,
}

//* --- templating -------------------------------------------------------------

fn default_response<'r>() -> Response<'r> {
    Response::build()
    .header(ContentType::Plain)
    .raw_header("X-CUSTOM-ID", "CUSTOM")
    .finalize()
}

/*
    Injecting a tracing ID to requests and responses
*/
const X_TRACE_ID: &str = "X-TRACE-ID";
struct XTraceId {}
#[rocket::async_trait]
impl Fairing for XTraceId {
    fn info(&self) -> Info {
        Info {
            name: "X-TRACE-ID Injector",
            kind: Kind::Request | Kind::Response,
        }
    }
    async fn on_request(&self, req: &mut Request<'_>, _: &mut Data<'_>) {
        req.add_header(Header::new(
            X_TRACE_ID,
            Uuid::new_v4().hyphenated().to_string()
        ));
    }
    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        res.set_header(Header::new(
            X_TRACE_ID,
            req.headers().get_one(X_TRACE_ID).unwrap()
        ));
    }
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

/*
    Implement Fairing for VisitorCounter
    ignite and liftoff are not really needed, just used to show
    event handling. Counter will be incremented on every request.
*/

#[rocket::async_trait]
impl Fairing for VisitorCounter {
    fn info(&self) -> Info {
        Info {
            name: "Visitor Counter",
            kind: Kind::Ignite | Kind::Liftoff | Kind::Request,
        }
    }
    async fn on_ignite(&self, rocket: Rocket<Build>) -> fairing::Result {
        println!("Setting up visitor counter");
        Ok(rocket)
    }
    async fn on_liftoff(&self, _: &Rocket<Orbit>) {
        println!("Finish setting up visitor counter");
    }
    async fn on_request(&self, _: &mut Request<'_>, _: &mut Data<'_>) {
        self.increment();
    }
}

//* --- user -------------------------------------------------------------------

#[derive(FromForm)]
struct Filters {
    age: i16,
    active: bool,
}

#[derive(Debug, FromRow)]
struct User {
    uuid: Uuid,
    name: String,
    age: i16,
    grade: i16,
    active: bool,
}

impl<'r> Responder<'r, 'r> for User {
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
struct NewUser(Vec<User>);

impl<'r> Responder<'r, 'r> for NewUser {
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
    grade: i16,
}

impl<'r> FromParam<'r> for NameGrade<'r> {
    type Error = &'static str;
    fn from_param(param: &'r str) -> Result<Self, Self::Error> {
        const ERROR_MESSAGE: Result<NameGrade, &'static str> = Err("Error parsing user parameter");
        let name_grade_vec: Vec<&'r str> = param.split('_').collect();
        match name_grade_vec.len() {
            2 => match name_grade_vec[1].parse::<i16>() {
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

//* --- database ---------------------------------------------------------------


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
async fn user(pool: &State<PgPool>, uuid: &str) -> Result<User, Status> {
    let parsed_uuid = Uuid::parse_str(uuid)
        .map_err(|_| Status::BadRequest)?;
    let user = sqlx::query_as!(
        User,
        "SELECT * FROM users WHERE uuid = $1",
        parsed_uuid
    )
    .fetch_one(pool.inner())
    .await;
    user.map_err(|_| Status::NotFound)
}

/*#[get("/users/<grade>?<filters..>")]
fn users(grade: u8, filters: Filters) {
    unimplemented!()
}*/

#[get("/users/<name_grade>?<filters..>")]
async fn users(pool: &State<PgPool>, name_grade: NameGrade<'_>, filters: Option<Filters>) -> Result<NewUser, Status> {
    let mut query_str = String::from("SELECT * FROM users WHERE name LIKE $1 AND grade = $2");
    if filters.is_some() {
        query_str.push_str("AND age = $3 AND active = $4");
    }
    let mut query = sqlx::query_as::<_, User>(&query_str)
        .bind(format!("%{}%", &name_grade.name))
        .bind(name_grade.grade);
    if let Some(fts) = &filters {
        query = query.bind(fts.age).bind(fts.active);
    }
    let unwrapped_users = query.fetch_all(pool.inner()).await;
    let users: Vec<User> = unwrapped_users.map_err(|_| Status::InternalServerError)?;
    if users.is_empty() {
        Err(Status::NotFound)
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
async fn rocket() -> Rocket<Build> {
    let this_rocket = rocket::build();
    let config: Config = this_rocket
        .figment()
        .extract()
        .expect("Incorrect Rocket.toml configuration");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
        .expect("Failed to connect to database");
    let visitor_counter = VisitorCounter {
        visitor: AtomicU64::new(0),
    };
    let x_trace_id = XTraceId {};
    this_rocket
        .attach(visitor_counter)
        .attach(x_trace_id)
        .manage(pool)
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
