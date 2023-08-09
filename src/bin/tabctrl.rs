use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::errors::ReplyError;
use x11rb::rust_connection::RustConnection;


use std::process::{Command, Stdio};
use std::io::BufRead;


use clap::{Parser, Subcommand};
use clap_num::maybe_hex;


use nonempty::{NonEmpty, nonempty};


use tabbed_rs::*;


/// Utility functions to manipulate a tabbed window.
/// All input window ids can be in decimal, hex with the prefix "0x", or the string "focused" to
/// apply to the currently focused window.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Reparent a set of windows to a tabbed instance, creating one if necessary
    Create {
        // Window IDs to combine into a tabbed instance
        #[arg(num_args=1.., value_parser=maybe_hex::<Window>)]
        wids: Vec<Window>,
    },
    /// Attach window <WID0> to tabbed <WID1>.
    ///
    /// If <WID0> is tabbed, use the active window instead.
    /// If <WID1> is not tabbed, call `create <WID1>` first.
    Transfer {
        #[arg(value_parser=maybe_hex::<Window>)]
        wid0: Window,
        #[arg(value_parser=maybe_hex::<Window>)]
        wid1: Window,
    },
    /// Detach from a tabbed container; by default, detaches active window only
    Detach {
        /// Window to detach from, expected to be a tabbed instance, no-op otherwise
        #[arg(value_parser=maybe_hex::<Window>)]
        wid: Window,

        /// Detach all children of the window instead of only active; deletes the tabbed instance
        #[arg(short,long)]
        all: bool,
    },
    Query {
        #[arg(value_parser=maybe_hex::<Window>)]
        wid: Window,
    },
    /// Embed the next opened program with the target window
    Embed {
        /// Target window to autoattach to once
        #[arg(value_parser=maybe_hex::<Window>)]
        wid: Window,
    },
}




fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let (conn, screen_num) = x11rb::connect(None).unwrap();
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    match cli.command {
        Commands::Create { wids } => {
            let wids = NonEmpty::from_vec(wids)
                .expect("create args cannot be empty");
            create(&conn, wids)?;
        },
        Commands::Transfer { wid0, wid1 } => {
            transfer(&conn, wid0, wid1)?;
        },
        Commands::Detach { wid, all: true } => {
            reparent_all(&conn, wid, root)?;
        },
        Commands::Detach { wid, all: false } => {
            reparent_current(&conn, wid, root)?;
        },
        Commands::Query { wid } => {
            query(&conn, wid)?;
        },
        Commands::Embed { wid } => {
            embed(&conn, wid)?;
        },
    }

    conn.flush()?;
    Ok(())
}


fn create(conn: &RustConnection, wids: NonEmpty<Window>) -> Result<Window, ReplyError> {
    let mut to_reparent = Vec::new();

    for &w in wids.iter().take(wids.len() - 1) {
        if is_tabbed(conn, w)? {
            let mut q = conn.query_tree(w)?.reply()?;
            to_reparent.append(&mut q.children);
        } else {
            to_reparent.push(w);
        }
    }

    let &last = wids.last();
    bspc_focus(last);

    // If the last window is tabbed, use it. Otherwise, spawn a new tabbed and use that
    let tabbed = if is_tabbed(conn, last)? {
        last
    } else {
        to_reparent.push(last);
        create_tabbed()
    };

    for &w in &to_reparent {
        conn.reparent_window(w, tabbed, 0, 0)?.check()?;
    }

    conn.flush()?;

    Ok(tabbed)
}


fn transfer(conn: &RustConnection, wid0: Window, wid1: Window) -> Result<(), ReplyError> {
    let tabbed_window = create(conn, nonempty![wid1])?;
    if is_tabbed(conn, wid0)? {
        reparent_current(conn, wid0, tabbed_window)?;
    } else {
        conn.reparent_window(wid0, tabbed_window, 0, 0)?.check()?;
    }
    bspc_focus(tabbed_window);
    Ok(())
}


fn embed(conn: &RustConnection, wid: Window) -> Result<(), ReplyError> {
    let child = Command::new("bspc")
        .args(["subscribe", "node_add"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let output = child.stdout.unwrap();
    let mut lines = std::io::BufReader::new(output).lines();

    if let Some(Ok(text)) = lines.next() {
        let parts: Vec<_> = text.split_whitespace().collect();
        let id_str = parts[4].strip_prefix("0x").unwrap().trim();
        let new_wid = Window::from_str_radix(id_str, 16).unwrap();

        let tabbed_window = create(conn, nonempty![wid])?;
        conn.reparent_window(new_wid, tabbed_window, 0, 0)?.check()?;
        bspc_focus(tabbed_window);
    }

    Ok(())
}


fn is_tabbed(conn: &RustConnection, wid: Window) -> Result<bool, ReplyError> {
    let prop = conn.get_property(false, wid, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 8);
    Ok(prop?.reply()?.value == TABBED_WINDOW_CLASS.as_bytes())
}


fn reparent_all(conn: &RustConnection, wid0: Window, wid1: Window) -> Result<Vec<Window>, ReplyError> {
    let q = conn.query_tree(wid0)?.reply()?;

    for &w in &q.children {
        conn.reparent_window(w, wid1, 0, 0)?.check()?;
    }

    Ok(q.children)
}


fn reparent_current(conn: &RustConnection, wid0: Window, wid1: Window) -> Result<Option<Window>, ReplyError> {
    let q = conn.query_tree(wid0)?.reply()?;
    if let Some(&active) = q.children.last() {
        conn.reparent_window(active, wid1, 0, 0)?.check()?;
        Ok(Some(active))
    } else {
        Ok(None)
    }
}


fn query(conn: &RustConnection, wid: Window) -> Result<(), ReplyError> {
    println!("wid: {} 0x{:X}", wid, wid);
    println!("is_tabbed: {}", is_tabbed(conn, wid)?);
    println!("children: {:?}", conn.query_tree(wid)?.reply()?.children);
    Ok(())
}


fn create_tabbed() -> Window {
    let child = Command::new("tabbed-rs")
        .args(["-c", "-d"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to execute command");

    let output = child.wait_with_output().unwrap();
    let id_str = std::str::from_utf8(&output.stdout).unwrap()
        .strip_prefix("0x").unwrap().trim();

    Window::from_str_radix(id_str, 16).unwrap()
}

fn bspc_focus(wid: Window) {
    Command::new("bspc")
        .args(["node", &wid.to_string(), "--focus"])
        .status()
        .expect("failed to execute bspc node");
}

