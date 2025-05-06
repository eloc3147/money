use std::{
    borrow::Cow,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
    str::{CharIndices, FromStr},
};

use chrono::{DateTime, FixedOffset, Local, NaiveDateTime, TimeZone};
use color_eyre::{
    Report, Result,
    eyre::{Context, OptionExt, bail, eyre},
};
use encoding_rs::WINDOWS_1252;
use rust_decimal::Decimal;

pub fn load_file(path: &Path) -> Result<()> {
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

    let file_str = match encoding {
        HeaderEncoding::Windows1252 => WINDOWS_1252.decode(&file_bytes).0,
        HeaderEncoding::Utf8 => Cow::Borrowed(str::from_utf8(&file_bytes)?),
    };

    // Parse file
    let lexer = LexerIterator::new(&file_str, is_xml);
    let mut parser = DocumentParser::new(lexer);

    let document = parser.parse_document()?;

    Ok(())
}

trait PutOrElse<T> {
    fn put_or_else<F>(&mut self, name: &str, op: F) -> Result<()>
    where
        F: FnOnce() -> Result<T>;
}

impl<T> PutOrElse<T> for Option<T> {
    fn put_or_else<F>(&mut self, name: &str, op: F) -> Result<()>
    where
        F: FnOnce() -> Result<T>,
    {
        match self {
            Some(_) => Err(eyre!("Duplicate key '{}'", name)),
            None => {
                *self = Some(op()?);
                Ok(())
            }
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

        if line_ref.len() == 0 {
            // Double newline means end of header
            break;
        }

        let header = str::from_utf8(line_ref).wrap_err_with(|| {
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
                let parsed = match value {
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
            if kv_pair.len() == 0 {
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
        if kv_pair.len() == 0 {
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

enum LexerState {
    Idle,
    CaptureKey(usize),
    CaptureCloseKey(usize),
    CaptureValue(usize),
}

struct LexerIterator<'a> {
    state: LexerState,
    src: &'a str,
    char_iter: CharIndices<'a>,
    hide_field_close: bool,
    last_item_was_value: bool,
    last_open: Option<&'a str>,
}

impl<'a> LexerIterator<'a> {
    fn new(src: &'a str, hide_field_close: bool) -> Self {
        Self {
            state: LexerState::Idle,
            src,
            char_iter: src.char_indices(),
            hide_field_close,
            last_item_was_value: false,
            last_open: None,
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

impl<'a> Iterator for LexerIterator<'a> {
    type Item = Result<QfxToken<'a>>;

    fn next(&mut self) -> Option<Result<QfxToken<'a>>> {
        loop {
            let Some((idx, char)) = self.char_iter.next() else {
                return match self.state {
                    LexerState::Idle => None,
                    LexerState::CaptureKey(start) | LexerState::CaptureCloseKey(start) => {
                        Some(Err(self.err(
                            "End of file in key",
                            self.src.len() - 1,
                            self.src.len() - start,
                        )))
                    }
                    LexerState::CaptureValue(start) => {
                        Some(Ok(QfxToken::Value(&self.src[start..])))
                    }
                };
            };
            match char {
                '<' => match self.state {
                    LexerState::Idle => {
                        self.state = LexerState::CaptureKey(idx);
                    }
                    LexerState::CaptureKey(start) | LexerState::CaptureCloseKey(start) => {
                        return Some(Err(self.err(
                            "Start of new key inside key",
                            idx,
                            idx - start,
                        )));
                    }
                    LexerState::CaptureValue(start) => {
                        self.state = LexerState::CaptureKey(idx);
                        self.last_item_was_value = true;
                        let value = &self.src[start..idx];
                        // Ignore empty values
                        if value.trim().len() == 0 {
                            self.state = LexerState::CaptureKey(idx);
                        } else {
                            return Some(Ok(QfxToken::Value(value)));
                        }
                    }
                },
                '>' => match self.state {
                    LexerState::Idle | LexerState::CaptureValue(_) => {
                        return Some(Err(self.err("End of key without start of key", idx, 1)));
                    }
                    LexerState::CaptureKey(start) => {
                        self.state = LexerState::Idle;

                        let name = &self.src[start + 1..idx];
                        self.last_item_was_value = false;
                        self.last_open = Some(name);
                        return Some(Ok(QfxToken::OpenKey(name)));
                    }
                    LexerState::CaptureCloseKey(start) => {
                        self.state = LexerState::Idle;

                        let name = &self.src[start + 2..idx];
                        let hide = self.hide_field_close
                            && self.last_item_was_value
                            && self.last_open == Some(name);

                        self.last_item_was_value = false;
                        self.last_open = None;

                        if !hide {
                            return Some(Ok(QfxToken::CloseKey(name)));
                        }
                    }
                },
                '/' => match self.state {
                    LexerState::Idle | LexerState::CaptureValue(_) => {}
                    LexerState::CaptureKey(start) => {
                        if start != idx.saturating_sub(1) {
                            return Some(Err(self.err("Slash in key name", idx, idx - start)));
                        }

                        self.state = LexerState::CaptureCloseKey(start)
                    }
                    LexerState::CaptureCloseKey(start) => {
                        return Some(Err(self.err("Slash in key name", idx, idx - start)));
                    }
                },
                '\r' | '\n' => {}
                _ => match self.state {
                    LexerState::Idle => self.state = LexerState::CaptureValue(idx),
                    LexerState::CaptureKey(_)
                    | LexerState::CaptureCloseKey(_)
                    | LexerState::CaptureValue(_) => {}
                },
            }
        }
    }
}

#[derive(Debug)]
pub struct Document<'a> {
    sign_on_message_response: SignOnMessageResponseV1<'a>,
    institution_message_response: InstitutionMessageResponseV1<'a>,
}

#[derive(Debug)]
pub struct SignOnMessageResponseV1<'a> {
    sign_on_response: SignOnResponse<'a>,
}

#[derive(Debug)]
pub struct SignOnResponse<'a> {
    status: Status<'a>,
    server_date: DateTime<FixedOffset>,
    language: &'a str,
    last_profile_update: Option<DateTime<FixedOffset>>,
    financial_institution: FinancialInstitution<'a>,
    bank_id: u32,
}

#[derive(Debug)]
pub enum Severity {
    Info,
}

#[derive(Debug)]
pub struct Status<'a> {
    code: u32,
    severity: Severity,
    message: Option<&'a str>,
}

#[derive(Debug)]
pub struct FinancialInstitution<'a> {
    organization: &'a str,
    institution_id: u32,
}

#[derive(Debug)]
pub struct InstitutionMessageResponseV1<'a> {
    statement_transaction_response: StatementTransactionResponse<'a>,
}

#[derive(Debug)]
pub struct StatementTransactionResponse<'a> {
    unique_id: u32,
    status: Status<'a>,
    statement_response: StatementResponse<'a>,
}

#[derive(Debug)]
pub struct StatementResponse<'a> {
    default_currency: Currency,
    bank_account_from: AccountFrom,
    statement_transaction_data: StatementTransactionData<'a>,
    ledger_balance: Balance,
    available_balance: Balance,
}

#[derive(Debug)]
pub enum Currency {
    Cad,
}

#[derive(Debug)]
pub struct AccountFrom {
    bank_id: Option<u32>,
    account_number: u32,
    account_type: Option<AccountType>,
}

#[derive(Debug)]
pub struct StatementTransactionData<'a> {
    start_date: DateTime<FixedOffset>,
    end_date: DateTime<FixedOffset>,
    transactions: Vec<StatementTransaction<'a>>,
}

#[derive(Debug)]
pub struct StatementTransaction<'a> {
    transaction_type: TransactionType,
    date_posted: NaiveDateTime,
    user_date: Option<NaiveDateTime>,
    amount: Decimal,
    transaction_id: &'a str,
    name: &'a str,
    account_to: Option<AccountTo>,
    memo: Option<&'a str>,
}

#[derive(Debug)]
pub struct Balance {
    amount: Decimal,
    timestamp: DateTime<FixedOffset>,
}

#[derive(Debug)]
pub struct AccountTo {
    account_id: u32,
}

#[derive(Debug)]
pub enum TransactionType {
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

struct DocumentParser<'a> {
    tokens: LexerIterator<'a>,
    local_timezone: Option<FixedOffset>,
}

impl<'a> DocumentParser<'a> {
    fn new(tokens: LexerIterator<'a>) -> Self {
        Self {
            tokens,
            local_timezone: None,
        }
    }

    fn parse_document(&mut self) -> Result<Document> {
        let document = self.get_ofx().wrap_err("Failed to parse struct 'OFX'")?;
        self.expect_done()?;

        Ok(document)
    }

    fn get_ofx(&mut self) -> Result<Document<'a>> {
        let first_key = self.get_key()?;
        if first_key != "OFX" {
            bail!(
                "Unexpected key '{}', expected key 'OFX' at start of document",
                first_key
            );
        }

        let mut sign_on_message_response = None;
        let mut institution_message_response = None;
        loop {
            match self.get_field("OFX")? {
                Some("SIGNONMSGSRSV1") => {
                    sign_on_message_response.put_or_else("SIGNONMSGSRSV1", || {
                        self.get_sign_on_message_response_v1()
                            .wrap_err("Failed to parse struct 'SIGNONMSGSRSV1'")
                    })?
                }
                Some("BANKMSGSRSV1") => {
                    institution_message_response.put_or_else("BANKMSGSRSV1", || {
                        self.get_msg_srs_v1("BANKMSGSRSV1")
                            .wrap_err("Failed to parse struct 'BANKMSGSRSV1'")
                    })?
                }
                Some("CREDITCARDMSGSRSV1") => {
                    institution_message_response.put_or_else("CREDITCARDMSGSRSV1", || {
                        self.get_msg_srs_v1("CREDITCARDMSGSRSV1")
                            .wrap_err("Failed to parse struct 'CREDITCARDMSGSRSV1'")
                    })?
                }
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(Document {
            sign_on_message_response: sign_on_message_response
                .ok_or_eyre("Missing key 'SIGNONMSGSRSV1'")?,
            institution_message_response: institution_message_response
                .ok_or_eyre("Missing key 'BANKMSGSRSV1' or 'CREDITCARDMSGSRSV1'")?,
        })
    }

    fn get_sign_on_message_response_v1(&mut self) -> Result<SignOnMessageResponseV1<'a>> {
        let mut sign_on_response = None;
        match self.get_key()? {
            "SONRS" => sign_on_response.put_or_else("SONRS", || {
                self.get_sign_on_response()
                    .wrap_err("Failed to parse struct 'SONRS'")
            })?,
            key => bail!("Unexpected key '{}'", key),
        }
        self.expect_close("SIGNONMSGSRSV1")?;

        Ok(SignOnMessageResponseV1 {
            sign_on_response: sign_on_response.ok_or_eyre("Missing key 'SONRS'")?,
        })
    }

    fn get_sign_on_response(&mut self) -> Result<SignOnResponse<'a>> {
        let mut status = None;
        let mut dtserver = None;
        let mut language = None;
        let mut last_profile_update = None;
        let mut financial_institution = None;
        let mut bank_id = None;
        loop {
            match self.get_field("SONRS")? {
                Some("STATUS") => status.put_or_else("STATUS", || {
                    self.get_status()
                        .wrap_err("Failed to parse struct 'STATUS'")
                })?,
                Some("DTSERVER") => dtserver.put_or_else("DTSERVER", || self.get_timestamp())?,
                Some("LANGUAGE") => language.put_or_else("LANGUAGE", || self.get_value())?,
                Some("DTPROFUP") => {
                    last_profile_update.put_or_else("DTPROFUP", || self.get_timestamp())?
                }
                Some("FI") => financial_institution.put_or_else("FI", || {
                    self.get_financial_institution()
                        .wrap_err("Failed to parse struct 'FI'")
                })?,
                Some("INTU.BID") => bank_id.put_or_else("INTU.BID", || self.get_u32())?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(SignOnResponse {
            status: status.ok_or_eyre("Missing key 'STATUS'")?,
            server_date: dtserver.ok_or_eyre("Missing key 'DTSERVER'")?,
            language: language.ok_or_eyre("Missing key 'LANGUAGE'")?,
            last_profile_update,
            financial_institution: financial_institution.ok_or_eyre("Missing key 'FI'")?,
            bank_id: bank_id.ok_or_eyre("Missing key 'INTU.BID'")?,
        })
    }

    fn get_status(&mut self) -> Result<Status<'a>> {
        let mut code = None;
        let mut severity = None;
        let mut message = None;
        loop {
            match self.get_field("STATUS")? {
                Some("CODE") => code.put_or_else("CODE", || self.get_u32())?,
                Some("SEVERITY") => severity.put_or_else("SEVERITY", || self.get_severity())?,
                Some("MESSAGE") => message.put_or_else("MESSAGE", || self.get_value())?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(Status {
            code: code.ok_or_eyre("Missing key 'CODE'")?,
            severity: severity.ok_or_eyre("Missing key 'SEVERITY'")?,
            message,
        })
    }

    fn get_financial_institution(&mut self) -> Result<FinancialInstitution<'a>> {
        let mut organization = None;
        let mut institution_id = None;
        loop {
            match self.get_field("FI")? {
                Some("ORG") => organization.put_or_else("ORG", || self.get_value())?,
                Some("FID") => institution_id.put_or_else("FID", || self.get_u32())?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(FinancialInstitution {
            organization: organization.ok_or_eyre("Missing key 'ORG'")?,
            institution_id: institution_id.ok_or_eyre("Missing key 'FID'")?,
        })
    }

    fn get_msg_srs_v1(&mut self, struct_name: &str) -> Result<InstitutionMessageResponseV1<'a>> {
        let mut statement_transaction_response = None;
        loop {
            match self.get_field(struct_name)? {
                Some("STMTTRNRS") => {
                    statement_transaction_response.put_or_else("STMTTRNRS", || {
                        self.get_statement_transaction_response("STMTTRNRS")
                            .wrap_err("Failed to parse struct 'STMTTRNRS'")
                    })?
                }
                Some("CCSTMTTRNRS") => {
                    statement_transaction_response.put_or_else("CCSTMTTRNRS", || {
                        self.get_statement_transaction_response("CCSTMTTRNRS")
                            .wrap_err("Failed to parse struct 'CCSTMTTRNRS'")
                    })?
                }
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(InstitutionMessageResponseV1 {
            statement_transaction_response: statement_transaction_response
                .ok_or_eyre("Missing key 'STMTTRNRS' or 'CCSTMTTRNRS'")?,
        })
    }

    fn get_statement_transaction_response(
        &mut self,
        struct_name: &str,
    ) -> Result<StatementTransactionResponse<'a>> {
        let mut unique_id = None;
        let mut status = None;
        let mut statement_response = None;
        loop {
            match self.get_field(struct_name)? {
                Some("TRNUID") => unique_id.put_or_else("TRNUID", || self.get_u32())?,
                Some("STATUS") => status.put_or_else("STATUS", || {
                    self.get_status()
                        .wrap_err("Failed to parse struct 'STATUS'")
                })?,
                Some("STMTRS") if struct_name == "STMTTRNRS" => {
                    statement_response.put_or_else("STMTRS", || {
                        self.get_statement_response("STMTRS")
                            .wrap_err("Failed to parse struct 'STMTRS'")
                    })?
                }
                Some("CCSTMTRS") if struct_name == "CCSTMTTRNRS" => statement_response
                    .put_or_else("CCSTMTRS", || {
                        self.get_statement_response("CCSTMTRS")
                            .wrap_err("Failed to parse struct 'CCSTMTRS'")
                    })?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(StatementTransactionResponse {
            unique_id: unique_id.ok_or_eyre("Missing key 'TRNUID'")?,
            status: status.ok_or_eyre("Missing key 'STATUS'")?,
            statement_response: statement_response
                .ok_or_eyre("Missing key 'STMTRS' or 'CCSTMTRS'")?,
        })
    }

    fn get_statement_response(&mut self, struct_name: &str) -> Result<StatementResponse<'a>> {
        let mut default_currency = None;
        let mut bank_account_from = None;
        let mut statement_transaction_data = None;
        let mut ledger_balance = None;
        let mut available_balance = None;
        loop {
            match self.get_field(struct_name)? {
                Some("CURDEF") => default_currency.put_or_else("CURDEF", || self.get_currency())?,
                Some("BANKACCTFROM") => bank_account_from.put_or_else("BANKACCTFROM", || {
                    self.get_account_from("BANKACCTFROM")
                        .wrap_err("Failed to parse struct 'BANKACCTFROM'")
                })?,
                Some("CCACCTFROM") => bank_account_from.put_or_else("CCACCTFROM", || {
                    self.get_account_from("CCACCTFROM")
                        .wrap_err("Failed to parse struct 'CCACCTFROM'")
                })?,
                Some("BANKTRANLIST") => {
                    statement_transaction_data.put_or_else("BANKTRANLIST", || {
                        self.get_statement_transaction_data()
                            .wrap_err("Failed to parse struct 'BANKTRANLIST'")
                    })?
                }
                Some("LEDGERBAL") => ledger_balance.put_or_else("LEDGERBAL", || {
                    self.get_balance("LEDGERBAL")
                        .wrap_err("Failed to parse struct 'LEDGERBAL'")
                })?,
                Some("AVAILBAL") => available_balance.put_or_else("AVAILBAL", || {
                    self.get_balance("AVAILBAL")
                        .wrap_err("Failed to parse struct 'AVAILBAL'")
                })?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(StatementResponse {
            default_currency: default_currency.ok_or_eyre("Missing key 'CURDEF'")?,
            bank_account_from: bank_account_from
                .ok_or_eyre("Missing key 'BANKACCTFROM' or 'CCACCTFROM'")?,
            statement_transaction_data: statement_transaction_data
                .ok_or_eyre("Missing key 'BANKTRANLIST'")?,
            ledger_balance: ledger_balance.ok_or_eyre("Missing key 'LEDGERBAL'")?,
            available_balance: available_balance.ok_or_eyre("Missing key 'AVAILBAL'")?,
        })
    }

    fn get_account_from(&mut self, struct_name: &str) -> Result<AccountFrom> {
        let mut bank_id = None;
        let mut account_number = None;
        let mut account_type = None;
        loop {
            match self.get_field(struct_name)? {
                Some("BANKID") => bank_id.put_or_else("BANKID", || self.get_u32())?,
                Some("ACCTID") => account_number.put_or_else("ACCTID", || self.get_u32())?,
                Some("ACCTTYPE") => {
                    account_type.put_or_else("ACCTTYPE", || self.get_account_type())?
                }
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(AccountFrom {
            bank_id,
            account_number: account_number.ok_or_eyre("Missing key 'ACCTID'")?,
            account_type,
        })
    }

    fn get_statement_transaction_data(&mut self) -> Result<StatementTransactionData<'a>> {
        let mut start_date = None;
        let mut end_date = None;
        let mut transactions = Vec::new();
        loop {
            match self.get_field("BANKTRANLIST")? {
                Some("DTSTART") => start_date.put_or_else("DTSTART", || self.get_timestamp())?,
                Some("DTEND") => end_date.put_or_else("DTEND", || self.get_timestamp())?,
                Some("STMTTRN") => transactions.push(
                    self.get_statement_transaction("STMTTRN")
                        .wrap_err("Failed to parse struct 'STMTTRN'")?,
                ),
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(StatementTransactionData {
            start_date: start_date.ok_or_eyre("Missing key 'DTSTART'")?,
            end_date: end_date.ok_or_eyre("Missing key 'DTEND'")?,
            transactions,
        })
    }

    fn get_statement_transaction(&mut self, struct_name: &str) -> Result<StatementTransaction<'a>> {
        let mut transaction_type = None;
        let mut date_posted = None;
        let mut user_date = None;
        let mut amount = None;
        let mut transaction_id = None;
        let mut name = None;
        let mut account_to = None;
        let mut memo = None;
        loop {
            match self.get_field(struct_name)? {
                Some("TRNTYPE") => {
                    transaction_type.put_or_else("TRNTYPE", || self.get_transaction_type())?
                }
                Some("DTPOSTED") => {
                    date_posted.put_or_else("DTPOSTED", || self.get_timestamp_naive())?
                }
                Some("DTUSER") => user_date.put_or_else("DTUSER", || self.get_timestamp_naive())?,
                Some("TRNAMT") => amount.put_or_else("TRNAMT", || self.get_decimal())?,
                Some("FITID") => transaction_id.put_or_else("FITID", || self.get_value())?,
                Some("NAME") => name.put_or_else("NAME", || self.get_value())?,
                Some("CCACCTTO") => account_to.put_or_else("CCACCTTO", || {
                    self.get_account_to()
                        .wrap_err("Failed to parse struct 'CCACCTTO'")
                })?,
                Some("MEMO") => memo.put_or_else("MEMO", || self.get_value())?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(StatementTransaction {
            transaction_type: transaction_type.ok_or_eyre("Missing key 'TRNTYPE'")?,
            date_posted: date_posted.ok_or_eyre("Missing key 'DTPOSTED'")?,
            user_date,
            amount: amount.ok_or_eyre("Missing key 'TRNAMT'")?,
            transaction_id: transaction_id.ok_or_eyre("Missing key 'FITID'")?,
            name: name.ok_or_eyre("Missing key 'NAME'")?,
            account_to,
            memo,
        })
    }

    fn get_balance(&mut self, struct_name: &str) -> Result<Balance> {
        let mut amount = None;
        let mut timestamp = None;
        loop {
            match self.get_field(struct_name)? {
                Some("BALAMT") => amount.put_or_else("BALAMT", || self.get_decimal())?,
                Some("DTASOF") => timestamp.put_or_else("DTASOF", || self.get_timestamp())?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(Balance {
            amount: amount.ok_or_eyre("Missing key 'BALAMT'")?,
            timestamp: timestamp.ok_or_eyre("Missing key 'DTASOF'")?,
        })
    }

    fn get_account_to(&mut self) -> Result<AccountTo> {
        let mut account_id = None;
        loop {
            match self.get_field("CCACCTTO")? {
                Some("ACCTID") => account_id.put_or_else("ACCTID", || self.get_u32())?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(AccountTo {
            account_id: account_id.ok_or_eyre("Missing key 'ACCTID'")?,
        })
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

    fn expect_close(&mut self, key: &str) -> Result<()> {
        match self.tokens.next() {
            Some(Ok(QfxToken::CloseKey(k))) if k == key => Ok(()),
            Some(Ok(v)) => Err(eyre!("Expected close key '{}', got: {:?}", key, v)),
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

    fn get_decimal(&mut self) -> Result<Decimal> {
        Decimal::from_str(self.get_value()?).wrap_err("Failed to parse decimal value")
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

    fn get_currency(&mut self) -> Result<Currency> {
        match self.get_value() {
            Ok("CAD") => Ok(Currency::Cad),
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

    fn get_transaction_type(&mut self) -> Result<TransactionType> {
        match self.get_value() {
            Ok("DEBIT") => Ok(TransactionType::Debit),
            Ok("CREDIT") => Ok(TransactionType::Credit),
            Ok("POS") => Ok(TransactionType::Pos),
            Ok("ATM") => Ok(TransactionType::Atm),
            Ok("FEE") => Ok(TransactionType::Fee),
            Ok("OTHER") => Ok(TransactionType::Other),
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
