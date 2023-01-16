#[macro_use]
extern crate rocket;

use rocket::{Build, Rocket, form::Form, request::FromParam};
use lazy_static::lazy_static;
use hashbrown::HashMap;

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

#[post("/post", data="<data>")]
fn post(data: Form<Filters>) -> &'static str {
    "POST Request"
}

#[get("/user/<uuid>", rank=1, format="text/plain")]
fn user(uuid: &str) -> String {
    let user = USERS.get(uuid);
    match user {
        Some(user) => format!("Found user: {:?}", user),
        None => String::from("User not found"),
    }
}

/*#[get("/users/<grade>?<filters..>")]
fn users(grade: u8, filters: Filters) {
    unimplemented!()
}*/

#[get("/users/<name_grade>?<filters..>")]
fn users(name_grade: NameGrade, filters: Option<Filters>) -> String {
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
    if users.len() > 0 {
        users
            .iter()
            .map(|u| u.name.to_string())
            .collect::<Vec<String>>()
            .join(",")
    } else {
        String::from("No user found")
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
