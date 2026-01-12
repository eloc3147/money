// Compatible with Tangerine and Capital One QFX files

mod header;
mod lexer;

use std::borrow::Cow;
use std::cell::{Cell, OnceCell};
use std::path::Path;

use chrono::{DateTime, FixedOffset, Local, NaiveDateTime, TimeZone};
use color_eyre::Result;
use color_eyre::eyre::{Context, OptionExt, bail, eyre};
use rust_decimal::Decimal;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

use crate::importer::qfx_file::header::StringEncoding;
use crate::importer::qfx_file::lexer::{Lexer, QfxToken};
use crate::importer::{Transaction, TransactionImporter, TransactionReader, TransactionType};

pub struct QfxReader {
    contents: Vec<u8>,
    is_xml: bool,
    encoding: StringEncoding,
}

impl QfxReader {
    pub async fn open(path: &Path) -> Result<Self> {
        let mut reader = BufReader::new(File::open(path).await.wrap_err("Failed to open file")?);

        // Determine header type
        let buf = reader.fill_buf().await.wrap_err("Failed to read file")?;
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
            let file_header = header::read_xml_header(&mut reader)
                .await
                .wrap_err("Failed to read header")?;
            if file_header.ofxheader != 200 {
                bail!("Unsupported header: {}", file_header.ofxheader);
            }
            if file_header.version != 202 {
                bail!("Unsupported version: {}", file_header.version);
            }
            file_header.encoding
        } else {
            let file_header = header::read_sgml_header(&mut reader)
                .await
                .wrap_err("Failed to read header")?;
            if file_header.ofxheader != 100 {
                bail!("Unsupported header: {}", file_header.ofxheader);
            }
            if file_header.version != 102 {
                bail!("Unsupported version: {}", file_header.version);
            }
            file_header.encoding
        };

        // Load whole file
        let mut contents = Vec::new();
        reader
            .read_to_end(&mut contents)
            .await
            .wrap_err("Failed to read file")?;

        Ok(Self {
            contents,
            is_xml,
            encoding,
        })
    }
}

impl TransactionReader for QfxReader {
    async fn load(self, mut importer: TransactionImporter<'_>) -> Result<()> {
        let lexer = Lexer::new(self.contents, self.encoding, self.is_xml);
        let parser = DocumentParser::new(lexer);

        while let Some(transaction) = parser.next_statement_transaction()? {
            let file_transaction_type = match transaction.transaction_type {
                QfxTransactionType::Debit => TransactionType::Debit,
                QfxTransactionType::Credit => TransactionType::Credit,
                QfxTransactionType::Pos => TransactionType::Pos,
                QfxTransactionType::Atm => TransactionType::Atm,
                QfxTransactionType::Fee => TransactionType::Fee,
                QfxTransactionType::Other => TransactionType::Other,
            };
            let date = transaction.date_posted.date_naive();

            importer
                .import(Transaction {
                    transaction_type: file_transaction_type,
                    date_posted: date,
                    amount: transaction.amount,
                    transaction_id: Some(transaction.transaction_id),
                    category: None,
                    name: transaction.name,
                    memo: transaction.memo,
                })
                .await?;
        }

        Ok(())
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

trait PutLocalOrElse<T> {
    fn put_or_else(&self, name: &str, value: Result<T>) -> Result<()>;
}

impl<T> PutLocalOrElse<T> for OnceCell<T> {
    fn put_or_else(&self, name: &str, value: Result<T>) -> Result<()> {
        let val = value.wrap_err_with(|| eyre!("Error parsing key '{}'", name))?;
        self.set(val).map_err(|_| eyre!("Duplicate key '{}'", name))
    }
}

trait TrackLocalField {
    fn set_with(&mut self, struct_name: &str, check: Result<()>) -> Result<()>;
    fn set_with_value<T>(&mut self, struct_name: &str, check: Result<T>) -> Result<()> {
        self.set_with(struct_name, check.map(|_| ()))
    }

    fn ensure_field(&self, field_name: &str) -> Result<()>;
}

impl TrackLocalField for bool {
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

trait TrackField {
    fn set_with(&self, struct_name: &str, check: Result<()>) -> Result<()>;
    fn set_with_value<T>(&self, struct_name: &str, check: Result<T>) -> Result<()> {
        self.set_with(struct_name, check.map(|_| ()))
    }
}

impl TrackField for Cell<bool> {
    fn set_with(&self, struct_name: &str, check: Result<()>) -> Result<()> {
        let mut val = self.get();
        val.set_with(struct_name, check)?;
        self.set(val);

        Ok(())
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
    amount: Decimal,
    transaction_id: Cow<'a, str>,
    name: Cow<'a, str>,
    // account_to: Option<AccountTo>,
    memo: Option<Cow<'a, str>>,
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

#[derive(Debug, Clone, Copy)]
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

pub struct DocumentParser {
    tokens: Lexer,
    local_timezone: Cell<Option<FixedOffset>>,
    // State tracking
    institution_message_response_name: OnceCell<&'static [u8]>,
    statement_transaction_response_name: OnceCell<&'static [u8]>,
    statement_response_name: OnceCell<&'static [u8]>,
    state: Cell<ParserState>,
    read_sign_on_message_response: Cell<bool>,
    read_transaction_id: Cell<bool>,
    read_status: Cell<bool>,
    read_currency: Cell<bool>,
    read_account_from: Cell<bool>,
    read_start_date: Cell<bool>,
    read_end_date: Cell<bool>,
    read_ledger_balance: Cell<bool>,
    read_available_balance: Cell<bool>,
}

impl<'a> DocumentParser {
    fn new(lexer: Lexer) -> Self {
        Self {
            tokens: lexer,
            local_timezone: Cell::new(None),
            institution_message_response_name: OnceCell::new(),
            statement_transaction_response_name: OnceCell::new(),
            statement_response_name: OnceCell::new(),
            state: Cell::new(ParserState::NotStarted),
            read_sign_on_message_response: Cell::new(false),
            read_transaction_id: Cell::new(false),
            read_status: Cell::new(false),
            read_currency: Cell::new(false),
            read_account_from: Cell::new(false),
            read_start_date: Cell::new(false),
            read_end_date: Cell::new(false),
            read_ledger_balance: Cell::new(false),
            read_available_balance: Cell::new(false),
        }
    }

    fn next_statement_transaction(&'a self) -> Result<Option<StatementTransaction<'a>>> {
        // Transaction
        let mut transaction_type = None;
        let mut date_posted = None;
        let mut user_date = None;
        let mut amount = None;
        let mut transaction_id = None;
        let mut name = None;
        let mut account_to = None;
        let mut memo = None;

        loop {
            match self.state.get() {
                ParserState::NotStarted => {
                    let first_key = self.get_key()?;
                    if first_key != b"OFX" {
                        bail!("Unexpected key '{:?}' for state {:?}", first_key, self.state.get());
                    }
                    self.state.set(ParserState::ReadOpen);
                }
                ParserState::ReadOpen => match self.get_field(b"OFX")? {
                    Some(b"SIGNONMSGSRSV1") => {
                        self.read_sign_on_message_response
                            .set_with("SIGNONMSGSRSV1", self.check_sign_on_message_response_v1())?;
                    }
                    Some(b"BANKMSGSRSV1") => {
                        self.institution_message_response_name
                            .put_or_else("BANKMSGSRSV1", Ok(b"BANKMSGSRSV1"))?;
                        self.state.set(ParserState::ReadInstitutionMessage);
                    }
                    Some(b"CREDITCARDMSGSRSV1") => {
                        self.institution_message_response_name
                            .put_or_else("CREDITCARDMSGSRSV1", Ok(b"CREDITCARDMSGSRSV1"))?;
                        self.state.set(ParserState::ReadInstitutionMessage);
                    }
                    Some(key) => bail!("Unexpected key '{:?}' for state {:?}", key, self.state.get()),
                    None => {
                        self.expect_done()?;
                        self.state.set(ParserState::ReadClose);
                    },
                },
                ParserState::ReadInstitutionMessage => {
                    match self.get_field(self.institution_message_response_name.get().ok_or_eyre(
                        "Missing institution response in ReadInstitutionMessage state",
                    )?)? {
                        Some(b"STMTTRNRS") => {
                            self.statement_transaction_response_name
                                .put_or_else("STMTTRNRS",  Ok(b"STMTTRNRS"))?;
                            self.state.set(ParserState::ReadStatementTransactionResponse);
                        }
                        Some(b"CCSTMTTRNRS") => {
                            self.statement_transaction_response_name
                                .put_or_else("CCSTMTTRNRS", Ok(b"CCSTMTTRNRS"))?;
                            self.state.set(ParserState::ReadStatementTransactionResponse);
                        }
                        Some(key) => bail!("Unexpected key '{:?}' for state {:?}", key, self.state.get()),
                        None => self.state.set(ParserState::ReadOpen),
                    }
                }
                ParserState::ReadStatementTransactionResponse => match self.get_field(
                    self.statement_transaction_response_name.get().ok_or_eyre(
                        "Missing statement transaction response in ReadStatementTransactionRecord state",
                    )?,
                )? {
                    Some(b"TRNUID") => {
                        self
                        .read_transaction_id
                        .set_with_value("TRNUID", self.get_u32())?},
                    Some(b"STATUS") => {self.read_status.set_with("STATUS", self.check_status())?},
                    Some(b"STMTRS") => {
                        self.statement_response_name
                            .put_or_else("STMTRS", Ok(b"STMTRS"))?;
                        self.state.set(ParserState::ReadStatementResponse);
                    }
                    Some(b"CCSTMTRS") => {
                        self.statement_response_name
                            .put_or_else("CCSTMTRS", Ok(b"CCSTMTRS"))?;
                        self.state.set(ParserState::ReadStatementResponse);
                    }
                    Some(key) => bail!("Unexpected key '{:?}' for state {:?}", key, self.state.get()),
                    None => self.state.set(ParserState::ReadInstitutionMessage),
                },
                ParserState::ReadStatementResponse => match self.get_field(self.statement_response_name.get().ok_or_eyre(
                        "Missing statement response in ReadStatementResponse state",
                    )?,)? {
                    Some(b"CURDEF") => {
                        self
                        .read_currency
                        .set_with("CURDEF", self.check_currency())?},
                    Some(b"BANKACCTFROM") => {
                        self
                        .read_account_from
                        .set_with("BANKACCTFROM",  self.check_account_from(b"BANKACCTFROM"))?},
                    Some(b"CCACCTFROM") => {
                        self
                        .read_account_from
                        .set_with("CCACCTFROM",  self.check_account_from(b"CCACCTFROM"))?},
                    Some(b"BANKTRANLIST") => self.state.set(ParserState::ReadTransactionList),
                    Some(b"LEDGERBAL") => {
                        self.read_ledger_balance.set_with("LEDGERBAL", self.check_balance(b"LEDGERBAL"))?;
                    }
                    Some(b"AVAILBAL") => {
                        self.read_available_balance.set_with("AVAILBAL", self.check_balance(b"AVAILBAL"))?;
                    }
                    Some(key) => bail!("Unexpected key '{:?}' for state {:?}", key, self.state.get()),
                    None => self.state.set(ParserState::ReadStatementTransactionResponse),
                },
                ParserState::ReadTransactionList => match self.get_field(b"BANKTRANLIST")? {
                    Some(b"DTSTART") => {let check = self.get_timestamp();self.read_start_date.set_with_value("DTSTART",  check)?},
                    Some(b"DTEND") => {let check = self.get_timestamp();self.read_end_date.set_with_value("DTEND",  check)?},
                    Some(b"STMTTRN") => self.state.set(ParserState::ReadTransaction),
                    Some(key) => bail!("Unexpected key '{:?}' for state {:?}", key, self.state),
                    None => self.state.set(ParserState::ReadStatementResponse),
                },
                ParserState::ReadTransaction => match self.get_field(b"STMTTRN")? {
                    Some(b"TRNTYPE") => {transaction_type.put_or_else("TRNTYPE",  self.get_transaction_type())?},
                    Some(b"DTPOSTED") => {date_posted.put_or_else("DTPOSTED",  self.get_timestamp())?},
                    Some(b"DTUSER") => {user_date.put_or_else("DTUSER",  self.get_timestamp_naive())?},
                    Some(b"TRNAMT") => {amount.put_or_else("TRNAMT",  self.get_decimal())?},
                    Some(b"FITID") => {transaction_id.put_or_else("FITID",  self.get_value())?},
                    Some(b"NAME") => {name.put_or_else("NAME",   self.get_value())?},
                    Some(b"CCACCTTO") => { account_to.put_or_else("CCACCTTO",  self.get_account_to())?},
                    Some(b"MEMO") => {memo.put_or_else("MEMO", self.get_value())?},
                    Some(key) => bail!("Unexpected key '{:?}' for state {:?}", key, self.state),
                    None => {
                        let _ = user_date.take();
                        let _ = account_to.take();
                        let transaction = StatementTransaction {
                            transaction_type: transaction_type.take().ok_or_eyre("Missing key 'TRNTYPE'")?,
                            date_posted: date_posted.take().ok_or_eyre("Missing key 'DTPOSTED'")?,
                            // user_date: user_date.take(),
                            amount: amount.take().ok_or_eyre("Missing key 'TRNAMT'")?,
                            transaction_id: transaction_id.take().ok_or_eyre("Missing key 'FITID'")?,
                            name: name.take().ok_or_eyre("Missing key 'NAME'")?,
                            // account_to: account_to.take(),
                            memo: memo.take(),
                        };

                        self.state.set(ParserState::ReadTransactionList);
                        return Ok(Some(transaction));
                    },
                }
                ParserState::ReadClose => return Ok(None),
            }
        }
    }

    fn check_sign_on_message_response_v1(&self) -> Result<()> {
        let mut sign_on_response = false;
        loop {
            match self.get_field(b"SIGNONMSGSRSV1")? {
                Some(b"SONRS") => {
                    sign_on_response.set_with("SONRS", self.check_sign_on_response())?
                }
                Some(key) => bail!("Unexpected key '{:?}'", key),
                None => break,
            }
        }

        sign_on_response.ensure_field("SIGNONMSGSRSV1")?;
        Ok(())
    }

    fn check_sign_on_response(&self) -> Result<()> {
        let mut status = false;
        let mut server_date = false;
        let mut language = false;
        let mut last_profile_update = false;
        let mut financial_institution = false;
        let mut bank_id = false;
        loop {
            match self.get_field(b"SONRS")? {
                Some(b"STATUS") => status.set_with("STATUS", self.check_status())?,
                Some(b"DTSERVER") => {
                    server_date.set_with_value("DTSERVER", self.get_timestamp())?
                }
                Some(b"LANGUAGE") => language.set_with_value("LANGUAGE", self.get_value())?,
                Some(b"DTPROFUP") => {
                    last_profile_update.set_with_value("DTPROFUP", self.get_timestamp())?
                }
                Some(b"FI") => {
                    financial_institution.set_with("FI", self.check_financial_institution())?
                }
                Some(b"INTU.BID") => bank_id.set_with_value("INTU.BID", self.get_u32())?,
                Some(key) => bail!("Unexpected key '{:?}'", key),
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

    fn check_status(&self) -> Result<()> {
        let mut code = false;
        let mut severity = false;
        let mut message = false;
        loop {
            match self.get_field(b"STATUS")? {
                Some(b"CODE") => code.set_with_value("CODE", self.get_u32())?,
                Some(b"SEVERITY") => severity.set_with_value("SEVERITY", self.get_severity())?,
                Some(b"MESSAGE") => message.set_with_value("MESSAGE", self.get_value())?,
                Some(key) => bail!("Unexpected key '{:?}'", key),
                None => break,
            }
        }

        code.ensure_field("CODE")?;
        severity.ensure_field("SEVERITY")?;
        // message is optional

        Ok(())
    }

    fn check_financial_institution(&self) -> Result<()> {
        let mut organization = false;
        let mut institution_id = false;
        loop {
            match self.get_field(b"FI")? {
                Some(b"ORG") => organization.set_with_value("ORG", self.get_value())?,
                Some(b"FID") => institution_id.set_with_value("FID", self.get_u32())?,
                Some(key) => bail!("Unexpected key '{:?}'", key),
                None => break,
            }
        }

        organization.ensure_field("ORG")?;
        institution_id.ensure_field("FID")?;
        Ok(())
    }

    fn check_account_from(&self, struct_name: &[u8]) -> Result<()> {
        let mut bank_id = false;
        let mut account_number = false;
        let mut account_type = false;
        loop {
            match self.get_field(struct_name)? {
                Some(b"BANKID") => bank_id.set_with_value("BANKID", self.get_u32())?,
                Some(b"ACCTID") => account_number.set_with_value("ACCTID", self.get_u32())?,
                Some(b"ACCTTYPE") => {
                    account_type.set_with_value("ACCTTYPE", self.get_account_type())?
                }
                Some(key) => bail!("Unexpected key '{:?}'", key),
                None => break,
            }
        }

        account_number.ensure_field("ACCTID")?;
        Ok(())
    }

    fn check_balance(&self, struct_name: &[u8]) -> Result<()> {
        let mut amount = false;
        let mut timestamp = false;
        loop {
            match self.get_field(struct_name)? {
                Some(b"BALAMT") => amount.set_with_value("BALAMT", self.get_decimal())?,
                Some(b"DTASOF") => timestamp.set_with_value("DTASOF", self.get_timestamp())?,
                Some(key) => bail!("Unexpected key '{:?}'", key),
                None => break,
            }
        }

        amount.ensure_field("BALAMT")?;
        timestamp.ensure_field("DTASOF")?;

        Ok(())
    }

    fn get_account_to(&self) -> Result<AccountTo> {
        let mut account_id = None;
        loop {
            match self.get_field(b"CCACCTTO")? {
                Some(b"ACCTID") => account_id.put_or_else("ACCTID", self.get_u32())?,
                Some(key) => bail!("Unexpected key '{:?}'", key),
                None => break,
            }
        }

        let _ = account_id.ok_or_eyre("Missing key 'ACCTID'")?;
        Ok(AccountTo {})
    }

    fn get_key(&'a self) -> Result<&'a [u8]> {
        match self.get_token()? {
            QfxToken::OpenKey(key) => Ok(key),
            t => Err(eyre!("Expected key, got: {:?}", t)),
        }
    }

    fn get_field(&'a self, struct_name: &[u8]) -> Result<Option<&'a [u8]>> {
        match self.get_token()? {
            QfxToken::OpenKey(key) => Ok(Some(key)),
            QfxToken::CloseKey(k) if k == struct_name => Ok(None),
            t => Err(eyre!("Expected key, got: {:?}", t)),
        }
    }

    fn get_value(&'a self) -> Result<Cow<'a, str>> {
        match self.get_token()? {
            QfxToken::Value(value) => Ok(value),
            t => Err(eyre!("Expected value, got: {:?}", t)),
        }
    }

    fn get_token(&'a self) -> Result<QfxToken<'a>> {
        self.tokens.next()?.ok_or_eyre("Unexpected end of file")
    }

    fn get_u32(&self) -> Result<u32> {
        self.get_value()?
            .parse()
            .wrap_err("Failed to parse u32 value")
    }

    fn get_decimal(&self) -> Result<Decimal> {
        self.get_value()?
            .parse()
            .wrap_err("Failed to parse float value")
    }

    fn get_timestamp(&self) -> Result<DateTime<FixedOffset>> {
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
            let datetime = NaiveDateTime::parse_from_str(&value, "%Y%m%d%H%M%S%.f")
                .wrap_err("Failed to parse naive date value")?;

            (datetime, self.get_local_time())
        };

        offset
            .from_local_datetime(&timestamp)
            .single()
            .ok_or_eyre("Ambiguous date conversion")
    }

    fn get_timestamp_naive(&self) -> Result<NaiveDateTime> {
        let value = self.get_value()?;
        NaiveDateTime::parse_from_str(&value, "%Y%m%d%H%M%S%.f")
            .wrap_err("Failed to parse naive date value")
    }

    fn get_severity(&self) -> Result<Severity> {
        let value = self.get_value()?;
        match value.as_ref() {
            "INFO" => Ok(Severity::Info),
            v => Err(eyre!("Unexpected severity: '{}'", v)),
        }
    }

    fn check_currency(&self) -> Result<()> {
        let value = self.get_value()?;
        match value.as_ref() {
            "CAD" => Ok(()),
            v => Err(eyre!("Unexpected currency: '{}'", v)),
        }
    }

    fn get_account_type(&self) -> Result<AccountType> {
        let value = self.get_value()?;
        match value.as_ref() {
            "SAVINGS" => Ok(AccountType::Savings),
            v => Err(eyre!("Unexpected account type: '{}'", v)),
        }
    }

    fn get_transaction_type(&self) -> Result<QfxTransactionType> {
        let value = self.get_value()?;
        match value.as_ref() {
            "DEBIT" => Ok(QfxTransactionType::Debit),
            "CREDIT" => Ok(QfxTransactionType::Credit),
            "POS" => Ok(QfxTransactionType::Pos),
            "ATM" => Ok(QfxTransactionType::Atm),
            "FEE" => Ok(QfxTransactionType::Fee),
            "OTHER" => Ok(QfxTransactionType::Other),
            v => Err(eyre!("Unexpected transaction type: '{}'", v)),
        }
    }

    fn expect_done(&self) -> Result<()> {
        if let Some(v) = self.tokens.next()? {
            bail!("Unexpected token at end of file: {:?}", v);
        }
        Ok(())
    }

    fn get_local_time(&self) -> FixedOffset {
        match self.local_timezone.get() {
            Some(t) => t,
            None => {
                let local_timezone = *Local::now().offset();
                self.local_timezone.set(Some(local_timezone));
                local_timezone
            }
        }
    }
}
