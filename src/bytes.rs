// Copyright 2020 Dubiousjim <dubiousjim@gmail.com>. All rights reserved. MIT license.
#![allow(dead_code)]

use std::convert::AsRef;
use std::path::Path;

pub fn strip_quotes(value: &[u8]) -> &[u8] {
  let len = value.len();
  if len >= 2 && value[0] == b'"' && value[len - 1] == b'"' {
    return &value[1..len - 1];
  }
  value
}

pub fn trim_start(value: &[u8]) -> &[u8] {
  let mut i = 0;
  for j in value.iter() {
    if !j.is_ascii_whitespace() {
      break;
    }
    i += 1;
  }
  &value[i..]
}

pub fn get_word(value: &[u8], word: usize) -> Option<&[u8]> {
  let mut start = 0;
  let mut after = 0;
  let mut inword = false;
  let mut words_started: usize = 0;
  for (i, b) in value.iter().enumerate() {
    if b.is_ascii_whitespace() {
      if inword {
        inword = false;
        if words_started == word {
          return Some(&value[start..after]);
        }
      }
    } else if !inword {
      inword = true;
      start = i;
      words_started += 1;
    }
    if inword {
      after = i + 1
    }
  }
  if inword {
    Some(&value[start..after])
  } else {
    None
  }
}

pub fn get_words<'a>(value: &'a [u8], words: &[usize]) -> Option<Vec<&'a [u8]>> {
  let mut start = 0;
  let mut after = 0;
  let mut inword = false;
  let mut res: Vec<Option<&[u8]>> = vec![None; words.len()];
  let max_word = words.iter().max().expect("requested empty list of words");
  let mut words_started: usize = 0;
  for (i, b) in value.iter().enumerate() {
    if b.is_ascii_whitespace() {
      if inword {
        inword = false;
        for (ji, jw) in words.iter().enumerate() {
          if *jw == words_started {
            res[ji] = Some(&value[start..after]);
          }
        }
        if words_started == *max_word {
          break;
        }
      }
    } else if !inword {
      inword = true;
      start = i;
      words_started += 1;
    }
    if inword {
      after = i + 1
    }
  }
  if inword && words_started == *max_word {
    for (ji, jw) in words.iter().enumerate() {
      if *jw == words_started {
        res[ji] = Some(&value[start..after]);
      }
    }
  }
  // let res: Option<Vec<&[u8]>> = res.iter().copied().collect();
  res.iter().copied().collect()
}

pub fn strip_newline(value: &[u8]) -> &[u8] {
  let len = value.len();
  if len >= 1 && (value[len - 1] == 10 || value[len - 1] == 13) {
    return &value[..len - 1];
  }
  value
}

pub fn pop_newline(mut value: Vec<u8>) -> Vec<u8> {
  match value.pop() {
    Some(b) if b != 10 => value.push(b),
    _ => (),
  }
  value
}

pub fn read_all<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<u8>> {
  use std::fs::File;
  use std::io::Read;
  let mut f = File::open(path)?;
  let mut buffer = Vec::new();
  f.read_to_end(&mut buffer)?;
  Ok(buffer)
}
