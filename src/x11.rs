
use x11rb::atom_manager;
use x11rb::connection::Connection;
//use x11rb::rust_connection::RustConnection;
use x11rb::errors::ReplyOrIdError;
use x11rb::errors::ReplyError;
use x11rb::protocol::xproto::*;
//use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;

atom_manager! {
    pub Atoms: AtomsCookie {
        UTF8_STRING,
        WM_DELETE_WINDOW,
        WM_PROTOCOLS,
        _NET_WM_NAME,
        WM_NAME,
    }
}


pub fn rs_create_window(
    conn: &impl Connection,
    screen: &Screen,
    atoms: &Atoms,
    event_mask: EventMask,
    class: &str,
    (width, height): (u16, u16),
) -> Result<Window, ReplyOrIdError> {
    let win_id = conn.generate_id()?;

    let win_aux = CreateWindowAux::new()
        .event_mask(event_mask)
        .background_pixel(screen.black_pixel);

    conn.create_window(
        screen.root_depth,
        win_id,
        screen.root,
        0,
        0,
        width,
        height,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &win_aux,
    )?;

    conn.change_property8(
        PropMode::REPLACE,
        win_id,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        class.as_bytes(),
    )?;
    conn.change_property8(
        PropMode::REPLACE,
        win_id,
        AtomEnum::WM_CLASS,
        AtomEnum::STRING,
        class.as_bytes(),
    )?;
    conn.change_property8(
        PropMode::REPLACE,
        win_id,
        atoms._NET_WM_NAME,
        atoms.UTF8_STRING,
        class.as_bytes(),
    )?;
    conn.change_property32(
        PropMode::REPLACE,
        win_id,
        atoms.WM_PROTOCOLS,
        AtomEnum::ATOM,
        &[atoms.WM_DELETE_WINDOW],
    )?;

    conn.map_window(win_id)?;
    Ok(win_id)
}


pub fn rs_get_window_class(conn: &impl Connection, window: Window, )
    -> Result<String, ReplyError> {
    let bytes = rs_get_window_property8(conn, AtomEnum::WM_CLASS.into(), window)?;
    Ok(String::from_utf8(bytes).unwrap_or_default())
}

pub fn rs_get_window_name(conn: &impl Connection, atoms: &Atoms, window: Window, )
    -> Result<String, ReplyError> {
    let mut bytes = rs_get_window_property8(conn, atoms._NET_WM_NAME, window)?;

    if bytes.is_empty() {
        bytes = rs_get_window_property8(conn, atoms.WM_NAME, window)?;
    }

    Ok(String::from_utf8(bytes).unwrap_or_default())
}


fn rs_get_property_any(conn: &impl Connection, atom: Atom, window: Window)
-> Result<GetPropertyReply, ReplyError> {
    conn.get_property(
        false,
        window,
        atom,
        AtomEnum::ANY,
        0,
        u32::MAX,
        )?.reply()
}

pub fn rs_get_window_property8(conn: &impl Connection, atom: Atom, window: Window)
-> Result<Vec<u8>, ReplyError> {
    let reply = rs_get_property_any(conn, atom, window)?;
    Ok(reply.value8().into_iter().flatten().collect())
}

pub fn rs_get_window_property16(conn: &impl Connection, atom: Atom, window: Window)
-> Result<Vec<u16>, ReplyError> {
    let reply = rs_get_property_any(conn, atom, window)?;
    Ok(reply.value16().into_iter().flatten().collect())
}

pub fn rs_get_window_property32(conn: &impl Connection, atom: Atom, window: Window)
-> Result<Vec<u32>, ReplyError> {
    let reply = rs_get_property_any(conn, atom, window)?;
    Ok(reply.value32().into_iter().flatten().collect())
}


pub fn rs_send_delete_window_event(conn: &impl Connection, atoms: &Atoms, window: Window)
-> Result<(), ReplyError> {
    let event = ClientMessageEvent::new(
        32,
        window,
        atoms.WM_PROTOCOLS,
        [atoms.WM_DELETE_WINDOW, 0, 0, 0, 0],
    );

    conn.send_event(false, window, EventMask::NO_EVENT, event)?.check()
}




/// A rust version of XCB's `xcb_visualtype_t` struct. This is used in a FFI-way.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct xcb_visualtype_t {
    pub visual_id: u32,
    pub class: u8,
    pub bits_per_rgb_value: u8,
    pub colormap_entries: u16,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub pad0: [u8; 4],
}

/// Find a `xcb_visualtype_t` based on its ID number
pub fn find_xcb_visualtype(conn: &impl Connection, visual_id: u32) -> Option<xcb_visualtype_t> {
    for root in &conn.setup().roots {
        for depth in &root.allowed_depths {
            for visual in &depth.visuals {
                if visual.visual_id == visual_id {
                    return Some((*visual).into());
                }
            }
        }
    }
    None
}


impl From<Visualtype> for xcb_visualtype_t {
    fn from(value: Visualtype) -> xcb_visualtype_t {
        xcb_visualtype_t {
            visual_id: value.visual_id,
            class: value.class.into(),
            bits_per_rgb_value: value.bits_per_rgb_value,
            colormap_entries: value.colormap_entries,
            red_mask: value.red_mask,
            green_mask: value.green_mask,
            blue_mask: value.blue_mask,
            pad0: [0; 4],
        }
    }
}
