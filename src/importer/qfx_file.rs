// Compatible with Tangerine and Capital One QFX files

use std::borrow::Cow;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use chrono::{DateTime, FixedOffset, Local, NaiveDateTime, TimeZone};
use color_eyre::eyre::{Context, OptionExt, bail, eyre};
use color_eyre::{Report, Result};
use encoding_rs::WINDOWS_1252;
use self_cell::self_cell;

use crate::importer::{Transaction, TransactionType};

#[derive(Debug)]
struct DecodedContents<'a>(pub Cow<'a, str>);

self_cell!(
    struct FileContents {
        owner: Vec<u8>,

        #[covariant]
        dependent: DecodedContents,
    }

    impl {Debug}
);

pub struct QfxReader {
    contents: FileContents,
    is_xml: bool,
}

impl<'a> QfxReader {
    pub fn open(path: &Path) -> Result<Self> {
        let mut reader = BufReader::new(File::open(path).wrap_err("Failed to open file")?);

        // Determine header type
        let buf = reader.fill_buf().wrap_err("Failed to read file")?;
        let mut skipped = 0;
        let mut xml = None;
        for byte in buf {
            match *byte {
                b'<' => {
                    xml = Some(true);
                    break;
                }
                b if b.is_ascii_whitespace() => {}
                b if b.is_ascii_alphabetic() => {
                    xml = Some(false);
                    break;
                }
                b => bail!("Invalid character: {}", b),
            }
            skipped += 1;
        }
        reader.consume(skipped);

        let is_xml = xml.ok_or_eyre("File is empty")?;
        // Read header
        let encoding = if is_xml {
            let header = read_xml_header(&mut reader).wrap_err("Failed to read header")?;
            if header.ofxheader != 200 {
                bail!("Unsupported header: {}", header.ofxheader);
            }
            if header.version != 202 {
                bail!("Unsupported version: {}", header.version);
            }
            header.encoding
        } else {
            let header = read_sgml_header(&mut reader).wrap_err("Failed to read header")?;
            if header.ofxheader != 100 {
                bail!("Unsupported header: {}", header.ofxheader);
            }
            if header.version != 102 {
                bail!("Unsupported version: {}", header.version);
            }
            header.encoding
        };

        // Load whole file
        let mut file_bytes = Vec::new();
        reader
            .read_to_end(&mut file_bytes)
            .wrap_err("Failed to read file")?;

        let contents = FileContents::new(file_bytes, |bytes| match encoding {
            HeaderEncoding::Windows1252 => DecodedContents(WINDOWS_1252.decode(bytes).0),
            HeaderEncoding::Utf8 => DecodedContents(Cow::Borrowed(
                str::from_utf8(bytes)
                    .expect("Invalid UTF-8. Not bothering to handle this case sorry"),
            )),
        });

        Ok(Self { contents, is_xml })
    }

    pub fn read(&'a self) -> Result<QfxTransactionIter<'a>> {
        let lexer = Lexer::new(self.contents.borrow_dependent(), self.is_xml);
        let parser = DocumentParser::new(lexer);

        Ok(QfxTransactionIter { parser })
    }
}

trait PutOrElse<T> {
    fn put_or_else(&mut self, name: &str, value: Result<T>) -> Result<()>;
}

impl<T> PutOrElse<T> for Option<T> {
    fn put_or_else(&mut self, name: &str, value: Result<T>) -> Result<()> {
        match self {
            Some(_) => Err(eyre!("Duplicate key '{}'", name)),
            None => {
                *self = Some(value.wrap_err_with(|| eyre!("Error parsing key '{}'", name))?);
                Ok(())
            }
        }
    }
}

trait TrackState {
    fn set_with(&mut self, struct_name: &str, check: Result<()>) -> Result<()>;
    fn set_with_value<T>(&mut self, struct_name: &str, check: Result<T>) -> Result<()> {
        self.set_with(struct_name, check.map(|_| ()))
    }

    fn ensure_field(&self, field_name: &str) -> Result<()>;
}

impl TrackState for bool {
    fn set_with(&mut self, struct_name: &str, check: Result<()>) -> Result<()> {
        match (check, *self) {
            (Ok(()), false) => {
                *self = true;
                Ok(())
            }
            (Ok(()), true) => Err(eyre!("Duplicate struct '{}'", struct_name)),
            (Err(e), false) => {
                Err(e).wrap_err_with(|| format!("Failed to parse struct '{}'", struct_name))
            }
            (Err(e), true) => Err(e)
                .wrap_err_with(|| format!("Failed to parse duplicate struct '{}'", struct_name)),
        }
    }

    fn ensure_field(&self, field_name: &str) -> Result<()> {
        match *self {
            true => Ok(()),
            false => Err(eyre!("Missing field '{}'", field_name)),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum HeaderEncoding {
    Utf8,
    Windows1252,
}

#[derive(Debug)]
struct Header {
    ofxheader: u32,
    version: u32,
    encoding: HeaderEncoding,
}

fn read_sgml_header(src: &mut BufReader<File>) -> Result<Header> {
    let mut ofxheader = None;
    let mut data = false;
    let mut version = None;
    let mut security = false;
    let mut encoding = false;
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

        if line_ref.is_empty() {
            // Double newline means end of header
            break;
        }

        let header = str::from_utf8(line_ref).wrap_err_with(|| {
            format!(
                "Invalid utf8 in header: {:?}",
                String::from_utf8_lossy(line_ref)
            )
        })?;

        let mut parts = header.split(':');
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
                if data {
                    bail!("Repeated header 'DATA");
                }
                match value {
                    "OFXSGML" => data = true,
                    v => bail!("Unrecognized DATA value: {:?}", v),
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
                if encoding {
                    bail!("Repeated header 'ENCODING");
                }
                match value {
                    "USASCII" => encoding = true,
                    v => bail!("Unrecognized ENCODING value: {:?}", v),
                };
            }
            "CHARSET" => {
                let parsed = match value {
                    "1252" => HeaderEncoding::Windows1252,
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

    if !data {
        bail!("Header 'DATA' missing");
    }
    if !security {
        bail!("Header 'SECURITY' missing");
    }
    if !encoding {
        bail!("Header 'ENCODING' missing");
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
        ofxheader: ofxheader.ok_or_eyre("Header 'OFXHEADER' missing")?,
        version: version.ok_or_eyre("Header 'VERSION' missing")?,
        encoding: charset.ok_or_eyre("Header 'CHARSET' missing")?,
    })
}

fn read_xml_header(src: &mut BufReader<File>) -> Result<Header> {
    let mut line_buf = Vec::with_capacity(128);

    let mut encoding = None;
    let mut ofxheader = None;
    let mut version = None;
    let mut security = false;
    let mut oldfileuid = false;
    let mut newfileuid = false;

    // XML header line
    let _ = src.read_until(b'\n', &mut line_buf)?;
    {
        let line = str::from_utf8(&line_buf)
            .wrap_err_with(|| {
                format!(
                    "Invalid utf8 in header: {:?}",
                    String::from_utf8_lossy(&line_buf)
                )
            })?
            .trim_ascii();

        let xml_values = line
            .strip_prefix("<?xml")
            .ok_or_else(|| eyre!("Missing XML header start in line: {:?}", line))?
            .strip_suffix("?>")
            .ok_or_else(|| eyre!("Missing XML header end in line: {:?}", line))?;

        let kv_pairs = xml_values.split(' ');
        for kv_pair in kv_pairs {
            if kv_pair.is_empty() {
                continue;
            }

            let mut kv_parts = kv_pair.split('=');
            let key = kv_parts.next().ok_or_eyre("Missing key")?;
            let value = kv_parts
                .next()
                .ok_or_eyre("Missing value")?
                .strip_prefix('"')
                .ok_or_eyre("Value missing opening quote")?
                .strip_suffix('"')
                .ok_or_eyre("Value missing close quote")?;

            if kv_parts.next().is_some() {
                bail!("Unexpected data after key value pair");
            }

            match key {
                "version" => {
                    match value {
                        "1.0" => {}
                        v => {
                            bail!("Unsupported XML version: {:?}", v);
                        }
                    }
                    if value != "1.0" {
                        bail!("Unsupported XML version: {:?}", value);
                    }
                }
                "encoding" => match value {
                    "utf-8" => encoding = Some(HeaderEncoding::Utf8),
                    v => {
                        bail!("Unsupported XML version: {:?}", v);
                    }
                },
                v => {
                    bail!("Unsupported XML header key: {:?}", v);
                }
            }
        }
    }

    // OFX header line
    line_buf.clear();
    let _ = src.read_until(b'\n', &mut line_buf)?;
    let line = str::from_utf8(&line_buf)
        .wrap_err_with(|| {
            format!(
                "Invalid utf8 in header: {:?}",
                String::from_utf8_lossy(&line_buf)
            )
        })?
        .trim_ascii();

    let xml_values = line
        .strip_prefix("<?OFX")
        .ok_or_else(|| eyre!("Missing OFX header start in line: {:?}", line))?
        .strip_suffix("?>")
        .ok_or_else(|| eyre!("Missing OFX header end in line: {:?}", line))?;

    let kv_pairs = xml_values.split(' ');
    for kv_pair in kv_pairs {
        if kv_pair.is_empty() {
            continue;
        }

        let mut kv_parts = kv_pair.split('=');
        let key = kv_parts.next().ok_or_eyre("Missing key")?;
        let value = kv_parts
            .next()
            .ok_or_eyre("Missing value")?
            .strip_prefix('"')
            .ok_or_eyre("Value missing opening quote")?
            .strip_suffix('"')
            .ok_or_eyre("Value missing close quote")?;
        if kv_parts.next().is_some() {
            bail!("Unexpected data after key value pair");
        }

        match key {
            "OFXHEADER" => {
                let parsed = value.parse::<u32>().wrap_err_with(|| {
                    format!("Cannot parse header 'OFXHEADER' value: {:?}", value)
                })?;
                if ofxheader.replace(parsed).is_some() {
                    bail!("Repeated header 'OFXHEADER")
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
            h => bail!("Unrecognized OFX header key: {:?}", h),
        }
    }

    if !security {
        bail!("Header 'SECURITY' missing");
    }
    if !oldfileuid {
        bail!("Header 'OLDFILEUID' missing");
    }
    if !newfileuid {
        bail!("Header 'NEWFILEUID' missing");
    }

    Ok(Header {
        ofxheader: ofxheader.ok_or_eyre("Header 'OFXHEADER' missing")?,
        version: version.ok_or_eyre("Header 'VERSION' missing")?,
        encoding: encoding.ok_or_eyre("XML encoding missing")?,
    })
}

#[derive(Debug)]
enum QfxToken<'a> {
    OpenKey(&'a str),
    CloseKey(&'a str),
    Value(&'a str),
}

#[derive(Debug)]
enum LexerState {
    Idle,
    CaptureKey(usize),
    CaptureCloseKey(usize),
    CaptureValue(usize),
}

struct Lexer<'a> {
    state: LexerState,
    src: &'a DecodedContents<'a>,
    next_idx: usize,
    hide_field_close: bool,
    last_item_was_value: bool,
    last_open: Option<&'a str>,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a DecodedContents<'a>, hide_field_close: bool) -> Self {
        Self {
            state: LexerState::Idle,
            src,
            next_idx: 0,
            hide_field_close,
            last_item_was_value: false,
            last_open: None,
        }
    }

    fn err(&self, msg: &str, idx: usize, item_len: usize) -> Report {
        let item_start = idx - item_len;
        let start_pad = std::cmp::min(item_start, 5);
        let display_start = item_start - start_pad;
        let display_end = std::cmp::min(self.src.0.len(), idx + 10);
        eyre!(
            "{0}: '{1}'\n{2:3$}{4:^<5$}",
            msg,
            &self.src.0[display_start..display_end],
            "",
            msg.len() + 3 + start_pad + 1,
            "",
            item_len
        )
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<QfxToken<'a>>;

    fn next(&mut self) -> Option<Result<QfxToken<'a>>> {
        let mut chars = self.src.0[self.next_idx..].char_indices();

        loop {
            let Some((idx, char)) = chars.next() else {
                return match self.state {
                    LexerState::Idle => None,
                    LexerState::CaptureKey(start) | LexerState::CaptureCloseKey(start) => {
                        Some(Err(self.err(
                            "End of file in key",
                            self.src.0.len() - 1,
                            self.src.0.len() - start,
                        )))
                    }
                    LexerState::CaptureValue(start) => {
                        self.state = LexerState::Idle;
                        Some(Ok(QfxToken::Value(&self.src.0[start..])))
                    }
                };
            };
            let full_idx = self.next_idx + idx;
            match char {
                '<' => match self.state {
                    LexerState::Idle => {
                        self.state = LexerState::CaptureKey(full_idx);
                    }
                    LexerState::CaptureKey(start) | LexerState::CaptureCloseKey(start) => {
                        self.next_idx = full_idx + 1;
                        return Some(Err(self.err(
                            "Start of new key inside key",
                            full_idx,
                            full_idx - start,
                        )));
                    }
                    LexerState::CaptureValue(start) => {
                        self.state = LexerState::CaptureKey(full_idx);
                        self.last_item_was_value = true;
                        let value = &self.src.0[start..full_idx];
                        // Ignore empty values
                        if value.trim().is_empty() {
                            self.state = LexerState::CaptureKey(full_idx);
                        } else {
                            self.next_idx = full_idx + 1;
                            return Some(Ok(QfxToken::Value(value)));
                        }
                    }
                },
                '>' => match self.state {
                    LexerState::Idle | LexerState::CaptureValue(_) => {
                        self.next_idx = full_idx + 1;
                        return Some(Err(self.err(
                            "End of key without start of key",
                            full_idx,
                            1,
                        )));
                    }
                    LexerState::CaptureKey(start) => {
                        self.state = LexerState::Idle;

                        let name = &self.src.0[start + 1..full_idx];
                        self.last_item_was_value = false;
                        self.last_open = Some(name);
                        self.next_idx = full_idx + 1;
                        return Some(Ok(QfxToken::OpenKey(name)));
                    }
                    LexerState::CaptureCloseKey(start) => {
                        self.state = LexerState::Idle;

                        let name = &self.src.0[start + 2..full_idx];
                        let hide = self.hide_field_close
                            && self.last_item_was_value
                            && self.last_open == Some(name);

                        self.last_item_was_value = false;
                        self.last_open = None;

                        if !hide {
                            self.next_idx = full_idx + 1;
                            return Some(Ok(QfxToken::CloseKey(name)));
                        }
                    }
                },
                '/' => match self.state {
                    LexerState::Idle | LexerState::CaptureValue(_) => {}
                    LexerState::CaptureKey(start) => {
                        if start != full_idx.saturating_sub(1) {
                            self.next_idx = full_idx + 1;
                            return Some(Err(self.err(
                                "Slash in key name",
                                full_idx,
                                full_idx - start,
                            )));
                        }

                        self.state = LexerState::CaptureCloseKey(start)
                    }
                    LexerState::CaptureCloseKey(start) => {
                        self.next_idx = full_idx + 1;
                        return Some(Err(self.err(
                            "Slash in key name",
                            full_idx,
                            full_idx - start,
                        )));
                    }
                },
                '\r' | '\n' => {}
                _ => match self.state {
                    LexerState::Idle => self.state = LexerState::CaptureValue(full_idx),
                    LexerState::CaptureKey(_)
                    | LexerState::CaptureCloseKey(_)
                    | LexerState::CaptureValue(_) => {}
                },
            }
        }
    }
}

#[derive(Debug)]
pub enum Severity {
    Info,
}

#[derive(Debug)]
pub struct StatementTransaction<'a> {
    transaction_type: QfxTransactionType,
    date_posted: DateTime<FixedOffset>,
    // user_date: Option<NaiveDateTime>,
    amount: f64,
    transaction_id: &'a str,
    name: &'a str,
    // account_to: Option<AccountTo>,
    memo: Option<&'a str>,
}

#[derive(Debug)]
pub struct AccountTo {
    // account_id: u32,
}

#[derive(Debug)]
pub enum QfxTransactionType {
    Debit,
    Credit,
    Pos,
    Atm,
    Fee,
    Other,
}

#[derive(Debug)]
pub enum AccountType {
    Savings,
}

#[derive(Debug)]
enum ParserState {
    NotStarted,
    ReadOpen,
    ReadClose,
    ReadInstitutionMessage,
    ReadStatementTransactionResponse,
    ReadStatementResponse,
    ReadTransactionList,
    ReadTransaction,
}

struct DocumentParser<'a> {
    tokens: Lexer<'a>,
    local_timezone: Option<FixedOffset>,
    // State tracking
    state: ParserState,
    read_sign_on_message_response: bool,
    institution_message_response_name: Option<&'a str>,
    statement_transaction_response_name: Option<&'a str>,
    read_transaction_id: bool,
    read_status: bool,
    statement_response_name: Option<&'a str>,
    read_currency: bool,
    read_account_from: bool,
    read_start_date: bool,
    read_end_date: bool,
    read_ledger_balance: bool,
    read_available_balance: bool,
    // Transaction
    transaction_type: Option<QfxTransactionType>,
    date_posted: Option<DateTime<FixedOffset>>,
    user_date: Option<NaiveDateTime>,
    amount: Option<f64>,
    transaction_id: Option<&'a str>,
    name: Option<&'a str>,
    account_to: Option<AccountTo>,
    memo: Option<&'a str>,
}

impl<'a> DocumentParser<'a> {
    fn new(lexer: Lexer<'a>) -> Self {
        Self {
            tokens: lexer,
            local_timezone: None,
            state: ParserState::NotStarted,
            read_sign_on_message_response: false,
            institution_message_response_name: None,
            statement_transaction_response_name: None,
            read_transaction_id: false,
            read_status: false,
            statement_response_name: None,
            read_currency: false,
            read_account_from: false,
            read_start_date: false,
            read_end_date: false,
            read_ledger_balance: false,
            read_available_balance: false,
            transaction_type: None,
            date_posted: None,
            user_date: None,
            amount: None,
            transaction_id: None,
            name: None,
            account_to: None,
            memo: None,
        }
    }

    fn next_transaction(&mut self) -> Result<Option<StatementTransaction<'a>>> {
        loop {
            match self.state {
                ParserState::NotStarted => {
                    let first_key = self.get_key()?;
                    if first_key != "OFX" {
                        bail!("Unexpected key '{}' for state {:?}", first_key, self.state);
                    }
                    self.state = ParserState::ReadOpen;
                }
                ParserState::ReadOpen => match self.get_field("OFX")? {
                    Some("SIGNONMSGSRSV1") => {
                        let check = self.check_sign_on_message_response_v1();
                        self.read_sign_on_message_response
                            .set_with("SIGNONMSGSRSV1", check)?;
                    }
                    Some("BANKMSGSRSV1") => {
                        self.institution_message_response_name
                            .put_or_else("BANKMSGSRSV1", Ok("BANKMSGSRSV1"))?;
                        self.state = ParserState::ReadInstitutionMessage;
                    }
                    Some("CREDITCARDMSGSRSV1") => {
                        self.institution_message_response_name
                            .put_or_else("CREDITCARDMSGSRSV1", Ok("CREDITCARDMSGSRSV1"))?;
                        self.state = ParserState::ReadInstitutionMessage;
                    }
                    Some(key) => bail!("Unexpected key '{}' for state {:?}", key, self.state),
                    None => {
                        self.expect_done()?;
                        self.state = ParserState::ReadClose;
                    },
                },
                ParserState::ReadInstitutionMessage => {
                    match self.get_field(self.institution_message_response_name.ok_or_eyre(
                        "Missing institution response in ReadInstitutionMessage state",
                    )?)? {
                        Some("STMTTRNRS") => {
                            self.statement_transaction_response_name
                                .put_or_else("STMTTRNRS",  Ok("STMTTRNRS"))?;
                            self.state = ParserState::ReadStatementTransactionResponse;
                        }
                        Some("CCSTMTTRNRS") => {
                            self.statement_transaction_response_name
                                .put_or_else("CCSTMTTRNRS", Ok("CCSTMTTRNRS"))?;
                            self.state = ParserState::ReadStatementTransactionResponse;
                        }
                        Some(key) => bail!("Unexpected key '{}' for state {:?}", key, self.state),
                        None => self.state = ParserState::ReadOpen,
                    }
                }
                ParserState::ReadStatementTransactionResponse => match self.get_field(
                    self.statement_transaction_response_name.ok_or_eyre(
                        "Missing statement transaction response in ReadStatementTransactionRecord state",
                    )?,
                )? {
                    Some("TRNUID") => {
                        let check = self.get_u32();
                        self
                        .read_transaction_id
                        .set_with_value("TRNUID", check)?},
                    Some("STATUS") => {let check = self.check_status(); self.read_status.set_with("STATUS", check)?},
                    Some("STMTRS") => {
                        self.statement_response_name
                            .put_or_else("STMTRS", Ok("STMTRS"))?;
                        self.state = ParserState::ReadStatementResponse;
                    }
                    Some("CCSTMTRS") => {
                        self.statement_response_name
                            .put_or_else("CCSTMTRS", Ok("CCSTMTRS"))?;
                        self.state = ParserState::ReadStatementResponse;
                    }
                    Some(key) => bail!("Unexpected key '{}' for state {:?}", key, self.state),
                    None => self.state = ParserState::ReadInstitutionMessage,
                },
                ParserState::ReadStatementResponse => match self.get_field(self.statement_response_name.ok_or_eyre(
                        "Missing statement response in ReadStatementResponse state",
                    )?,)? {
                    Some("CURDEF") => {
                        let check = self.check_currency();
                        self
                        .read_currency
                        .set_with("CURDEF", check)?},
                    Some("BANKACCTFROM") => {
                        let check = self.check_account_from("BANKACCTFROM");
                        self
                        .read_account_from
                        .set_with("BANKACCTFROM",  check)?},
                    Some("CCACCTFROM") => {
                        let check = self.check_account_from("CCACCTFROM");
                        self
                        .read_account_from
                        .set_with("CCACCTFROM",  check)?},
                    Some("BANKTRANLIST") => self.state = ParserState::ReadTransactionList,
                    Some("LEDGERBAL") => {
                        let check = self.check_balance("LEDGERBAL");
                        self.read_ledger_balance.set_with("LEDGERBAL", check)?;
                    }
                    Some("AVAILBAL") => {
                        let check = self.check_balance("AVAILBAL");
                        self.read_available_balance.set_with("AVAILBAL", check)?;
                    }
                    Some(key) => bail!("Unexpected key '{}' for state {:?}", key, self.state),
                    None => self.state = ParserState::ReadStatementTransactionResponse,
                },
                ParserState::ReadTransactionList => match self.get_field("BANKTRANLIST")? {
                    Some("DTSTART") => {let check = self.get_timestamp();self.read_start_date.set_with_value("DTSTART",  check)?},
                    Some("DTEND") => {let check = self.get_timestamp();self.read_end_date.set_with_value("DTEND",  check)?},
                    Some("STMTTRN") => self.state = ParserState::ReadTransaction,
                    Some(key) => bail!("Unexpected key '{}' for state {:?}", key, self.state),
                    None => self.state = ParserState::ReadStatementResponse,
                },
                ParserState::ReadTransaction => match self.get_field("STMTTRN")? {
                    Some("TRNTYPE") => {let check = self.get_transaction_type();self.transaction_type.put_or_else("TRNTYPE", check)?},
                    Some("DTPOSTED") => {let check = self.get_timestamp();self.date_posted.put_or_else("DTPOSTED", check)?},
                    Some("DTUSER") => {let check = self.get_timestamp_naive();self.user_date.put_or_else("DTUSER", check)?},
                    Some("TRNAMT") => {let check = self.get_float();self.amount.put_or_else("TRNAMT", check)?},
                    Some("FITID") => {let check = self.get_value();self.transaction_id.put_or_else("FITID", check)?},
                    Some("NAME") => {let check = self.get_value();self.name.put_or_else("NAME",  check)?},
                    Some("CCACCTTO") => {let check =self.get_account_to(); self.account_to.put_or_else("CCACCTTO",  check)?},
                    Some("MEMO") => {let check = self.get_value();self.memo.put_or_else("MEMO", check)?},
                    Some(key) => bail!("Unexpected key '{}' for state {:?}", key, self.state),
                    None => {
                        let _ = self.user_date.take();
                        let _ = self.account_to.take();
                        let transaction = StatementTransaction {
                            transaction_type: self.transaction_type.take().ok_or_eyre("Missing key 'TRNTYPE'")?,
                            date_posted: self.date_posted.take().ok_or_eyre("Missing key 'DTPOSTED'")?,
                            // user_date: self.user_date.take(),
                            amount: self.amount.take().ok_or_eyre("Missing key 'TRNAMT'")?,
                            transaction_id: self.transaction_id.take().ok_or_eyre("Missing key 'FITID'")?,
                            name: self.name.take().ok_or_eyre("Missing key 'NAME'")?,
                            // account_to: self.account_to.take(),
                            memo: self.memo.take(),
                        };

                        self.state = ParserState::ReadTransactionList;
                        return Ok(Some(transaction));
                    },
                }
                ParserState::ReadClose => return Ok(None),
            }
        }
    }

    fn check_sign_on_message_response_v1(&mut self) -> Result<()> {
        let mut sign_on_response = false;
        loop {
            match self.get_field("SIGNONMSGSRSV1")? {
                Some("SONRS") => {
                    sign_on_response.set_with("SONRS", self.check_sign_on_response())?
                }
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        sign_on_response.ensure_field("SIGNONMSGSRSV1")?;
        Ok(())
    }

    fn check_sign_on_response(&mut self) -> Result<()> {
        let mut status = false;
        let mut server_date = false;
        let mut language = false;
        let mut last_profile_update = false;
        let mut financial_institution = false;
        let mut bank_id = false;
        loop {
            match self.get_field("SONRS")? {
                Some("STATUS") => status.set_with("STATUS", self.check_status())?,
                Some("DTSERVER") => server_date.set_with_value("DTSERVER", self.get_timestamp())?,
                Some("LANGUAGE") => language.set_with_value("LANGUAGE", self.get_value())?,
                Some("DTPROFUP") => {
                    last_profile_update.set_with_value("DTPROFUP", self.get_timestamp())?
                }
                Some("FI") => {
                    financial_institution.set_with("FI", self.check_financial_institution())?
                }
                Some("INTU.BID") => bank_id.set_with_value("INTU.BID", self.get_u32())?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        status.ensure_field("STATUS")?;
        server_date.ensure_field("DTSERVER")?;
        language.ensure_field("LANGUAGE")?;
        // last_profile_update is optional
        financial_institution.ensure_field("FI")?;
        bank_id.ensure_field("INTU.BID")?;
        Ok(())
    }

    fn check_status(&mut self) -> Result<()> {
        let mut code = false;
        let mut severity = false;
        let mut message = false;
        loop {
            match self.get_field("STATUS")? {
                Some("CODE") => code.set_with_value("CODE", self.get_u32())?,
                Some("SEVERITY") => severity.set_with_value("SEVERITY", self.get_severity())?,
                Some("MESSAGE") => message.set_with_value("MESSAGE", self.get_value())?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        code.ensure_field("CODE")?;
        severity.ensure_field("SEVERITY")?;
        // message is optional

        Ok(())
    }

    fn check_financial_institution(&mut self) -> Result<()> {
        let mut organization = false;
        let mut institution_id = false;
        loop {
            match self.get_field("FI")? {
                Some("ORG") => organization.set_with_value("ORG", self.get_value())?,
                Some("FID") => institution_id.set_with_value("FID", self.get_u32())?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        organization.ensure_field("ORG")?;
        institution_id.ensure_field("FID")?;
        Ok(())
    }

    fn check_account_from(&mut self, struct_name: &str) -> Result<()> {
        let mut bank_id = false;
        let mut account_number = false;
        let mut account_type = false;
        loop {
            match self.get_field(struct_name)? {
                Some("BANKID") => bank_id.set_with_value("BANKID", self.get_u32())?,
                Some("ACCTID") => account_number.set_with_value("ACCTID", self.get_u32())?,
                Some("ACCTTYPE") => {
                    account_type.set_with_value("ACCTTYPE", self.get_account_type())?
                }
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        account_number.ensure_field("ACCTID")?;
        Ok(())
    }

    fn check_balance(&mut self, struct_name: &str) -> Result<()> {
        let mut amount = false;
        let mut timestamp = false;
        loop {
            match self.get_field(struct_name)? {
                Some("BALAMT") => amount.set_with_value("BALAMT", self.get_float())?,
                Some("DTASOF") => timestamp.set_with_value("DTASOF", self.get_timestamp())?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        amount.ensure_field("BALAMT")?;
        timestamp.ensure_field("DTASOF")?;

        Ok(())
    }

    fn get_account_to(&mut self) -> Result<AccountTo> {
        let mut account_id = None;
        loop {
            match self.get_field("CCACCTTO")? {
                Some("ACCTID") => account_id.put_or_else("ACCTID", self.get_u32())?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        let _ = account_id.ok_or_eyre("Missing key 'ACCTID'")?;
        Ok(AccountTo {})
    }

    fn get_key(&mut self) -> Result<&'a str> {
        match self.tokens.next() {
            Some(Ok(QfxToken::OpenKey(key))) => Ok(key),
            Some(Ok(v)) => Err(eyre!("Expected key, got: {:?}", v)),
            Some(Err(e)) => Err(e),
            None => Err(eyre!("Unexpected end of file")),
        }
    }

    fn get_field(&mut self, struct_name: &str) -> Result<Option<&'a str>> {
        match self.tokens.next() {
            Some(Ok(QfxToken::OpenKey(key))) => Ok(Some(key)),
            Some(Ok(QfxToken::CloseKey(k))) if k == struct_name => Ok(None),
            Some(Ok(v)) => Err(eyre!("Expected key, got: {:?}", v)),
            Some(Err(e)) => Err(e),
            None => Err(eyre!("Unexpected end of file")),
        }
    }

    fn get_value(&mut self) -> Result<&'a str> {
        match self.tokens.next() {
            Some(Ok(QfxToken::Value(value))) => Ok(value),
            Some(Ok(v)) => Err(eyre!("Expected value, got: {:?}", v)),
            Some(Err(e)) => Err(e),
            None => Err(eyre!("Unexpected end of file")),
        }
    }

    fn get_u32(&mut self) -> Result<u32> {
        self.get_value()?
            .parse()
            .wrap_err("Failed to parse u32 value")
    }

    fn get_float(&mut self) -> Result<f64> {
        self.get_value()?
            .parse()
            .wrap_err("Failed to parse float value")
    }

    fn get_timestamp(&mut self) -> Result<DateTime<FixedOffset>> {
        let value = self.get_value()?;

        let (timestamp, offset) = if value.ends_with(']') {
            let mut datetime_parts = value.split('[');
            let datetime_str = datetime_parts
                .next()
                .ok_or_eyre("Timestamp missing start of timezone block")?;

            let datetime = NaiveDateTime::parse_from_str(datetime_str, "%Y%m%d%H%M%S%.f")
                .wrap_err("Failed to parse timestamp")?;

            let mut timezone_parts = datetime_parts
                .next()
                .ok_or_eyre("Timestamp missing timezone block")?
                .split(':');
            let offset_hours = timezone_parts
                .next()
                .ok_or_eyre("Timestamp missing timezone offset")?
                .parse::<i8>()
                .wrap_err("Invalid timezone offset")?;

            let offset = FixedOffset::east_opt(offset_hours as i32 * 60 * 60)
                .ok_or_eyre("Out of bounds timezone offset")?;

            (datetime, offset)
        } else {
            // Fallback to assuming this is local time. This will have annoying daylight savings time implications
            let datetime = NaiveDateTime::parse_from_str(value, "%Y%m%d%H%M%S%.f")
                .wrap_err("Failed to parse naive date value")?;

            (datetime, self.get_local_time())
        };

        offset
            .from_local_datetime(&timestamp)
            .single()
            .ok_or_eyre("Ambiguous date conversion")
    }

    fn get_timestamp_naive(&mut self) -> Result<NaiveDateTime> {
        NaiveDateTime::parse_from_str(self.get_value()?, "%Y%m%d%H%M%S%.f")
            .wrap_err("Failed to parse naive date value")
    }

    fn get_severity(&mut self) -> Result<Severity> {
        match self.get_value() {
            Ok("INFO") => Ok(Severity::Info),
            Ok(v) => Err(eyre!("Unexpected severity: '{}'", v)),
            Err(e) => Err(e.wrap_err("Failed to parse severity")),
        }
    }

    fn check_currency(&mut self) -> Result<()> {
        match self.get_value() {
            Ok("CAD") => Ok(()),
            Ok(v) => Err(eyre!("Unexpected currency: '{}'", v)),
            Err(e) => Err(e.wrap_err("Failed to parse currency")),
        }
    }

    fn get_account_type(&mut self) -> Result<AccountType> {
        match self.get_value() {
            Ok("SAVINGS") => Ok(AccountType::Savings),
            Ok(v) => Err(eyre!("Unexpected account type: '{}'", v)),
            Err(e) => Err(e.wrap_err("Failed to parse account type")),
        }
    }

    fn get_transaction_type(&mut self) -> Result<QfxTransactionType> {
        match self.get_value() {
            Ok("DEBIT") => Ok(QfxTransactionType::Debit),
            Ok("CREDIT") => Ok(QfxTransactionType::Credit),
            Ok("POS") => Ok(QfxTransactionType::Pos),
            Ok("ATM") => Ok(QfxTransactionType::Atm),
            Ok("FEE") => Ok(QfxTransactionType::Fee),
            Ok("OTHER") => Ok(QfxTransactionType::Other),
            Ok(v) => Err(eyre!("Unexpected transaction type: '{}'", v)),
            Err(e) => Err(e.wrap_err("Failed to parse transaction type")),
        }
    }

    fn expect_done(&mut self) -> Result<()> {
        match self.tokens.next() {
            Some(Ok(v)) => Err(eyre!("Unexpected token at end of file: {:?}", v)),
            Some(Err(e)) => Err(e.wrap_err("Error at end of file")),
            None => Ok(()),
        }
    }

    fn get_local_time(&mut self) -> FixedOffset {
        match self.local_timezone {
            Some(t) => t,
            None => {
                let local_timezone = *Local::now().offset();
                self.local_timezone = Some(local_timezone);
                local_timezone
            }
        }
    }
}

pub struct QfxTransactionIter<'a> {
    parser: DocumentParser<'a>,
}

impl<'a> Iterator for QfxTransactionIter<'a> {
    type Item = Result<Transaction<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.parser.next_transaction() {
            Ok(Some(StatementTransaction {
                transaction_type,
                date_posted,
                amount,
                transaction_id,
                name,
                memo,
                ..
            })) => {
                let file_transaction_type = match transaction_type {
                    QfxTransactionType::Debit => TransactionType::Debit,
                    QfxTransactionType::Credit => TransactionType::Credit,
                    QfxTransactionType::Pos => TransactionType::Pos,
                    QfxTransactionType::Atm => TransactionType::Atm,
                    QfxTransactionType::Fee => TransactionType::Fee,
                    QfxTransactionType::Other => TransactionType::Other,
                };
                let date = date_posted.date_naive();

                Some(Ok(Transaction {
                    transaction_type: file_transaction_type,
                    date_posted: date,
                    amount,
                    transaction_id: Some(Cow::Borrowed(transaction_id)),
                    category: None,
                    name: Cow::Borrowed(name),
                    memo: memo.map(Cow::Borrowed),
                }))
            }
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}
