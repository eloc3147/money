use std::borrow::Cow;
use std::cell::Cell;
use std::ops::Range;

use color_eyre::eyre::{OptionExt, Result, bail};
use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};

use crate::importer::qfx_file::header::StringEncoding;

#[derive(Debug)]
pub enum QfxToken<'a> {
    OpenKey(&'a [u8]),
    CloseKey(&'a [u8]),
    Value(Cow<'a, str>),
}

#[derive(Clone, Copy)]
enum KeyType {
    Key,
    CloseKey,
}

const fn count_leading_ascii(buf: &[u8]) -> usize {
    let mut count = 0;
    let mut bytes = buf;
    while let [first, rest @ ..] = bytes {
        if !first.is_ascii_whitespace() {
            break;
        }

        count += 1;
        bytes = rest;
    }
    count
}

const fn count_trailing_ascii(buf: &[u8]) -> usize {
    let mut count = 0;
    let mut bytes = buf;
    while let [rest @ .., last] = bytes {
        if !last.is_ascii_whitespace() {
            break;
        }

        count += 1;
        bytes = rest;
    }
    count
}

fn strip_ascii_range(buf: &[u8], range: Range<usize>) -> Range<usize> {
    let selected = &buf[range.clone()];
    let leading = count_leading_ascii(selected);
    let trailing = count_trailing_ascii(selected);
    Range {
        start: range.start + leading,
        end: range.end - trailing,
    }
}

struct TokenSearch {
    consumed: usize,
    value_range: Range<usize>,
    key_type: Option<KeyType>,
}

fn find_token(buf: &[u8]) -> Result<TokenSearch> {
    let mut key_type = None;

    for (idx, byte) in buf.iter().enumerate() {
        match byte {
            b'<' => match key_type {
                Some(KeyType::Key | KeyType::CloseKey) => {
                    bail!("Start of key inside key")
                }
                None => {
                    if idx > 0 {
                        return Ok(TokenSearch {
                            // Leave '<' for next token
                            consumed: idx,
                            value_range: Range { start: 0, end: idx },
                            key_type: None,
                        });
                    }

                    key_type = Some(KeyType::Key);
                }
            },
            b'>' => match key_type {
                Some(t) => {
                    // Do not include the '>' in the key name
                    let value_range = match t {
                        // Do not include the '<' in the key name
                        KeyType::Key => Range { start: 1, end: idx },
                        // Do not include the '</' in the key name
                        KeyType::CloseKey => Range { start: 2, end: idx },
                    };

                    return Ok(TokenSearch {
                        // Consume '>'
                        consumed: idx + 1,
                        value_range,
                        key_type,
                    });
                }
                None => bail!("End of key without start of key"),
            },
            b'/' => match key_type {
                Some(KeyType::Key) => {
                    if idx != 1 {
                        // The first key in buf should be '<', so this must the immediate next character
                        bail!("Slash in key name")
                    }

                    key_type = Some(KeyType::CloseKey);
                }
                Some(KeyType::CloseKey) => bail!("Slash in key name"),
                None => {}
            },
            _ => {}
        }
    }

    if buf.is_empty() {
        bail!("Can't find token in empty buf");
    }

    if key_type.is_some() {
        bail!("End of file in key");
    }

    Ok(TokenSearch {
        consumed: buf.len(),
        value_range: Range {
            start: 0,
            end: buf.len(),
        },
        key_type: None,
    })
}

pub struct Lexer {
    data: Vec<u8>,
    decoder: &'static Encoding,
    hide_field_close: bool,
    // State
    last_open: Cell<Option<Range<usize>>>,
    consumed: Cell<usize>,
    last_item_was_value: Cell<bool>,
}

impl<'a> Lexer {
    pub fn new(data: Vec<u8>, string_encoding: StringEncoding, hide_field_close: bool) -> Self {
        let decoder = match string_encoding {
            StringEncoding::Utf8 => UTF_8,
            StringEncoding::Windows1252 => WINDOWS_1252,
        };

        Self {
            data,
            decoder,
            hide_field_close,
            last_open: Cell::new(None),
            consumed: Cell::new(0),
            last_item_was_value: Cell::new(false),
        }
    }

    /// Read the next token from the file
    ///
    /// Warning: This must not be called again following an error.
    /// Doing so will cause the lexer to potentially repeat tokens
    pub fn next(&'a self) -> Result<Option<QfxToken<'a>>> {
        loop {
            let consumed = self.consumed.get();
            if consumed == self.data.len() {
                return Ok(None);
            }

            let search = find_token(&self.data[consumed..])?;

            let mut range = search.value_range;
            range.start += consumed;
            range.end += consumed;
            range = strip_ascii_range(&self.data, range);

            self.consumed.update(|c| c + search.consumed);

            let token = match search.key_type {
                Some(key_type) => {
                    if range.is_empty() {
                        bail!("Empty key");
                    }

                    let value = &self.data[range.clone()];
                    match key_type {
                        KeyType::Key => {
                            self.last_item_was_value.set(false);
                            self.last_open.set(Some(range));

                            QfxToken::OpenKey(value)
                        }
                        KeyType::CloseKey => {
                            // This sets last open to None
                            let last_open = self.last_open.take();
                            let hide = self.hide_field_close
                                & self.last_item_was_value.get()
                                & last_open.is_some()
                                && last_open.map(|r| &self.data[r]) == Some(&self.data[range]);

                            self.last_item_was_value.set(false);

                            if hide {
                                continue;
                            }
                            QfxToken::CloseKey(value)
                        }
                    }
                }
                None => {
                    if range.is_empty() {
                        continue;
                    }

                    self.last_item_was_value.set(true);

                    let value = self
                        .decoder
                        .decode_without_bom_handling_and_without_replacement(&self.data[range])
                        .ok_or_eyre("Failed to decode value")?;

                    QfxToken::Value(value)
                }
            };

            return Ok(Some(token));
        }
    }
}
