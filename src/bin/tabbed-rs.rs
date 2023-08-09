use std::collections::HashMap;

use x11rb::CURRENT_TIME;
use x11rb::connection::Connection;
use x11rb::errors::ReplyError;
use x11rb::errors::ReplyOrIdError;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::xcb_ffi::XCBConnection;
use x11rb::protocol::xproto::ConnectionExt;
use x11rb_protocol::protocol::xproto::*;
use x11rb_protocol::protocol::Event;

use clap::Parser;
use std::path::PathBuf;

use fork::{daemon, Fork};

use tabbed_rs::*;
use tabbed_rs::config::*;
use tabbed_rs::x11::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    close: bool,
    #[arg(short, long)]
    detach: bool,
    #[arg(long)]
    config: Option<PathBuf>,
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let config = match read_config(&cli.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            return Ok(());
        }
    };

    let (conn, screen_num) = XCBConnection::connect(None)?;
    // We leak the connection so it doesn't get dropped when detaching,
    // since it lives for the duration of the entire program anyway.
    let conn = Box::leak(Box::new(conn));

    let screen = &conn.setup().roots[screen_num];
    let atoms = Atoms::new(conn)?.reply()?;


    let event_mask = EventMask::BUTTON_PRESS
        | EventMask::SUBSTRUCTURE_NOTIFY
        | EventMask::STRUCTURE_NOTIFY
        | EventMask::FOCUS_CHANGE
        | EventMask::EXPOSURE;

    let win_id = rs_create_window(
        conn,
        screen,
        &atoms,
        event_mask,
        TABBED_WINDOW_CLASS,
        (200, 200),
    )?;

    conn.flush()?;

    println!("0x{:X}", win_id);

    if cli.detach {
        if let Ok(Fork::Parent(_)) = daemon(false, false) {
            return Ok(());
        }
    }

    for keybind in &config.keybinds {
        conn.grab_key(
            true,
            win_id,
            keybind.mod_mask(),
            keybind.key,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        )?.check()?;
    }

    let mut tabbed = Tabbed::new(&conn, &atoms, &cli, config, screen, win_id)?;

    while tabbed.running {
        let event = conn.wait_for_event()?;
        println!("Event: {:?}\n", event);

        match event {
            Event::KeyPress(e) => tabbed.handle_key_press(e),
            Event::ReparentNotify(e) => tabbed.handle_reparent_notify(e),
            Event::DestroyNotify(e) => tabbed.handle_destroy_notify(e),
            Event::ConfigureNotify(e) => tabbed.handle_configure_notify(e),
            Event::MapNotify(e) => tabbed.handle_map_notify(e),
            Event::PropertyNotify(e) => tabbed.handle_property_notify(e),
            Event::ButtonPress(e) => tabbed.handle_button_press(e),
            Event::ClientMessage(e) => {
                let data = e.data.as_data32();
                if e.format == 32 && e.window == win_id && data[0] == atoms.WM_DELETE_WINDOW {
                    tabbed.running = false;
                }
            }
            Event::FocusIn(_) => tabbed.is_focused = true,
            Event::FocusOut(_) => tabbed.is_focused = false,
            _e => {
                //println!("Unhandled Event: {:?}", _e);
            }
        }

        if tabbed.need_redraw {
            tabbed.drawbar()?;
            tabbed.need_redraw = false;
        }
        conn.sync()?;
    }

    tabbed.cleanup()?;

    Ok(())
}




struct Tabbed<'a> {
    conn: &'a XCBConnection,
    atoms: &'a Atoms,
    cli: &'a Cli,
    config: Config,
    screen: &'a Screen,
    win_id: Window,
    win_width: u16,
    win_height: u16,
    children: Vec<Window>,
    child_names: HashMap<Window, String>,
    focused: Option<usize>,
    is_focused: bool,
    cairo_surface: cairo::XCBSurface,
    running: bool,
    need_redraw: bool,
}

impl<'a> Tabbed<'a> {
    fn new(
        conn: &'a XCBConnection,
        atoms: &'a Atoms,
        cli: &'a Cli,
        config: Config,
        screen: &'a Screen,
        win_id: Window,
    ) -> Result<Self, ReplyOrIdError> {
        let geometry = conn.get_geometry(win_id).unwrap().reply().unwrap();


        let visualid = screen.root_visual;
        let mut visual = find_xcb_visualtype(conn, visualid).unwrap();
        let visual = unsafe { cairo::XCBVisualType::from_raw_none(&mut visual as *mut _ as _) };

        let cairo_conn =
            unsafe { cairo::XCBConnection::from_raw_none(conn.get_raw_xcb_connection() as _) };

        let surface = cairo::XCBSurface::create(
            &cairo_conn,
            &cairo::XCBDrawable(win_id),
            &visual,
            geometry.width.into(),
            geometry.height.into(),
        ).unwrap();


        Ok(Self {
            conn,
            atoms,
            cli,
            config,
            screen,
            win_id,
            win_width: geometry.width,
            win_height: geometry.height,
            children: vec![],
            child_names: HashMap::new(),
            focused: None,
            is_focused: true,
            cairo_surface: surface,
            running: true,
            need_redraw: true,
        })
    }


    fn swap_focused_relative(&mut self, offset: i32) {
        let len = self.children.len();

        let target = self.focused
            .and_then(|i| (i as i32 + offset).checked_rem_euclid(len as i32).map(|i| i as usize));

        if let (Some(a), Some(b)) = (self.focused, target) {
            self.children.swap(a, b);
            self.focus(Some(b));
        }
    }

    fn cycle_focus_relative(&mut self, offset: i32) {
        let len = self.children.len();
        self.focus(
            self.focused
                .and_then(|i| (i as i32 + offset).checked_rem_euclid(len as i32).map(|i| i as usize)),
        );
    }

    fn cycle_focus_down(&mut self) {
        self.cycle_focus_relative(-1);
    }

    fn cycle_focus_up(&mut self) {
        self.cycle_focus_relative(1);
    }


    fn focus(&mut self, focused: Option<usize>) {
        if let Some(i) = focused {
            if i >= self.children.len() {
                return;
            }
        }
        if self.focused != focused {
            self.need_redraw = true;
        }
        self.focused = focused;
        if let Some(i) = self.focused {
            self.conn
                .configure_window(
                    self.children[i],
                    &ConfigureWindowAux::new()
                        .stack_mode(StackMode::ABOVE)
                        .x(0)
                        .y(20)
                        .width(self.win_width as u32)
                        .height(self.win_height as u32 - 20)
                        .border_width(0),
                ).unwrap();

            self.conn.set_input_focus(
                InputFocus::PARENT,
                self.children[i],
                CURRENT_TIME)
                .unwrap();
        }
    }

    fn drawbar(&self) -> Result<(), cairo::Error> {
        let cr =
            cairo::Context::new(&self.cairo_surface)?;

        cr.select_font_face(
            "monospace",
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal
        );
        cr.set_font_size(12.);

        let tab_width = self.win_width as f64 / self.children.len() as f64;
        println!("{:?}", tab_width);
        if tab_width.is_infinite() {
            cr.set_source_rgb(0.5, 0.5, 0.5);
            cr.rectangle(0., 0., self.win_width as _, 20.);
            cr.fill()?;
        }
        
        for i in 0..self.children.len() {
            let name = self.child_names.get(&self.children[i]).map_or("", String::as_str);
            let tab_x = i as f64 * tab_width;
            let (r, g, b) = color_hash(&self.children[i]);

            let (bg_bright, outline_height) =
                if self.focused == Some(i) { (0.0, 14.) } else { (0.2, 0.) };
            
            cr.set_source_rgb(bg_bright, bg_bright, bg_bright);
            cr.rectangle(tab_x, 0., tab_width, 20.);
            cr.fill()?;

            cr.set_source_rgb(r.max(0.25), g.max(0.25), b.max(0.25));
            cr.rectangle(tab_x+3., 3., tab_width-6., outline_height);
            cr.set_line_width(2.);
            cr.stroke()?;

            cr.set_source_rgb(1., 1., 1.);
            cr.move_to(tab_x+5., 13.5);
            cr.show_text(&name)?;
            cr.stroke()?;
            
        }

        self.cairo_surface.flush();
        Ok(())
    }

    fn manage(&mut self, wid: Window) {
        self.children.push(wid);
        self.focus(Some(self.children.len() - 1));

        self.conn
            .change_window_attributes(
                wid,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
            ).unwrap();

        self.check_name(wid);

        self.need_redraw = true;
    }

    fn unmanage(&mut self, wid: Window) {
        let maybe_index = self.children.iter().position(|&w| w == wid);
        if let Some(index) = maybe_index {
            self.children.remove(index);
        }

        if self.cli.close && self.children.is_empty() {
            self.running = false;
        }

        if self.focused >= maybe_index {
            self.cycle_focus_down();
        }

        self.need_redraw = true;
    }

    fn handle_key_press(&mut self, event: KeyPressEvent) {
        let maybe_action = self.config.keybinds
            .iter()
            //.find(|Keybind { modifiers, key, .. }|
            //      modifiers == &u32::from(event.state) && key == &event.detail)
            .find(|keybind|
                  keybind.key == event.detail && keybind.key_but_mask() == event.state)
            .map(|Keybind { action, .. }| action.clone());

        if let Some(action) = maybe_action {
            self.do_action(&action);
        }
    }

    fn do_action(&mut self, action: &Action) {
        match action {
            Action::FocusUp => { self.cycle_focus_up(); },
            Action::FocusDown => { self.cycle_focus_down(); },
            Action::ShiftUp => { self.swap_focused_relative(1); },
            Action::ShiftDown => { self.swap_focused_relative(-1); },
            Action::Focus(index) => { self.focus(Some(*index)); },
            Action::DetachFocused => { self.detach_focused().unwrap(); },
            Action::DetachAll => { self.detach_all().unwrap(); },
            Action::ToggleAutoAttach => {},
        }
    }


    fn detach_focused(&mut self) -> Result<(), ReplyError> {
        if let Some(index) = self.focused {
            let active = self.children[index];
            self.conn.reparent_window(active, self.screen.root, 0, 0)?.check()?;
        }
        Ok(())
    }

    fn detach_all(&mut self) -> Result<(), ReplyError> {
        for &wid in &self.children {
            self.conn.reparent_window(wid, self.screen.root, 0, 0)?.check()?;
        }
        Ok(())
    }


    fn handle_reparent_notify(&mut self, event: ReparentNotifyEvent) {
        if event.parent == self.win_id {
            if let Some(index) = self.children.iter().position(|&w| w == event.window) {
                self.focus(Some(index));
            } else {
                self.manage(event.window);
            }
        } else {
            self.unmanage(event.window);
        }
    }

    fn handle_destroy_notify(&mut self, event: DestroyNotifyEvent) {
        self.unmanage(event.window);
    }

    fn handle_configure_notify(&mut self, event: ConfigureNotifyEvent) {
        if event.window == self.win_id {
            if (event.width, event.height) == (self.win_width, self.win_height) {
                return;
            };
            self.win_width = event.width;
            self.win_height = event.height;

            self.cairo_surface
                .set_size(event.width as _, event.height as _)
                .unwrap();

            self.focus(self.focused);
            self.need_redraw = true;
        }
    }

    fn handle_map_notify(&mut self, event: MapNotifyEvent) {
        if event.window == self.win_id && self.focused.is_some() {
            self.need_redraw = true;
        }
    }

    fn handle_property_notify(&mut self, event: PropertyNotifyEvent) {
        if event.window == self.screen.root {
            let reply = self.conn.get_atom_name(event.atom).unwrap().reply().unwrap();
            println!("{:?}", std::str::from_utf8(&reply.name));
        }
        if self.children.contains(&event.window) && event.atom == AtomEnum::WM_NAME.into() {
            self.check_name(event.window);
        }
    }

    fn handle_button_press(&mut self, event: ButtonPressEvent) {
        let ButtonPressEvent { event_x, event_y, .. } = event;

        if self.children.is_empty() || event_y > 20 {
            return;
        }

        let tab_width = self.win_width as f64 / self.children.len() as f64;
        let index = (event_x as f64 / tab_width).floor() as usize;
        self.focus(Some(index));
    }

    fn check_name(&mut self, wid: Window) {
        let new_name = rs_get_window_name(self.conn, self.atoms, wid).unwrap_or_default();
        let old_name = self.child_names.insert(wid, new_name.clone());

        if old_name != Some(new_name) {
            self.need_redraw = true;
        }
    }


    fn cleanup(&mut self) -> Result<(), ReplyError> {
        for &wid in &self.children {
            let reply: Vec<u32> = rs_get_window_property32(self.conn, self.atoms.WM_PROTOCOLS, wid)?;

            if reply.contains(&self.atoms.WM_DELETE_WINDOW) {
                rs_send_delete_window_event(self.conn, self.atoms, wid)?;
            }
        }

        self.running = false;
        self.conn.sync()
    }
}

