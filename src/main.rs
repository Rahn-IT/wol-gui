use rocket::{fairing::AdHoc, fs::FileServer, Build, Rocket};
use rocket_dyn_templates::Template;

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_sync_db_pools;
#[macro_use]
extern crate diesel;

mod schema;
mod wol;

#[database("sqlite_database")]
pub struct DbConn(diesel::SqliteConnection);

#[launch]
async fn rocket() -> _ {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    rocket::build()
        .configure(rocket::Config::figment().merge(("port", port)))
        .mount(
            "/",
            routes![
                wol::index,
                wol::create,
                wol::update,
                wol::delete,
                wol::wake,
                wol::online_status
            ],
        )
        .mount("/static", FileServer::from("templates/static"))
        .attach(Template::fairing())
        .attach(DbConn::fairing())
        .attach(AdHoc::on_ignite("Run Migrations", run_migrations))
}

async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

    DbConn::get_one(&rocket)
        .await
        .expect("database connection")
        .run(|conn| {
            conn.run_pending_migrations(MIGRATIONS)
                .expect("diesel migrations");
        })
        .await;

    rocket
}
