// Copyright 2020 Dubiousjim <dubiousjim@gmail.com>. All rights reserved. MIT license.
#![allow(dead_code)]


use nix::{errno::Errno, /*Error,*/ NixPath, Result};
use std::os::unix::io::RawFd;

use crate::open::Symlink;
pub use nix::unistd::AccessFlags;
use std::path::PathBuf;

pub fn get_accmode(fd: RawFd, want_read: bool, want_write: bool) -> Result<bool> {
  /*
  use nix::fcntl::{fcntl, FcntlArg, OFlag};
  let flags = fcntl(fd, FcntlArg::F_GETFL)?;
  let flags = OFlag::from_bits_truncate(flags);
  let mode = OFlag::O_ACCMODE & flags;
  if mode == OFlag::O_WRONLY && want_read {
    Ok(false)
  } else if mode == OFlag::O_RDONLY && want_write {
    Ok(false)
  } else {
    Ok(true)
  }
  */
  let res = unsafe { libc::fcntl(fd, libc::F_GETFL) };
  Errno::result(res).map(|flags| {
    #[cfg(target_os = "linux")]
    {
      if flags & libc::O_PATH != 0 {
        return !want_read && !want_write;
      }
    }
    let mode = libc::O_ACCMODE & flags;
    #[allow(clippy::if_same_then_else)]
    #[allow(clippy::needless_bool)]
    {
      if mode == libc::O_WRONLY && want_read {
        false
      } else if mode == libc::O_RDONLY && want_write {
        false
      } else {
        true
      }
    }
  })
}

// Based on https://github.com/rust-lang/rust/blob/master/src/libstd/sys/unix/fs.rs
// Windows Vista+ have GetFinalPathNameByHandle or GetFileInformationByHandleEx, passing FileNameInfo: https://stackoverflow.com/a/1188803/272427

pub fn get_path(fd: RawFd) -> Option<PathBuf> {
  use nix::unistd::getcwd;
  use std::ffi::OsString;
  use std::os::unix::ffi::OsStringExt;
  if fd == libc::AT_FDCWD {
    return getcwd().ok();
  }
  #[cfg(target_os = "linux")]
  {
    use libc::{c_char, size_t};
    use std::ffi::CStr;
    let mut path = PathBuf::from("/proc/self/fd");
    path.push(fd.to_string());
    // nix::fcntl::readlink(&path).ok().map(PathBuf::from)
    let mut buf = Vec::with_capacity(libc::PATH_MAX as usize);
    let mut path_vec = path.into_os_string().into_vec();
    path_vec.push(0);
    let res = unsafe {
      let cstr = CStr::from_bytes_with_nul_unchecked(&path_vec);
      libc::readlink(cstr.as_ptr(), buf.as_mut_ptr() as *mut c_char, buf.capacity() as size_t)
    };
    if res == -1 {
      None
    } else {
      unsafe { buf.set_len(res as usize) };
      Some(PathBuf::from(OsString::from_vec(buf)))
    }
  }
  #[cfg(any(target_os = "macos", target_os = "netbsd"))]
  {
    let mut buf = vec![0; libc::PATH_MAX as usize];
    let res = unsafe { libc::fcntl(fd, libc::F_GETPATH, buf.as_ptr()) };
    if res == -1 {
      None
    } else {
      let len = buf.iter().position(|&c| c == 0).unwrap();
      buf.truncate(len as usize);
      buf.shrink_to_fit();
      Some(PathBuf::from(OsString::from_vec(buf)))
    }
  }
  #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "netbsd")))]
  {
    None
  }
}

// Based on https://github.com/nix-rust/nix/pull/1134

/// Checks the file named by `path` for accessibility according to the flags given by `mode`.
///
/// If `dirfd` has a value, then `path` is relative to directory associated with the file descriptor.
///
/// If `dirfd` is `None`, then `path` is relative to the current working directory.
///
/// If `links` is `Symlink::Open` and `path` names a symbolic link,
/// then the mode of the symbolic link is queried. (On macos, not available until 10.15.)
///
/// # References
///
/// [faccessat(2)](http://pubs.opengroup.org/onlinepubs/9699919799/functions/faccessat.html)
pub fn faccessat<P: ?Sized + NixPath>(dirfd: Option<RawFd>, path: &P, mode: AccessFlags, links: Symlink) -> Result<()> {
  let flag = match links {
    Symlink::Follow => 0,
    Symlink::Open => {
      #[cfg(all(target_os = "macos", not(MACOS_ATLEAST_10_15)))]
      {
        use nix::Error;
        return Err(Error::UnsupportedOperation);
      }
      #[cfg(any(not(target_os = "macos"), MACOS_ATLEAST_10_15))]
      libc::AT_SYMLINK_NOFOLLOW
    }
    Symlink::Fail => {
      // TODO
      use nix::Error;
      return Err(Error::UnsupportedOperation);
    }
  };
  let res = path.with_nix_path(|cstr| unsafe {
    libc::faccessat(dirfd.unwrap_or(libc::AT_FDCWD), cstr.as_ptr(), mode.bits(), flag)
  })?;
  Errno::result(res).map(drop)
}

#[cfg(test)]
mod tests {
  use super::*;
  use nix::fcntl::{open, /*AtFlags,*/ OFlag};
  use nix::sys::stat::Mode;
  use std::fs::File;

  #[test]
  fn test_faccessat_none_not_existing() {
    let tempdir = tempfile::tempdir().unwrap();
    let dir = tempdir.path().join("does_not_exist.txt");
    assert_eq!(
      faccessat(None, &dir, AccessFlags::F_OK, Symlink::Follow)
        .err()
        .unwrap()
        .as_errno()
        .unwrap(),
      Errno::ENOENT
    );
    #[cfg(any(not(target_os = "macos"), MACOS_ATLEAST_10_15))]
    assert_eq!(
      faccessat(None, &dir, AccessFlags::F_OK, Symlink::Open)
        .err()
        .unwrap()
        .as_errno()
        .unwrap(),
      Errno::ENOENT
    );
  }

  #[test]
  fn test_faccessat_not_existing() {
    let tempdir = tempfile::tempdir().unwrap();
    let dirfd = open(tempdir.path(), OFlag::empty(), Mode::empty()).unwrap();
    let not_exist_file = "does_not_exist.txt";
    assert_eq!(
      faccessat(Some(dirfd), not_exist_file, AccessFlags::F_OK, Symlink::Follow)
        .err()
        .unwrap()
        .as_errno()
        .unwrap(),
      Errno::ENOENT
    );
    #[cfg(any(not(target_os = "macos"), MACOS_ATLEAST_10_15))]
    assert_eq!(
      faccessat(Some(dirfd), not_exist_file, AccessFlags::F_OK, Symlink::Open)
        .err()
        .unwrap()
        .as_errno()
        .unwrap(),
      Errno::ENOENT
    );
  }

  #[test]
  fn test_faccessat_none_file_exists() {
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("does_exist.txt");
    let _file = File::create(path.clone()).unwrap();
    assert!(faccessat(None, &path, AccessFlags::R_OK | AccessFlags::W_OK, Symlink::Follow).is_ok());
    #[cfg(any(not(target_os = "macos"), MACOS_ATLEAST_10_15))]
    assert!(faccessat(None, &path, AccessFlags::R_OK | AccessFlags::W_OK, Symlink::Open).is_ok());
  }

  #[test]
  fn test_faccessat_file_exists() {
    let tempdir = tempfile::tempdir().unwrap();
    let dirfd = open(tempdir.path(), OFlag::empty(), Mode::empty()).unwrap();
    let exist_file = "does_exist.txt";
    let path = tempdir.path().join(exist_file);
    let _file = File::create(path.clone()).unwrap();
    assert!(faccessat(
      Some(dirfd),
      &path,
      AccessFlags::R_OK | AccessFlags::W_OK,
      Symlink::Follow
    )
    .is_ok());
    #[cfg(any(not(target_os = "macos"), MACOS_ATLEAST_10_15))]
    assert!(faccessat(Some(dirfd), &path, AccessFlags::R_OK | AccessFlags::W_OK, Symlink::Open).is_ok());
  }
}
