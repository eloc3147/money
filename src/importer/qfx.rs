use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
    str::{CharIndices, FromStr},
};

use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone};
use color_eyre::{
    Report, Result,
    eyre::{Context, bail, eyre},
};
use encoding_rs::WINDOWS_1252;
use rust_decimal::Decimal;

pub fn load_file(path: &Path) -> Result<()> {
    println!("TMP: Load QFX {:?}", path);

    let mut reader = BufReader::new(File::open(path).wrap_err("Failed to open file")?);

    // Read header
    let header = read_header(&mut reader).wrap_err("Failed to read header")?;
    if header.ofxheader != 100 {
        bail!("Unsupported header: {}", header.ofxheader);
    }
    if header.data != HeaderDataType::OfxSgml {
        bail!("Unsupported data type: {:?}", header.data);
    }
    if header.version != 102 {
        bail!("Unsupported version: {}", header.version);
    }
    if header.encoding != HeaderEncoding::UsaAscii {
        bail!("Unsupported encoding: {:?}", header.encoding);
    }
    if header.charset != HeaderCharset::Windows1252 {
        bail!("Unsupported charset: {:?}", header.charset);
    }

    // Load whole file
    let mut file_bytes = Vec::new();
    reader
        .read_to_end(&mut file_bytes)
        .wrap_err("Failed to read file")?;
    let (file_string, _, _) = WINDOWS_1252.decode(&file_bytes);

    // Parse file
    let lexer = LexerIterator::new(&file_string.as_ref());
    let mut parser = DocumentParser::new(lexer);

    let document = parser.parse_document()?;

    println!("Contents: {:#?}", document);

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
enum HeaderDataType {
    OfxSgml,
}

#[derive(Debug, PartialEq, Eq)]
enum HeaderEncoding {
    UsaAscii,
}

#[derive(Debug, PartialEq, Eq)]
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
}

impl<'a> LexerIterator<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            state: LexerState::Idle,
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
                        return Some(Ok(QfxToken::Value(&self.src[start..idx])));
                    }
                },
                '>' => match self.state {
                    LexerState::Idle | LexerState::CaptureValue(_) => {
                        return Some(Err(self.err("End of key without start of key", idx, 1)));
                    }
                    LexerState::CaptureKey(start) => {
                        self.state = LexerState::Idle;
                        return Some(Ok(QfxToken::OpenKey(&self.src[start + 1..idx])));
                    }
                    LexerState::CaptureCloseKey(start) => {
                        self.state = LexerState::Idle;
                        return Some(Ok(QfxToken::CloseKey(&self.src[start + 2..idx])));
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
    sign_on_message_response: SignOnMessageResponse<'a>,
    bank_message_response: BankMessageResponseV1<'a>,
}

#[derive(Debug)]
pub struct SignOnMessageResponse<'a> {
    sign_on_response: SignOnResponse<'a>,
}

#[derive(Debug)]
pub struct SignOnResponse<'a> {
    status: Status<'a>,
    server_date: DateTime<FixedOffset>,
    language: &'a str,
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
pub struct BankMessageResponseV1<'a> {
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
    bank_account_from: BankAccountFrom,
    statement_transaction_data: StatementTransactionData<'a>,
    ledger_balance: Balance,
    available_balance: Balance,
}

#[derive(Debug)]
pub enum Currency {
    Cad,
}

#[derive(Debug)]
pub struct BankAccountFrom {
    bank_id: u32,
    account_number: u32,
    account_type: AccountType,
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
    amount: Decimal,
    transaction_id: &'a str,
    name: &'a str,
    memo: Option<&'a str>,
}

#[derive(Debug)]
pub struct Balance {
    amount: Decimal,
    timestamp: DateTime<FixedOffset>,
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
}

impl<'a> DocumentParser<'a> {
    fn new(tokens: LexerIterator<'a>) -> Self {
        Self { tokens }
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
        let mut bank_message_response = None;
        loop {
            match self.get_field("OFX")? {
                Some("SIGNONMSGSRSV1") => {
                    sign_on_message_response.put_or_else("SIGNONMSGSRSV1", || {
                        self.get_sign_on_message_response()
                            .wrap_err("Failed to parse struct 'SIGNONMSGSRSV1'")
                    })?
                }
                Some("BANKMSGSRSV1") => {
                    bank_message_response.put_or_else("BANKMSGSRSV1", || {
                        self.get_bank_msg_srs_v1()
                            .wrap_err("Failed to parse struct 'BANKMSGSRSV1'")
                    })?
                }
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(Document {
            sign_on_message_response: sign_on_message_response
                .ok_or_else(|| eyre!("Missing key 'SIGNONMSGSRSV1'"))?,
            bank_message_response: bank_message_response
                .ok_or_else(|| eyre!("Missing key 'BANKMSGSRSV1'"))?,
        })
    }

    fn get_sign_on_message_response(&mut self) -> Result<SignOnMessageResponse<'a>> {
        let mut sign_on_response = None;
        match self.get_key()? {
            "SONRS" => sign_on_response.put_or_else("SONRS", || {
                self.get_sign_on_response()
                    .wrap_err("Failed to parse struct 'SONRS'")
            })?,
            key => bail!("Unexpected key '{}'", key),
        }
        self.expect_close("SIGNONMSGSRSV1")?;

        Ok(SignOnMessageResponse {
            sign_on_response: sign_on_response.ok_or_else(|| eyre!("Missing key 'SONRS'"))?,
        })
    }

    fn get_sign_on_response(&mut self) -> Result<SignOnResponse<'a>> {
        let mut status = None;
        let mut dtserver = None;
        let mut language = None;
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
            status: status.ok_or_else(|| eyre!("Missing key 'STATUS'"))?,
            server_date: dtserver.ok_or_else(|| eyre!("Missing key 'DTSERVER'"))?,
            language: language.ok_or_else(|| eyre!("Missing key 'LANGUAGE'"))?,
            financial_institution: financial_institution
                .ok_or_else(|| eyre!("Missing key 'FI'"))?,
            bank_id: bank_id.ok_or_else(|| eyre!("Missing key 'INTU.BID'"))?,
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
            code: code.ok_or_else(|| eyre!("Missing key 'CODE'"))?,
            severity: severity.ok_or_else(|| eyre!("Missing key 'SEVERITY'"))?,
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
            organization: organization.ok_or_else(|| eyre!("Missing key 'ORG'"))?,
            institution_id: institution_id.ok_or_else(|| eyre!("Missing key 'FID'"))?,
        })
    }

    fn get_bank_msg_srs_v1(&mut self) -> Result<BankMessageResponseV1<'a>> {
        let mut statement_transaction_response = None;
        loop {
            match self.get_field("BANKMSGSRSV1")? {
                Some("STMTTRNRS") => {
                    statement_transaction_response.put_or_else("STMTTRNRS", || {
                        self.get_statement_transaction_response()
                            .wrap_err("Failed to parse struct 'STMTTRNRS'")
                    })?
                }
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(BankMessageResponseV1 {
            statement_transaction_response: statement_transaction_response
                .ok_or_else(|| eyre!("Missing key 'STMTTRNRS'"))?,
        })
    }

    fn get_statement_transaction_response(&mut self) -> Result<StatementTransactionResponse<'a>> {
        let mut unique_id = None;
        let mut status = None;
        let mut statement_response = None;
        loop {
            match self.get_field("STMTTRNRS")? {
                Some("TRNUID") => unique_id.put_or_else("TRNUID", || self.get_u32())?,
                Some("STATUS") => status.put_or_else("STATUS", || {
                    self.get_status()
                        .wrap_err("Failed to parse struct 'STATUS'")
                })?,
                Some("STMTRS") => statement_response.put_or_else("STMTRS", || {
                    self.get_statement_response()
                        .wrap_err("Failed to parse struct 'STMTRS'")
                })?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(StatementTransactionResponse {
            unique_id: unique_id.ok_or_else(|| eyre!("Missing key 'TRNUID'"))?,
            status: status.ok_or_else(|| eyre!("Missing key 'STATUS'"))?,
            statement_response: statement_response.ok_or_else(|| eyre!("Missing key 'STMTRS'"))?,
        })
    }

    fn get_statement_response(&mut self) -> Result<StatementResponse<'a>> {
        let mut default_currency = None;
        let mut bank_account_from = None;
        let mut statement_transaction_data = None;
        let mut ledger_balance = None;
        let mut available_balance = None;
        loop {
            match self.get_field("STMTRS")? {
                Some("CURDEF") => default_currency.put_or_else("CURDEF", || self.get_currency())?,
                Some("BANKACCTFROM") => bank_account_from.put_or_else("BANKACCTFROM", || {
                    self.get_bank_account_from()
                        .wrap_err("Failed to parse struct 'BANKACCTFROM'")
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
            default_currency: default_currency.ok_or_else(|| eyre!("Missing key 'CURDEF'"))?,
            bank_account_from: bank_account_from
                .ok_or_else(|| eyre!("Missing key 'BANKACCTFROM'"))?,
            statement_transaction_data: statement_transaction_data
                .ok_or_else(|| eyre!("Missing key 'BANKTRANLIST'"))?,
            ledger_balance: ledger_balance.ok_or_else(|| eyre!("Missing key 'LEDGERBAL'"))?,
            available_balance: available_balance.ok_or_else(|| eyre!("Missing key 'AVAILBAL'"))?,
        })
    }

    fn get_bank_account_from(&mut self) -> Result<BankAccountFrom> {
        let mut bank_id = None;
        let mut account_number = None;
        let mut account_type = None;
        loop {
            match self.get_field("BANKACCTFROM")? {
                Some("BANKID") => bank_id.put_or_else("BANKID", || self.get_u32())?,
                Some("ACCTID") => account_number.put_or_else("ACCTID", || self.get_u32())?,
                Some("ACCTTYPE") => {
                    account_type.put_or_else("ACCTTYPE", || self.get_account_type())?
                }
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(BankAccountFrom {
            bank_id: bank_id.ok_or_else(|| eyre!("Missing key 'BANKID'"))?,
            account_number: account_number.ok_or_else(|| eyre!("Missing key 'ACCTID'"))?,
            account_type: account_type.ok_or_else(|| eyre!("Missing key 'ACCTTYPE'"))?,
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
                    self.get_statement_transaction()
                        .wrap_err("Failed to parse struct 'STMTTRN'")?,
                ),
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(StatementTransactionData {
            start_date: start_date.ok_or_else(|| eyre!("Missing key 'DTSTART'"))?,
            end_date: end_date.ok_or_else(|| eyre!("Missing key 'DTEND'"))?,
            transactions,
        })
    }

    fn get_statement_transaction(&mut self) -> Result<StatementTransaction<'a>> {
        let mut transaction_type = None;
        let mut date_posted = None;
        let mut amount = None;
        let mut transaction_id = None;
        let mut name = None;
        let mut memo = None;
        loop {
            match self.get_field("STMTTRN")? {
                Some("TRNTYPE") => {
                    transaction_type.put_or_else("TRNTYPE", || self.get_transaction_type())?
                }
                Some("DTPOSTED") => {
                    date_posted.put_or_else("DTPOSTED", || self.get_timestamp_naive())?
                }
                Some("TRNAMT") => amount.put_or_else("TRNAMT", || self.get_decimal())?,
                Some("FITID") => transaction_id.put_or_else("FITID", || self.get_value())?,
                Some("NAME") => name.put_or_else("NAME", || self.get_value())?,
                Some("MEMO") => memo.put_or_else("MEMO", || self.get_value())?,
                Some(key) => bail!("Unexpected key '{}'", key),
                None => break,
            }
        }

        Ok(StatementTransaction {
            transaction_type: transaction_type.ok_or_else(|| eyre!("Missing key 'TRNTYPE'"))?,
            date_posted: date_posted.ok_or_else(|| eyre!("Missing key 'DTPOSTED'"))?,
            amount: amount.ok_or_else(|| eyre!("Missing key 'TRNAMT'"))?,
            transaction_id: transaction_id.ok_or_else(|| eyre!("Missing key 'FITID'"))?,
            name: name.ok_or_else(|| eyre!("Missing key 'NAME'"))?,
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
            amount: amount.ok_or_else(|| eyre!("Missing key 'BALAMT'"))?,
            timestamp: timestamp.ok_or_else(|| eyre!("Missing key 'DTASOF'"))?,
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
        {
            let mut datetime_parts = self.get_value()?.split('[');
            let datetime_str = datetime_parts
                .next()
                .ok_or_else(|| eyre!("Timestamp missing start of timezone block"))?;

            let timestamp = NaiveDateTime::parse_from_str(datetime_str, "%Y%m%d%H%M%S%.f")
                .wrap_err("Failed to parse timestamp")?;

            let mut timezone_parts = datetime_parts
                .next()
                .ok_or_else(|| eyre!("Timestamp missing timezone block"))?
                .split(':');
            let offset_hours = timezone_parts
                .next()
                .ok_or_else(|| eyre!("Timestamp missing timezone offset"))?
                .parse::<i8>()
                .wrap_err("Invalid timezone offset")?;

            let offset = FixedOffset::east_opt(offset_hours as i32 * 60 * 60)
                .ok_or_else(|| eyre!("Out of bounds timezone offset"))?;

            offset
                .from_local_datetime(&timestamp)
                .single()
                .ok_or_else(|| eyre!("Ambiguous date conversion"))
        }
        .wrap_err("Failed to parse date value")
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
}
