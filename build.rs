// Copyright 2020 Dubiousjim <dubiousjim@gmail.com>. All rights reserved. MIT license.
// Build script reference: https://doc.rust-lang.org/cargo/reference/build-scripts.html
// and: https://doc.rust-lang.org/cargo/reference/build-script-examples.html
//
// This one draws from https://github.com/schultyy/os_type
// and https://benohead.com/blog/2015/01/28/linux-check-glibc-version/

use std::convert::AsRef;
use std::path::Path;
use std::process::Command;

#[path = "src/bytes.rs"]
mod bytes;
use bytes::{get_words, pop_newline, read_all, strip_quotes, trim_start};

#[allow(dead_code)]
fn as_singleton<T: Copy>(value: Vec<T>) -> Option<T> {
  if value.len() == 1 {
    Some(value[0])
  } else {
    None
  }
}

#[allow(dead_code)]
fn as_pair<T: Copy>(value: Vec<T>) -> Option<(T, T)> {
  if value.len() == 2 {
    Some((value[0], value[1]))
  } else {
    None
  }
}

#[allow(dead_code)]
fn as_triple<T: Copy>(value: Vec<T>) -> Option<(T, T, T)> {
  if value.len() == 3 {
    Some((value[0], value[1], value[2]))
  } else {
    None
  }
}

#[allow(dead_code)]
fn path_exists<P: AsRef<Path>>(path: P) -> bool {
  // also Path.is_file, is_dir
  let metadata = std::fs::metadata(path);
  match metadata {
    Ok(m) => m.is_dir() || m.is_file(),
    Err(_) => false,
  }
}

#[allow(dead_code)]
fn handle_output<F, T>(cmd: &mut std::process::Command, f: F) -> Option<T>
where
  F: FnOnce(Vec<u8>) -> Option<T>,
{
  cmd.output().ok().and_then(|output| {
    if output.status.success() {
      f(output.stdout)
    } else {
      None
    }
  })
}

fn version_info() -> Option<(Vec<u8>, Option<Vec<u8>>)> {
  let os_release = if path_exists("/etc/os-release") {
    Some("/etc/os-release")
  } else if path_exists("/usr/lib/os-release") {
    Some("/usr/lib/os-release")
  } else {
    None
  };
  const NAME: &[u8] = b"NAME=";
  const VERSION_ID: &[u8] = b"VERSION_ID=";
  const PRODUCT_NAME: &[u8] = b"ProductName:";
  const PRODUCT_VERSION: &[u8] = b"ProductVersion:";
  match os_release {
    Some(release_path) => read_all(release_path).ok().and_then(|stdout| {
      let mut lines = stdout.split(|c| *c == 10 || *c == 13);
      loop {
        match lines.next() {
          Some(line) => {
            if line.starts_with(NAME) {
              let value = line.split_at(NAME.len()).1;
              break Some(strip_quotes(value));
            }
          }
          None => break None,
        }
      }
      .map(|name| {
        let name = name.to_owned();
        loop {
          match lines.next() {
            Some(line) => {
              if line.starts_with(VERSION_ID) {
                let value = line.split_at(VERSION_ID.len()).1;
                break (name, Some(strip_quotes(value).to_owned()));
              }
            }
            None => break (name, None),
          }
        }
      })
    }),
    None => handle_output(&mut Command::new("sw_vers"), |bytes| {
      let mut lines = bytes.split(|c| *c == 10 || *c == 13);
      let line1 = lines.next().expect("short output from sw_vers");
      let line2 = lines.next().expect("short output from sw_vers");
      if line1.starts_with(PRODUCT_NAME) {
        let name = trim_start(line1.split_at(PRODUCT_NAME.len()).1).to_owned();
        if line2.starts_with(PRODUCT_VERSION) {
          let version = trim_start(line2.split_at(PRODUCT_VERSION.len()).1).to_owned();
          Some((name, Some(version)))
        } else {
          Some((name, None))
        }
      } else {
        None
      }
    }),
  }
}

fn which(bin: &str) -> Option<Vec<u8>> {
  handle_output(Command::new("sh").args(&["-c", &format!("command -v {}", bin)]), |bytes| {
    Some(pop_newline(bytes))
  })
}

fn libc_info() -> Option<String> {
  /*
  let _: Option<()> = handle_output(Command::new("ldd").args(&["--version"]), |bytes| {
    println!("output of ldd --version=<{}>", std::str::from_utf8(&bytes).unwrap());
    None
  });
  */

  which("rustc")
    .and_then(|rustc| {
      // println!("location of rustc={:?}", std::str::from_utf8(&rustc).unwrap()); // FIXME
      let rustc: &std::ffi::OsStr = std::os::unix::ffi::OsStrExt::from_bytes(&rustc);
      handle_output(Command::new("ldd").args(&[rustc]), |bytes| {
        // println!("output of ldd rustc={:?}", std::str::from_utf8(&bytes).unwrap()); // FIXME
        let mut lines = bytes.split(|c| *c == 10 || *c == 13);
        loop {
          match lines.next() {
            Some(line) => match get_words(&line, &[1, 3]).and_then(as_pair) {
              Some((u, v)) if u.starts_with(b"libc.") => return Some(v.to_vec()),
              _ => (),
            },
            None => return None,
          }
        }
      })
    })
    // if ldd wasn't patched, can skip preceding and just replace libc below with "ldd"
    .and_then(|libc| {
      // println!("loc of libc={:?}", std::str::from_utf8(&libc).unwrap()); // FIXME
      let libc: &std::ffi::OsStr = std::os::unix::ffi::OsStrExt::from_bytes(&libc);
      handle_output(Command::new(libc).args(&["--version"]), |bytes| {
        // println!("output of libc --version=<{}>", std::str::from_utf8(&bytes).unwrap()); // FIXME
        String::from_utf8(bytes).ok()
      })
    })
}

fn parse_version(ver: &str) -> (u32, u32) {
  let mut pieces = ver.split('.');
  /*
  let major = u32::from_str_radix(pieces.next().unwrap(), 10).unwrap();
  let minor = u32::from_str_radix(pieces.next().unwrap(), 10).unwrap();
  */
  use std::str::FromStr;
  let major = u32::from_str(pieces.next().unwrap()).unwrap();
  let minor = u32::from_str(pieces.next().unwrap()).unwrap();
  (major, minor)
}

#[allow(clippy::assertions_on_constants)]
fn main() {
  /*
  println!("cargo:rerun-if-env-changed=OPENSSL_VERSION_NUMBER");
  if let Ok(v) = std::env::var("OPENSSL_VERSION_NUMBER") {
    let version = u64::from_str_radix(&v, 16).unwrap();
    ...
    println!(r#"cargo:rustc-cfg=feature="SSL...""#);
    // now crate can test cfg!(feature = "SSL...")
  }
  */
  println!("cargo:rerun-if-changed=build.rs");
  println!("cargo:rerun-if-changed=src/bytes.rs");
  match version_info() {
    Some((name, Some(ver))) if &name == b"Mac OS X" => {
      assert!(cfg!(target_os = "macos"));
      // ver = b"10.10.5"
      let (major, minor) = parse_version(std::str::from_utf8(&ver).expect("bad utf8 in ProductVersion"));
      assert!(major == 10 && minor >= 10);
      for i in 10..=minor {
        println!("cargo:rustc-cfg=MACOS_ATLEAST_10_{}", i);
      }
      println!("cargo:rustc-env=CARGO_BUILTFOR=mac_{}.{}", major, minor);
    }
    Some((name, _)) => {
      println!(
        "cargo:rustc-env=CARGO_BUILTFOR={} os-release",
        String::from_utf8(name).unwrap()
      );
    }
    // _ => { println!("cargo:rustc-env=CARGO_BUILTFOR=unknown"); }
    _ => {}
  }
  if cfg!(target_os = "linux") {
// ldd --version: ldd (Ubuntu GLIBC 2.23-0ubuntu11) 2.23\n ...
// libc --version: GNU C Library (Ubuntu GLIBC 2.23-0ubuntu11) stable release version 2.23, ...

    if let Some(version) = libc_info() {
      if version.starts_with("GNU C Library (GNU libc) ") {
        assert!(cfg!(target_env = "gnu"));
        let start = version.find("version").expect("no version") + 8;
        let after = version.find(".\n").expect("no newline");
        let (major, minor) = parse_version(&version[start..after]);
        assert!(
          major > 2 || (major == 2 && minor >= 15),
          "expected glibc >= 2.15 (March 2012)"
        );
        assert!(major == 2, "glibc > 2.x");
        for i in 15..100 {
          if major == 2 && minor >= i {
            println!("cargo:rustc-cfg=GLIBC_ATLEAST_2_{}", i);
          }
        }
        println!("cargo:rustc-env=CARGO_BUILTFOR=glibc_{}.{}", major, minor);
      } else if version.starts_with("musl libc ") {
        assert!(cfg!(target_env = "musl"));
        let start = version.find("Version").expect("no version") + 8;
        let after = version[start..].find('\n').expect("no newline");
        let version = &version[start..start + after].replacen(".", "0", 1);
        let (major, minor) = parse_version(&version);
        assert!(major >= 101, "expected musl >= 1.1.0 (April 2014)");
        if major >= 102 {
          assert!(major == 102, "musl libc > 1.2.x");
          for i in 0..20 {
            if minor >= i {
              println!("cargo:rustc-cfg=MUSL_ATLEAST_1_2_{}", i);
            }
          }
        } else {
          for i in 0..100 {
            if minor >= i {
              println!("cargo:rustc-cfg=MUSL_ATLEAST_1_1_{}", i);
            }
          }
        }
        println!("cargo:rustc-env=CARGO_BUILTFOR=musl_1.{}.{}", major, minor);
      } else {
        // assert!(cfg!(not(target_env = "gnu")), "didn't recognize glibc");
        // assert!(cfg!(not(target_env = "musl")), "didn't recognize musl libc");
        println!("cargo:rustc-env=CARGO_BUILTFOR=unknown_libc");
        println!("libc_info={:?}", version); // FIXME
      }
      // } else {
      //  println!("cargo:rustc-env=CARGO_BUILTFOR=libc_info_was_None");
    }
  }
}
