use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
    str::CharIndices,
};

use color_eyre::{
    Report, Result,
    eyre::{Context, bail, eyre},
};
use encoding_rs::WINDOWS_1252;

pub fn load_file(path: &Path) -> Result<()> {
    println!("TMP: Load QFX {:?}", path);

    let mut reader = BufReader::new(File::open(path).wrap_err("Failed to open file")?);

    // Read header
    let header = read_header(&mut reader).wrap_err("Failed to read header")?;
    if header.ofxheader != 100 {
        bail!("Unsupported header: {}", header.ofxheader);
    }
    if header.version != 102 {
        bail!("Unsupported version: {}", header.version);
    }

    // Load whole file
    let mut file_bytes = Vec::new();
    reader
        .read_to_end(&mut file_bytes)
        .wrap_err("Failed to read file")?;
    let (file_string, _, _) = WINDOWS_1252.decode(&file_bytes);

    // Parse file
    let mut lexer = QfxLexerIterator::new(&file_string.as_ref());
    let tokens = lexer
        .collect::<Result<Vec<QfxToken>>>()
        .wrap_err("TMP: Error lexing file")?;

    println!("Contents: {:?}", tokens);

    Ok(())
}

#[derive(Debug)]
enum HeaderDataType {
    OfxSgml,
}

#[derive(Debug)]
enum HeaderEncoding {
    UsaAscii,
}

#[derive(Debug)]
enum HeaderCharset {
    Windows1252,
}

#[derive(Debug)]
struct Header {
    ofxheader: u32,
    data: HeaderDataType,
    version: u32,
    encoding: HeaderEncoding,
    charset: HeaderCharset,
}

fn read_header(src: &mut BufReader<File>) -> Result<Header> {
    let mut ofxheader = None;
    let mut data = None;
    let mut version = None;
    let mut security = false;
    let mut encoding = None;
    let mut charset = None;
    let mut compression = false;
    let mut oldfileuid = false;
    let mut newfileuid = false;

    let mut line_buf = Vec::with_capacity(32);
    loop {
        line_buf.clear();
        let _ = src.read_until(b'\n', &mut line_buf)?;

        // Remove newlines
        let mut line_ref = line_buf.as_slice();
        if line_ref.ends_with(b"\n") {
            line_ref = &line_ref[..line_ref.len() - 1];
        }
        if line_ref.ends_with(b"\r") {
            line_ref = &line_ref[..line_ref.len() - 1];
        }

        if line_ref.len() == 0 {
            // Double newline means end of header
            break;
        }

        let mut header = str::from_utf8(line_ref).wrap_err_with(|| {
            format!(
                "Invalid utf8 in header: {:?}",
                String::from_utf8_lossy(line_ref)
            )
        })?;

        let mut parts = header.split(|c| c == ':');
        let key = parts
            .next()
            .expect("Non zero length line should have at least one part");
        let value = parts
            .next()
            .ok_or_else(|| eyre!("Header line missing colon: {:?}", &line_buf))?;

        match key {
            "OFXHEADER" => {
                let parsed = value.parse::<u32>().wrap_err_with(|| {
                    format!("Cannot parse header 'OFXHEADER' value: {:?}", value)
                })?;
                if ofxheader.replace(parsed).is_some() {
                    bail!("Repeated header 'OFXHEADER")
                }
            }
            "DATA" => {
                let parsed = match value {
                    "OFXSGML" => HeaderDataType::OfxSgml,
                    v => bail!("Unrecognized DATA value: {:?}", v),
                };
                if data.replace(parsed).is_some() {
                    bail!("Repeated header 'DATA")
                }
            }
            "VERSION" => {
                let parsed = value.parse::<u32>().wrap_err_with(|| {
                    format!("Cannot parse header 'VERSION' value: {:?}", value)
                })?;
                if version.replace(parsed).is_some() {
                    bail!("Repeated header 'VERSION")
                }
            }
            "SECURITY" => {
                if security {
                    bail!("Repeated header 'SECURITY");
                }
                match value {
                    "NONE" => security = true,
                    v => bail!("Unrecognized SECURITY value: {:?}", v),
                }
            }
            "ENCODING" => {
                let parsed = match value {
                    "USASCII" => HeaderEncoding::UsaAscii,
                    v => bail!("Unrecognized ENCODING value: {:?}", v),
                };
                if encoding.replace(parsed).is_some() {
                    bail!("Repeated header 'ENCODING")
                }
            }
            "CHARSET" => {
                let parsed = match value {
                    "1252" => HeaderCharset::Windows1252,
                    v => bail!("Unrecognized CHARSET value: {:?}", v),
                };
                if charset.replace(parsed).is_some() {
                    bail!("Repeated header 'CHARSET")
                }
            }
            "COMPRESSION" => {
                if compression {
                    bail!("Repeated header 'COMPRESSION");
                }
                match value {
                    "NONE" => compression = true,
                    v => bail!("Unrecognized COMPRESSION value: {:?}", v),
                }
            }
            "OLDFILEUID" => {
                if oldfileuid {
                    bail!("Repeated header 'OLDFILEUID");
                }
                match value {
                    "NONE" => oldfileuid = true,
                    v => bail!("Unrecognized OLDFILEUID value: {:?}", v),
                }
            }
            "NEWFILEUID" => {
                if newfileuid {
                    bail!("Repeated header 'NEWFILEUID");
                }
                match value {
                    "NONE" => newfileuid = true,
                    v => bail!("Unrecognized NEWFILEUID value: {:?}", v),
                }
            }
            h => bail!("Unrecognized header: {:?}", h),
        }
    }

    if !compression {
        bail!("Header 'COMPRESSION' missing");
    }
    if !oldfileuid {
        bail!("Header 'OLDFILEUID' missing");
    }
    if !newfileuid {
        bail!("Header 'NEWFILEUID' missing");
    }

    Ok(Header {
        ofxheader: ofxheader.ok_or_else(|| eyre!("Header 'OFXHEADER' missing"))?,
        data: data.ok_or_else(|| eyre!("Header 'DATA' missing"))?,
        version: version.ok_or_else(|| eyre!("Header 'VERSION' missing"))?,
        encoding: encoding.ok_or_else(|| eyre!("Header 'ENCODING' missing"))?,
        charset: charset.ok_or_else(|| eyre!("Header 'CHARSET' missing"))?,
    })
}

#[derive(Debug)]
enum QfxKey {
    Tmp,
}

#[derive(Debug)]
enum QfxToken<'a> {
    OpenKey(&'a str),
    CloseKey(&'a str),
    Value(&'a str),
}

enum QfxLexerState {
    Idle,
    CaptureKey(usize),
    CaptureCloseKey(usize),
    CaptureValue(usize),
}

struct LexError {
    msg: String,
}

struct QfxLexerIterator<'a> {
    state: QfxLexerState,
    src: &'a str,
    char_iter: CharIndices<'a>,
}

impl<'a> QfxLexerIterator<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            state: QfxLexerState::Idle,
            src,
            char_iter: src.char_indices(),
        }
    }

    fn err(&self, msg: &str, idx: usize, item_len: usize) -> Report {
        let item_start = idx - item_len;
        let start_pad = std::cmp::min(item_start, 5);
        let display_start = item_start - start_pad;
        let display_end = std::cmp::min(self.src.len(), idx + 10);
        eyre!(
            "{0}: '{1}'\n{2:3$}{4:^<5$}",
            msg,
            &self.src[display_start..display_end],
            "",
            msg.len() + 3 + start_pad + 1,
            "",
            item_len
        )
    }
}

impl<'a> Iterator for QfxLexerIterator<'a> {
    type Item = Result<QfxToken<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some((idx, char)) = self.char_iter.next() else {
                return None;
                unimplemented!();
            };
            match char {
                '<' => match self.state {
                    QfxLexerState::Idle => {
                        self.state = QfxLexerState::CaptureKey(idx);
                    }
                    QfxLexerState::CaptureKey(start) | QfxLexerState::CaptureCloseKey(start) => {
                        return Some(Err(self.err(
                            "Start of new key inside key",
                            idx,
                            idx - start,
                        )));
                    }
                    QfxLexerState::CaptureValue(start) => {
                        self.state = QfxLexerState::CaptureKey(idx);
                        return Some(Ok(QfxToken::Value(&self.src[start..idx])));
                    }
                },
                '>' => match self.state {
                    QfxLexerState::Idle | QfxLexerState::CaptureValue(_) => {
                        return Some(Err(self.err("End of key without start of key", idx, 1)));
                    }
                    QfxLexerState::CaptureKey(start) => {
                        self.state = QfxLexerState::Idle;
                        return Some(Ok(QfxToken::OpenKey(&self.src[start + 1..idx])));
                    }
                    QfxLexerState::CaptureCloseKey(start) => {
                        self.state = QfxLexerState::Idle;
                        return Some(Ok(QfxToken::CloseKey(&self.src[start + 2..idx])));
                    }
                },
                '/' => match self.state {
                    QfxLexerState::Idle | QfxLexerState::CaptureValue(_) => {}
                    QfxLexerState::CaptureKey(start) => {
                        self.state = QfxLexerState::CaptureCloseKey(start)
                    }
                    QfxLexerState::CaptureCloseKey(start) => {
                        return Some(Err(self.err("Slash in key name", idx, idx - start)));
                    }
                },
                ch => match self.state {
                    QfxLexerState::Idle => self.state = QfxLexerState::CaptureValue(idx),
                    QfxLexerState::CaptureKey(_)
                    | QfxLexerState::CaptureCloseKey(_)
                    | QfxLexerState::CaptureValue(_) => {}
                },
            }
        }
    }
}
