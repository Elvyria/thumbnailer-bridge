use std::time::Duration;

use rustbus::{RpcConn, MessageBuilder, connection::{Timeout, Error}};

use crate::URI_PREFIX;

const TIMEOUT: Timeout = Timeout::Duration(Duration::from_secs(1));

// https://wiki.gnome.org/DraftSpecs/ThumbnailerSpec#org.freedesktop.thumbnails.Thumbnailer1
macro_rules! message {
    ($method:expr) => {
        MessageBuilder::new()
            .call($method)
            .with_interface("org.freedesktop.thumbnails.Thumbnailer1")
            .on("/org/freedesktop/thumbnails/Thumbnailer1")
            .at("org.freedesktop.thumbnails.Thumbnailer1")
            .build()
    };
}

pub fn list_flavors(conn: &mut RpcConn) -> Result<Vec<String>, Error> {
    let mut msg = message!("GetFlavors");
    let id = conn.send_message(&mut msg)?.write_all().map_err(|e| e.1)?;

    let resp = conn.wait_response(id, TIMEOUT)?;
    let flavors: Vec<String> = resp.body.parser().get()?;

    Ok(flavors)
}

pub fn request_supported(conn: &mut RpcConn) -> Result<u32, Error> {
    let mut msg = message!("GetSupported");
    let id = conn.send_message(&mut msg)?.write_all().map_err(|e| e.1)?;

    Ok(id)
}

pub fn wait_supported(conn: &mut RpcConn, id: u32) -> Result<(Vec<String>, Vec<String>), Error> {
    let resp = conn.wait_response(id, TIMEOUT)?;
    let (schemes, mimes): (Vec<String>, Vec<String>) = resp.body.parser().get2()?;

    Ok((schemes, mimes))
}

pub fn list_schedulers(conn: &mut RpcConn) -> Result<Vec<String>, Error> {
    let mut msg = message!("GetSchedulers");
    let id = conn.send_message(&mut msg)?.write_all().map_err(|e| e.1)?;

    let resp = conn.wait_response(id, TIMEOUT)?;
    let schedulers: Vec<String> = resp.body.parser().get()?;

    Ok(schedulers)
}

pub fn queue_thumbnails(conn: &mut RpcConn, uris: Vec<String>, mimes: Vec<String>, flavor: &str, scheduler: &str) -> Result<u32, Error> {
    let mut msg = message!("Queue");
    msg.body.push_param5(uris, mimes, flavor, scheduler, 0u32)?;

    let id = conn.send_message(&mut msg)?.write_all().map_err(|e| e.1)?;

    let resp = conn.wait_response(id, TIMEOUT)?;
    let handle = resp.body.parser().get::<u32>()?;

    Ok(handle)
}

fn monitor(conn: &mut RpcConn, handle: u32) -> Result<(), Error> {
    let timeout = Timeout::Duration(Duration::from_secs(60));

    loop {
        let Ok(signal) = conn.wait_signal(timeout) else {
            break;
        };

        match signal.dynheader.member.as_deref() {
            Some("Ready") => {
                let (reported_handle, uris): (u32, Vec<String>) = signal.body.parser().get2()?;

                if reported_handle == handle {
                    uris.iter()
                        .filter_map(|uri| uri.strip_prefix(URI_PREFIX))
                        .for_each(|path| println!("{}", path));
                }
            },
            Some("Finished") => {
                if handle == signal.body.parser().get::<u32>()? {
                    break
                }
            },
            _ => {}
        }
    }

    Ok(())
}

pub fn listen(conn: &mut RpcConn) -> Result<(), Error> {
    // https://dbus.freedesktop.org/doc/dbus-specification.html#bus-messages-become-monitor
    let mut msg = MessageBuilder::new()
        .call("BecomeMonitor")
        .with_interface("org.freedesktop.DBus.Monitoring")
        .on("/org/freedesktop/DBus")
        .at("org.freedesktop.DBus")
        .build();

    // https://dbus.freedesktop.org/doc/dbus-specification.html#message-bus-routing-match-rules
    msg.body.push_param2(vec!["type='signal',interface='org.freedesktop.thumbnails.Thumbnailer1',path='/org/freedesktop/thumbnails/Thumbnailer1',member='Ready'"], 0u32)?;

    let id = conn.send_message(&mut msg)?.write_all().map_err(|e| e.1)?;

    let _resp = conn.wait_response(id, Timeout::Infinite)?;

    loop {
        let signal = conn.wait_signal(Timeout::Infinite)?;

        if signal.dynheader.signature == Some("uas".to_owned()) {
            let (_handle, uris): (u32, Vec<String>) = signal.body.parser().get2()?;

            for uri in uris {
                if let Some(path) = uri.strip_prefix("file://") {
                    println!("{}", path);
                }
            }
        }
    }

    Ok(())
}
