use std::num::ParseIntError;

use diesel::{ExpressionMethods, QueryDsl, QueryResult, RunQueryDsl};
use rocket::{
    form::Form,
    request::FlashMessage,
    response::{Flash, Redirect},
    tokio::net::UdpSocket,
};
use rocket_dyn_templates::Template;
use serde::Serialize;
use thiserror::Error;

use crate::{schema::wol, DbConn};

#[derive(Serialize, Queryable, Insertable, Identifiable, AsChangeset, FromForm, Clone, Debug)]
#[serde(crate = "rocket::serde")]
#[diesel(table_name = wol)]
pub struct WolDevice {
    #[serde(skip_deserializing)]
    pub id: Option<i32>,
    pub name: String,
    pub mac: String,
    pub ip: Option<String>,
}

impl WolDevice {
    pub async fn count(conn: &DbConn) -> QueryResult<i64> {
        conn.run(|c| wol::table.count().first::<i64>(c)).await
    }

    pub async fn all(conn: &DbConn) -> QueryResult<Vec<WolDevice>> {
        conn.run(|c| wol::table.load::<WolDevice>(c)).await
    }

    pub async fn insert(device: WolDevice, conn: &DbConn) -> QueryResult<usize> {
        conn.run(move |c| diesel::insert_into(wol::table).values(&device).execute(c))
            .await
    }

    pub async fn update(id: i32, device: WolDevice, conn: &DbConn) -> QueryResult<usize> {
        conn.run(move |c| {
            diesel::update(wol::table)
                .filter(wol::id.eq(id))
                .set(&device)
                .execute(c)
        })
        .await
    }

    pub async fn delete(id: i32, conn: &DbConn) -> QueryResult<usize> {
        conn.run(move |c| diesel::delete(wol::table).filter(wol::id.eq(id)).execute(c))
            .await
    }

    pub async fn wake(id: i32, conn: &DbConn) -> Result<(), WakeError> {
        let device = conn
            .run(move |c| {
                wol::table
                    .select(wol::all_columns)
                    .filter(wol::id.eq(id))
                    .first::<WolDevice>(c)
            })
            .await?;

        let raw_mac = parse_mac(&device.mac)?;

        let mut magic_packet = [0u8; 6 + 16 * 6];
        magic_packet.iter_mut().take(6).for_each(|b| *b = 0xff);

        magic_packet
            .iter_mut()
            .skip(6)
            .enumerate()
            .for_each(|(i, b)| *b = raw_mac[i % 6] as u8);

        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.set_broadcast(true)?;
        socket.send_to(&magic_packet, "255.255.255.255:9").await?;

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum WakeError {
    #[error("Could not parse MAC address: {0}")]
    ParseMac(#[from] ParseMacError),
    #[error("DB error: {0}")]
    Query(#[from] diesel::result::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum ParseMacError {
    #[error("Invalid MAC address: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("wrong amound of touples: {0}")]
    InvalidToupleAmount(u8),
}

fn parse_mac(mac: &str) -> Result<[u8; 6], ParseMacError> {
    let parts = mac.split('-');
    let count = parts.clone().count() as u8;

    if count != 6 {
        return Err(ParseMacError::InvalidToupleAmount(count));
    }

    let mut mac_bytes = [0u8; 6];

    for (b, part) in mac_bytes.iter_mut().zip(parts) {
        *b = u8::from_str_radix(part, 16)?
    }

    Ok(mac_bytes)
}

#[derive(Serialize)]
struct Wol {
    flash: Option<(String, String)>,
    devices: Vec<WolDevice>,
    edit: Option<i32>,
}

impl Wol {
    pub async fn raw(conn: &DbConn, flash: Option<(String, String)>, edit: Option<i32>) -> Self {
        match WolDevice::all(conn).await {
            Ok(devices) => Self {
                flash,
                devices,
                edit,
            },
            Err(e) => {
                error!("DB error loading devices: {}", e);
                Self {
                    flash: Some(("error".into(), e.to_string())),
                    devices: Vec::new(),
                    edit: None,
                }
            }
        }
    }
}

#[get("/?<edit>")]
pub async fn index(edit: Option<i32>, flash: Option<FlashMessage<'_>>, conn: DbConn) -> Template {
    let flash = flash.map(FlashMessage::into_inner);
    Template::render("wol", Wol::raw(&conn, flash, edit).await)
}

#[post("/wol", data = "<device_form>")]
pub async fn create(device_form: Form<WolDevice>, conn: DbConn) -> Flash<Redirect> {
    let device = device_form.into_inner();

    // TODO: validate

    if let Err(e) = WolDevice::insert(device, &conn).await {
        Flash::error(Redirect::to("/"), e.to_string())
    } else {
        Flash::success(Redirect::to("/"), "Device created")
    }
}

#[post("/wol/<id>", data = "<device_form>")]
pub async fn update(id: i32, device_form: Form<WolDevice>, conn: DbConn) -> Flash<Redirect> {
    let mut device = device_form.into_inner();
    device.mac = device.mac.replace(":", "-");

    // TODO: validate

    if let Err(e) = parse_mac(&device.mac) {
        return Flash::error(
            Redirect::to(format!("/?edit={id}")),
            format!("Invalid MAC address: {}", e),
        );
    }

    if let Err(e) = WolDevice::update(id, device, &conn).await {
        Flash::error(Redirect::to(format!("/?edit={id}")), e.to_string())
    } else {
        Flash::success(Redirect::to("/"), "Device updated")
    }
}

#[post("/wol/<id>/delete", data = "<confirm>")]
pub async fn delete(id: i32, confirm: Form<bool>, conn: DbConn) -> Flash<Redirect> {
    if confirm.into_inner() {
        if let Err(e) = WolDevice::delete(id, &conn).await {
            Flash::error(Redirect::to("/"), e.to_string())
        } else {
            Flash::success(Redirect::to("/"), "Device deleted")
        }
    } else {
        Flash::error(Redirect::to("/"), "Delete cancelled")
    }
}

#[post("/wol/<id>/wake")]
pub async fn wake(id: i32, conn: DbConn) -> Flash<Redirect> {
    if let Err(e) = WolDevice::wake(id, &conn).await {
        Flash::error(Redirect::to("/"), e.to_string())
    } else {
        Flash::success(Redirect::to("/"), "Sent WOL packet")
    }
}
