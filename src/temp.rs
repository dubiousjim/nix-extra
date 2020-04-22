// Copyright 2020 Dubiousjim <dubiousjim@gmail.com>. All rights reserved. MIT license.
#![allow(dead_code)]


use nix::{errno::Errno, Error, /*NixPath,*/ Result};
use std::os::unix::io::RawFd;

use std::ffi::{CStr, OsString};
use std::path::PathBuf;

#[inline]
// pub fn with_mkstempat<P: ?Sized + NixPath, F>(prefix: &CStr, suffix: Option<&CStr>, f: F) -> Result<(RawFd, PathBuf)>
pub fn with_mkstempat<F>(prefix: &CStr, suffix: Option<&CStr>, f: F) -> Result<(RawFd, PathBuf)>
where
  F: Fn(&[u8]) -> Result<RawFd>,
{
  let prefix_bytes: &[u8] = prefix.to_bytes();
  let mut path_len = prefix_bytes.len() + 7;
  let suffix_bytes: Option<&[u8]> = suffix.map(|cstr| {
    let bytes = cstr.to_bytes();
    path_len += bytes.len();
    bytes
  });
  const STEP: u64 = 5_958_002; // 7777
  let mut tries = 32 * 32 * 32 * 8;
  /*
  let mut rng = rand:thread_rng();
  use rand::Rng;
  ...
  let unique: u32 = rng.gen();
  path_vec.append(&mut format!("{:08x}", unique).into_bytes());
  */
  /*
  let mut rng = rand:thread_rng();
  use rand::distributions::{Distribution, Uniform};
  static LETTERS: &[u8] = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".as_bytes();
  let between = Uniform::<usize>::new(0, 62);
  ...
  path_vec.push(LETTERS[between.sample(rng)]); // 6 times
  */
  let pid = std::process::id() as u64;
  use crate::time::{ClockLike, Duration};
  let mut random = Duration::now().as_rand_bits() ^ pid;
  let mut path_vec = Vec::<u8>::with_capacity(path_len);
  let fd = loop {
    path_vec.extend_from_slice(prefix_bytes);
    path_vec.push(b'.');
    let mut r = random;
    path_vec.push(('A' as u64 + (r & 15) + (r & 16) * 2) as u8);
    r >>= 5;
    path_vec.push(('A' as u64 + (r & 15) + (r & 16) * 2) as u8);
    r >>= 5;
    path_vec.push(('A' as u64 + (r & 15) + (r & 16) * 2) as u8);
    r >>= 5;
    path_vec.push(('A' as u64 + (r & 15) + (r & 16) * 2) as u8);
    r >>= 5;
    path_vec.push(('A' as u64 + (r & 15) + (r & 16) * 2) as u8);
    r >>= 5;
    path_vec.push(('A' as u64 + (r & 15) + (r & 16) * 2) as u8);
    if let Some(bytes) = suffix_bytes {
      path_vec.extend_from_slice(bytes)
    }
    match f(&path_vec) {
      Err(ref e) if e.as_errno() == Some(Errno::EEXIST) => (),
      Ok(fd) => break Ok(fd),
      Err(e) => break Err(e),
    }
    path_vec.clear();
    random = random.wrapping_add(STEP ^ pid);
    tries -= 1;
    if tries == 0 {
      break Err(Error::Sys(Errno::EEXIST));
    }
  }?;
  use std::os::unix::ffi::OsStringExt;
  let path = PathBuf::from(OsString::from_vec(path_vec));
  Ok((fd, path))
}

#[cfg(test)]
mod tests {
  use super::*;
  // TODO
}
