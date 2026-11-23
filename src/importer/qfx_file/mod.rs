// Compatible with Tangerine and Capital One QFX files

/// File header parsing.
mod header;
/// Tokenizing of the document body.
mod lexer;

use std::borrow::Cow;
use std::cell::{Cell, OnceCell};
use std::path::Path;

use chrono::{DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, TimeZone};
use color_eyre::Result;
use color_eyre::eyre::{Context, OptionExt, bail, eyre};
use rust_decimal::Decimal;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

use crate::importer::qfx_file::header::StringEncoding;
use crate::importer::qfx_file::lexer::{Key, Lexer, QfxToken, Value};
use crate::importer::{
    ImportProgress, Transaction, TransactionImporter, TransactionReader, TransactionType,
};

thread_local! {
    pub static LOCAL_TIMEZONE: Cell<FixedOffset> = Cell::new(*Local::now().offset());
}

/// Reads transactions from a QFX/OFX file.
pub struct QfxReader {
    /// The document body after the header.
    contents: Vec<u8>,
    /// Whether the file is OFX 2 (XML) rather than OFX 1 (SGML).
    is_xml: bool,
    /// Encoding declared by the header.
    encoding: StringEncoding,
}

impl QfxReader {
    /// Open a QFX file and parse its header.
    ///
    /// # Errors
    /// Fails if the file cannot be read or its header is unsupported.
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
    async fn load(
        self,
        mut importer: TransactionImporter<'_, '_>,
        progress: &ImportProgress,
    ) -> Result<()> {
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

            progress.add_transaction();
        }

        Ok(())
    }
}

/// Read a type from a lexer
///
/// This should expect it's outermost field to have been opened, and must read until it's close
/// Implementers of this trait must know the keys of any closing tags
trait ReadQfx<'a> {
    /// Read the value, consuming tokens up to and including its closing tag.
    fn read(lexer: &'a Lexer) -> Result<Self>
    where
        Self: Sized;
}

/// Read a type from a lexer
///
/// This should expect it's outermost field to have been opened, and must read until it's close
/// This trait supplies the name of the opening tag, so it can be used to match a closing tag
/// This should be used for multiple fields with the same schema
trait ReadQfxVariable<'a> {
    /// Read the value opened by `key`, consuming tokens up to and including its closing tag.
    fn read<'k>(lexer: &'a Lexer, key: Key<'k>) -> Result<Self>
    where
        Self: Sized;
}

/// Helper type for reading value fields
struct ValueOption<T> {
    /// The field name, for error messages.
    key: Key<'static>,
    /// The value, once read.
    value: Option<T>,
}

impl<T> ValueOption<T> {
    /// An empty slot for the field `key`.
    fn new<K: Into<Key<'static>>>(key: K) -> Self {
        Self {
            key: key.into(),
            value: None,
        }
    }

    /// The value, or an error naming the missing field.
    fn ok(self) -> Result<T> {
        self.value
            .ok_or_else(|| eyre!("Missing field \"{}\"", self.key))
    }

    /// The value, if present.
    fn option(self) -> Option<T> {
        self.value
    }
}

impl<'a, T: ReadQfx<'a>> ValueOption<T> {
    /// Read the value from `lexer`, failing if it was already set.
    fn fill(&mut self, lexer: &'a Lexer) -> Result<()> {
        match self.value {
            Some(_) => Err(eyre!("Duplicate field \"{}\"", self.key)),
            None => {
                self.value = Some(
                    T::read(lexer)
                        .wrap_err_with(|| eyre!("Error parsing field \"{}\"", self.key))?,
                );
                Ok(())
            }
        }
    }
}

/// Helper type for reading ignore fields
struct FieldFlag {
    /// The field name, set on first use for unnamed flags.
    key: OnceCell<Key<'static>>,
    /// Whether the field has been seen.
    value: Cell<bool>,
}

impl FieldFlag {
    /// A flag for the field `key`.
    fn new<K: Into<Key<'static>>>(key: K) -> Self {
        Self {
            key: OnceCell::from(key.into()),
            value: Cell::new(false),
        }
    }

    /// A flag whose field name is set by the first [`Self::check_var`].
    fn unnamed() -> Self {
        Self {
            key: OnceCell::new(),
            value: Cell::new(false),
        }
    }

    /// Read and discard the field, failing if it was already seen.
    fn check<'a, T: ReadQfx<'a>>(&self, lexer: &'a Lexer) -> Result<()> {
        let key_name = self.key.get().ok_or_eyre("Field flag missing name")?;

        if self.value.get() {
            bail!("Duplicate field \"{}\"", key_name);
        }

        let _ = T::read(lexer).wrap_err_with(|| eyre!("Error parsing field \"{}\"", key_name))?;
        self.value.set(true);
        Ok(())
    }

    /// Read and discard a field which may appear under one of several names, failing if it was already seen.
    fn check_var<'a, K: Into<Key<'static>>, T: ReadQfxVariable<'a>>(
        &self,
        key: K,
        lexer: &'a Lexer,
    ) -> Result<()> {
        let key = key.into();
        match self.key.get() {
            Some(k) if *k == key => {}
            Some(k) => bail!("Attempt to set second key for field flag \"{}\"", k),
            None => self.key.set(key).unwrap(),
        }

        if self.value.get() {
            bail!("Duplicate field \"{}\"", key);
        }

        let _ = T::read(lexer, key).wrap_err_with(|| eyre!("Error parsing field \"{}\"", key))?;
        self.value.set(true);
        Ok(())
    }
}

impl<'a> ReadQfx<'a> for Value<'a> {
    fn read(lexer: &'a Lexer) -> Result<Self> {
        lexer.expect_value()
    }
}

impl ReadQfx<'_> for u32 {
    fn read(lexer: &Lexer) -> Result<Self> {
        lexer
            .expect_value()?
            .parse()
            .wrap_err("Failed to parse u32 value")
    }
}

impl ReadQfx<'_> for u64 {
    fn read(lexer: &Lexer) -> Result<Self> {
        lexer
            .expect_value()?
            .parse()
            .wrap_err("Failed to parse u32 value")
    }
}

impl ReadQfx<'_> for Decimal {
    fn read(lexer: &Lexer) -> Result<Self> {
        lexer
            .expect_value()?
            .parse()
            .wrap_err("Failed to parse decimal value")
    }
}

impl ReadQfx<'_> for DateTime<FixedOffset> {
    fn read(lexer: &'_ Lexer) -> Result<Self> {
        let value = lexer.expect_value()?;

        let (timestamp, offset) = if value.ends_with(']') {
            let mut datetime_parts = value.split('[');
            let datetime_str = datetime_parts
                .next()
                .ok_or_eyre("Timestamp missing start of timezone block")?;

            let datetime = NaiveDateTime::parse_from_str(datetime_str, "%Y%m%d%H%M%S%.f")
                .wrap_err("Failed to parse timestamp")?;

            let timezone_offset_str = datetime_parts
                .next()
                .ok_or_eyre("Timestamp missing timezone block")?
                .split("]")
                .next()
                .unwrap()
                .split(':')
                .next()
                .unwrap();
            let offset_hours = timezone_offset_str
                .parse::<i8>()
                .wrap_err_with(|| format!("Invalid timezone offset: {:?}", timezone_offset_str))?;

            let offset = FixedOffset::east_opt(offset_hours as i32 * 60 * 60)
                .ok_or_eyre("Out of bounds timezone offset")?;

            (datetime, offset)
        } else {
            // Fallback to assuming this is local time. This will have annoying daylight savings time implications
            let datetime = NaiveDateTime::parse_from_str(&value, "%Y%m%d%H%M%S%.f")
                .or(NaiveDate::parse_from_str(&value, "%Y%m%d")
                    .map(|d| d.and_hms_opt(0, 0, 0).unwrap()))
                .wrap_err("Failed to parse naive date value")?;

            (datetime, LOCAL_TIMEZONE.get())
        };

        offset
            .from_local_datetime(&timestamp)
            .single()
            .ok_or_eyre("Ambiguous date conversion")
    }
}

impl ReadQfx<'_> for NaiveDateTime {
    fn read(lexer: &'_ Lexer) -> Result<Self> {
        let value = lexer.expect_value()?;
        Self::parse_from_str(&value, "%Y%m%d%H%M%S%.f").wrap_err("Failed to parse naive date value")
    }
}

/// Status severity.
#[derive(Debug)]
enum Severity {
    /// Informational.
    Info,
}

impl ReadQfx<'_> for Severity {
    fn read(lexer: &Lexer) -> Result<Self> {
        match lexer.expect_value()?.as_ref() {
            "INFO" => Ok(Self::Info),
            v => Err(eyre!("Unknown severity: \"{v}\"")),
        }
    }
}

/// Response language.
#[derive(Debug)]
enum Language {
    /// English (`ENG`).
    English,
}

impl ReadQfx<'_> for Language {
    fn read(lexer: &Lexer) -> Result<Self> {
        match lexer.expect_value()?.as_ref() {
            "ENG" => Ok(Self::English),
            v => Err(eyre!("Unknown language: \"{v}\"")),
        }
    }
}

/// Statement currency.
#[derive(Debug)]
enum Currency {
    /// Canadian dollars (`CAD`).
    CanadianDollar,
}

impl<'a> ReadQfx<'a> for Currency {
    fn read(lexer: &'a Lexer) -> Result<Self> {
        match lexer.expect_value()?.as_ref() {
            "CAD" => Ok(Self::CanadianDollar),
            v => Err(eyre!("Unknown currency: \"{v}\"")),
        }
    }
}

/// Bank account type.
#[derive(Debug)]
pub enum AccountType {
    /// A chequing account.
    Chequing,
    /// A savings account.
    Savings,
}

impl ReadQfx<'_> for AccountType {
    fn read(lexer: &Lexer) -> Result<Self> {
        match lexer.expect_value()?.as_ref() {
            "CHECKING" => Ok(Self::Chequing),
            "SAVINGS" => Ok(Self::Savings),
            v => Err(eyre!("Unknown account type: \"{v}\"")),
        }
    }
}

/// The `TRNTYPE` of a statement transaction.
#[derive(Debug)]
pub enum QfxTransactionType {
    /// Generic debit.
    Debit,
    /// Generic credit.
    Credit,
    /// Point of sale purchase.
    Pos,
    /// ATM transaction.
    Atm,
    /// Bank fee.
    Fee,
    /// Anything else.
    Other,
}

impl ReadQfx<'_> for QfxTransactionType {
    fn read(lexer: &Lexer) -> Result<Self> {
        match lexer.expect_value()?.as_ref() {
            "DEBIT" => Ok(Self::Debit),
            "CREDIT" => Ok(Self::Credit),
            "POS" => Ok(Self::Pos),
            "ATM" => Ok(Self::Atm),
            "FEE" => Ok(Self::Fee),
            "OTHER" => Ok(Self::Other),
            v => Err(eyre!("Unknown transaction type: \"{v}\"")),
        }
    }
}

/// A `STATUS` aggregate.
#[allow(unused)]
#[derive(Debug)]
struct Status<'a> {
    /// Status code (0 means success).
    code: u32,
    /// Severity.
    severity: Severity,
    /// Optional message.
    message: Option<Value<'a>>,
}

impl<'a> ReadQfx<'a> for Status<'a> {
    /// Read the tokens
    fn read(lexer: &'a Lexer) -> Result<Self> {
        let mut code = ValueOption::new(b"CODE");
        let mut severity = ValueOption::new(b"SEVERITY");
        let mut message = ValueOption::new(b"MESSAGE");
        loop {
            match lexer.expect_field(b"STATUS")? {
                Some(Key(b"CODE")) => code.fill(lexer)?,
                Some(Key(b"SEVERITY")) => severity.fill(lexer)?,
                Some(Key(b"MESSAGE")) => message.fill(lexer)?,
                Some(key) => bail!("Unexpected key \"{}\"", key),
                None => break,
            }
        }

        Ok(Self {
            code: code.ok()?,
            severity: severity.ok()?,
            message: message.option(),
        })
    }
}

/// The `FI` aggregate identifying the institution.
#[allow(unused)]
#[derive(Debug)]
struct FinancialInstitution<'a> {
    /// Organization name.
    organization: Value<'a>,
    /// Institution id.
    institution_id: u32,
}

impl<'a> ReadQfx<'a> for FinancialInstitution<'a> {
    fn read(lexer: &'a Lexer) -> Result<Self> {
        let mut organization = ValueOption::new(b"ORG");
        let mut institution_id = ValueOption::new(b"FID");
        loop {
            match lexer.expect_field(b"FI")? {
                Some(Key(b"ORG")) => organization.fill(lexer)?,
                Some(Key(b"FID")) => institution_id.fill(lexer)?,
                Some(key) => bail!("Unexpected key \"{}\"", key),
                None => break,
            }
        }

        Ok(Self {
            organization: organization.ok()?,
            institution_id: institution_id.ok()?,
        })
    }
}

/// The `SONRS` sign-on response.
#[allow(unused)]
#[derive(Debug)]
struct SignOnResponse<'a> {
    /// Status.
    status: Status<'a>,
    /// Server time.
    server_date: DateTime<FixedOffset>,
    /// Response language.
    language: Language,
    /// Last profile update.
    last_profile_update: Option<DateTime<FixedOffset>>,
    /// Institution, if reported.
    financial_institution: Option<FinancialInstitution<'a>>,
    /// Intuit bank id (`INTU.BID`).
    bank_id: u32,
}

impl<'a> ReadQfx<'a> for SignOnResponse<'a> {
    fn read(lexer: &'a Lexer) -> Result<Self> {
        let mut status = ValueOption::new(b"STATUS");
        let mut server_date = ValueOption::new(b"DTSERVER");
        let mut language = ValueOption::new(b"LANGUAGE");
        let mut last_profile_update = ValueOption::new(b"DTPROFUP");
        let mut financial_institution = ValueOption::new(b"FI");
        let mut bank_id = ValueOption::new(b"INTU.BID");
        loop {
            match lexer.expect_field(b"SONRS")? {
                Some(Key(b"STATUS")) => status.fill(lexer)?,
                Some(Key(b"DTSERVER")) => server_date.fill(lexer)?,
                Some(Key(b"LANGUAGE")) => language.fill(lexer)?,
                Some(Key(b"DTPROFUP")) => last_profile_update.fill(lexer)?,
                Some(Key(b"FI")) => financial_institution.fill(lexer)?,
                Some(Key(b"INTU.BID")) => bank_id.fill(lexer)?,
                Some(key) => bail!("Unexpected key \"{}\"", key),
                None => break,
            }
        }

        Ok(Self {
            status: status.ok()?,
            server_date: server_date.ok()?,
            language: language.ok()?,
            last_profile_update: last_profile_update.option(),
            financial_institution: financial_institution.option(),
            bank_id: bank_id.ok()?,
        })
    }
}

/// The `SIGNONMSGSRSV1` aggregate.
#[allow(unused)]
#[derive(Debug)]
struct SignOnMessageResponseV1<'a> {
    /// The sign-on response.
    response: SignOnResponse<'a>,
}

impl<'a> ReadQfx<'a> for SignOnMessageResponseV1<'a> {
    fn read(lexer: &'a Lexer) -> Result<Self> {
        let mut response = ValueOption::new(b"SONRS");
        loop {
            match lexer.expect_field(b"SIGNONMSGSRSV1")? {
                Some(Key(b"SONRS")) => response.fill(lexer)?,
                Some(key) => bail!("Unexpected key \"{}\"", key),
                None => break,
            }
        }

        Ok(Self {
            response: response.ok()?,
        })
    }
}

/// The `BANKACCTFROM`/`CCACCTFROM` account identification.
#[allow(unused)]
#[derive(Debug)]
struct AccountFrom {
    /// Account number.
    account_id: u64,
    /// Bank routing id.
    bank_id: Option<u32>,
    /// Account type.
    account_type: Option<AccountType>,
}

impl ReadQfxVariable<'_> for AccountFrom {
    fn read<'k>(lexer: &Lexer, key: Key<'k>) -> Result<Self> {
        let mut bank_id = ValueOption::new(b"BANKID");
        let mut account_id = ValueOption::new(b"ACCTID");
        let mut account_type = ValueOption::new(b"ACCTTYPE");
        loop {
            match lexer.expect_field(key)? {
                Some(Key(b"BANKID")) => bank_id.fill(lexer)?,
                Some(Key(b"ACCTID")) => account_id.fill(lexer)?,
                Some(Key(b"ACCTTYPE")) => account_type.fill(lexer)?,
                Some(key) => bail!("Unexpected key \"{}\"", key),
                None => break,
            }
        }

        Ok(Self {
            account_id: account_id.ok()?,
            bank_id: bank_id.option(),
            account_type: account_type.option(),
        })
    }
}

/// A `LEDGERBAL`/`AVAILBAL` balance.
#[allow(unused)]
#[derive(Debug)]
struct Balance {
    /// Balance amount.
    amount: Decimal,
    /// When the balance applied.
    timestamp: DateTime<FixedOffset>,
}

impl ReadQfxVariable<'_> for Balance {
    fn read<'k>(lexer: &'_ Lexer, key: Key<'k>) -> Result<Self> {
        let mut amount = ValueOption::new(b"BALAMT");
        let mut timestamp = ValueOption::new(b"DTASOF");
        loop {
            match lexer.expect_field(key)? {
                Some(Key(b"BALAMT")) => amount.fill(lexer)?,
                Some(Key(b"DTASOF")) => timestamp.fill(lexer)?,
                Some(key) => bail!("Unexpected key \"{}\"", key),
                None => break,
            }
        }

        Ok(Self {
            amount: amount.ok()?,
            timestamp: timestamp.ok()?,
        })
    }
}

/// The `CCACCTTO` transfer destination.
#[allow(unused)]
#[derive(Debug)]
struct AccountTo {
    /// Destination account number.
    account_id: u32,
}

impl ReadQfx<'_> for AccountTo {
    fn read(lexer: &'_ Lexer) -> Result<Self> {
        let mut account_id = ValueOption::new(b"ACCTID");
        loop {
            match lexer.expect_field(b"CCACCTTO")? {
                Some(Key(b"ACCTID")) => account_id.fill(lexer)?,
                Some(key) => bail!("Unexpected key \"{}\"", key),
                None => break,
            }
        }

        Ok(Self {
            account_id: account_id.ok()?,
        })
    }
}

/// A `STMTTRN` statement transaction.
#[derive(Debug)]
pub struct StatementTransaction<'a> {
    /// Transaction type.
    transaction_type: QfxTransactionType,
    /// Posted date.
    date_posted: DateTime<FixedOffset>,
    // user_date: Option<NaiveDateTime>,
    /// Signed amount.
    amount: Decimal,
    /// Bank transaction id (`FITID`).
    transaction_id: Cow<'a, str>,
    /// Transaction name.
    name: Cow<'a, str>,
    // account_to: Option<AccountTo>,
    /// Memo, if any.
    memo: Option<Cow<'a, str>>,
}

impl<'a> ReadQfx<'a> for StatementTransaction<'a> {
    fn read(lexer: &'a Lexer) -> Result<Self> {
        let mut transaction_type = ValueOption::new(b"TRNTYPE");
        let mut date_posted = ValueOption::new(b"DTPOSTED");
        let mut user_date: ValueOption<NaiveDateTime> = ValueOption::new(b"DTUSER");
        let mut amount = ValueOption::new(b"TRNAMT");
        let mut transaction_id = ValueOption::new(b"FITID");
        let mut name = ValueOption::new(b"NAME");
        let mut account_to: ValueOption<AccountTo> = ValueOption::new(b"CCACCTTO");
        let mut memo = ValueOption::new(b"MEMO");

        loop {
            match lexer.expect_field(b"STMTTRN")? {
                Some(Key(b"TRNTYPE")) => transaction_type.fill(lexer)?,
                Some(Key(b"DTPOSTED")) => date_posted.fill(lexer)?,
                Some(Key(b"DTUSER")) => user_date.fill(lexer)?,
                Some(Key(b"TRNAMT")) => amount.fill(lexer)?,
                Some(Key(b"FITID")) => transaction_id.fill(lexer)?,
                Some(Key(b"NAME")) => name.fill(lexer)?,
                Some(Key(b"CCACCTTO")) => account_to.fill(lexer)?,
                Some(Key(b"MEMO")) => memo.fill(lexer)?,
                Some(key) => bail!("Unexpected key \"{}\"", key),
                None => break,
            }
        }

        let _ = user_date;
        let _ = account_to;

        Ok(Self {
            transaction_type: transaction_type.ok()?,
            date_posted: date_posted.ok()?,
            amount: amount.ok()?,
            transaction_id: transaction_id.ok()?,
            name: name.ok()?,
            memo: memo.option(),
        })
    }
}

/// Position of the parser within the document structure.
#[derive(Debug, Clone, Copy)]
enum ParserState {
    /// Before the root `OFX` tag.
    NotStarted,
    /// Inside `OFX`.
    InOfx,
    /// Inside an institution message set (bank or credit card).
    InInstitutionMessage(Key<'static>),
    /// Inside a statement transaction response.
    InStatementTransactionResponse(Key<'static>, Key<'static>),
    /// Inside a statement response.
    InStatementResponse(Key<'static>, Key<'static>, Key<'static>),
    /// Inside the transaction list.
    ReadTransactionList(Key<'static>, Key<'static>, Key<'static>),
    /// The document has been fully read.
    Done,
}

/// Streams statement transactions out of an OFX document.
pub struct DocumentParser {
    /// Token source.
    tokens: Lexer,
    /// Current position.
    state: Cell<ParserState>,
    /// Sign-on response seen.
    sign_on_message_response_seen: FieldFlag,
    /// Transaction id seen.
    transaction_id_seen: FieldFlag,
    /// Status seen.
    status_seen: FieldFlag,
    /// Currency seen.
    currency_seen: FieldFlag,
    /// Account identification seen.
    account_from_seen: FieldFlag,
    /// Ledger balance seen.
    ledger_balance_seen: FieldFlag,
    /// Available balance seen.
    available_balance_seen: FieldFlag,
    /// List start date seen.
    start_date_seen: FieldFlag,
    /// List end date seen.
    end_date_seen: FieldFlag,
}

impl<'a> DocumentParser {
    /// Create a parser over `lexer`.
    fn new(lexer: Lexer) -> Self {
        Self {
            tokens: lexer,
            state: Cell::new(ParserState::NotStarted),
            sign_on_message_response_seen: FieldFlag::new(b"SIGNONMSGSRSV1"),
            transaction_id_seen: FieldFlag::new(b"TRNUID"),
            status_seen: FieldFlag::new(b"STATUS"),
            currency_seen: FieldFlag::new(b"CURDEF"),
            account_from_seen: FieldFlag::unnamed(),
            ledger_balance_seen: FieldFlag::new(b"LEDGERBAL"),
            available_balance_seen: FieldFlag::new(b"AVAILBAL"),
            start_date_seen: FieldFlag::new(b"DTSTART"),
            end_date_seen: FieldFlag::new(b"DTEND"),
        }
    }

    /// Read up to and return the next statement transaction, or `None` at the end of the document.
    ///
    /// # Errors
    /// Fails on malformed or unexpected document structure.
    fn next_statement_transaction(&'a self) -> Result<Option<StatementTransaction<'a>>> {
        loop {
            match self.state.get() {
                ParserState::NotStarted => match self.tokens.next()? {
                    QfxToken::OpenKey(Key(b"OFX")) => self.state.set(ParserState::InOfx),
                    t => bail!("Expected {}, got: {}", QfxToken::OpenKey(Key(b"QFX")), t),
                },
                ParserState::InOfx => match self.tokens.expect_field(b"OFX")? {
                    Some(Key(b"SIGNONMSGSRSV1")) => {
                        self.sign_on_message_response_seen
                            .check::<SignOnMessageResponseV1>(&self.tokens)?
                    }
                    Some(Key(b"BANKMSGSRSV1")) => self
                        .state
                        .set(ParserState::InInstitutionMessage(Key(b"BANKMSGSRSV1"))),
                    Some(Key(b"CREDITCARDMSGSRSV1")) => self.state.set(
                        ParserState::InInstitutionMessage(Key(b"CREDITCARDMSGSRSV1")),
                    ),
                    Some(key) => bail!(
                        "Unexpected key \"{}\" for state {:?}",
                        key,
                        self.state.get()
                    ),
                    None => {
                        self.tokens.expect_none()?;
                        self.state.set(ParserState::Done);
                    }
                },
                ParserState::InInstitutionMessage(k) => match self.tokens.expect_field(k)? {
                    Some(Key(b"STMTTRNRS")) => self.state.set(
                        ParserState::InStatementTransactionResponse(k, Key(b"STMTTRNRS")),
                    ),
                    Some(Key(b"CCSTMTTRNRS")) => self.state.set(
                        ParserState::InStatementTransactionResponse(k, Key(b"CCSTMTTRNRS")),
                    ),
                    Some(key) => bail!(
                        "Unexpected key \"{}\" for state {:?}",
                        key,
                        self.state.get()
                    ),
                    None => self.state.set(ParserState::InOfx),
                },
                ParserState::InStatementTransactionResponse(k1, k2) => {
                    match self.tokens.expect_field(k2)? {
                        Some(Key(b"TRNUID")) => {
                            self.transaction_id_seen.check::<Value>(&self.tokens)?
                        }
                        Some(Key(b"STATUS")) => self.status_seen.check::<Status>(&self.tokens)?,
                        Some(Key(b"STMTRS")) => {
                            self.state
                                .set(ParserState::InStatementResponse(k1, k2, Key(b"STMTRS")))
                        }
                        Some(Key(b"CCSTMTRS")) => self.state.set(ParserState::InStatementResponse(
                            k1,
                            k2,
                            Key(b"CCSTMTRS"),
                        )),
                        Some(key) => bail!(
                            "Unexpected key \"{}\" for state {:?}",
                            key,
                            self.state.get()
                        ),
                        None => self.state.set(ParserState::InInstitutionMessage(k1)),
                    }
                }
                ParserState::InStatementResponse(k1, k2, k3) => {
                    match self.tokens.expect_field(k3)? {
                        Some(Key(b"CURDEF")) => {
                            self.currency_seen.check::<Currency>(&self.tokens)?
                        }
                        Some(Key(b"BANKACCTFROM")) => self
                            .account_from_seen
                            .check_var::<_, AccountFrom>(b"BANKACCTFROM", &self.tokens)?,
                        Some(Key(b"CCACCTFROM")) => self
                            .account_from_seen
                            .check_var::<_, AccountFrom>(b"CCACCTFROM", &self.tokens)?,
                        Some(Key(b"BANKTRANLIST")) => {
                            self.state.set(ParserState::ReadTransactionList(k1, k2, k3))
                        }
                        Some(Key(b"LEDGERBAL")) => self
                            .ledger_balance_seen
                            .check_var::<_, Balance>(b"LEDGERBAL", &self.tokens)?,
                        Some(Key(b"AVAILBAL")) => self
                            .available_balance_seen
                            .check_var::<_, Balance>(b"AVAILBAL", &self.tokens)?,
                        Some(key) => bail!(
                            "Unexpected key \"{}\" for state {:?}",
                            key,
                            self.state.get()
                        ),
                        None => self
                            .state
                            .set(ParserState::InStatementTransactionResponse(k1, k2)),
                    }
                }
                ParserState::ReadTransactionList(k1, k2, k3) => {
                    match self.tokens.expect_field(b"BANKTRANLIST")? {
                        Some(Key(b"DTSTART")) => self
                            .start_date_seen
                            .check::<DateTime<FixedOffset>>(&self.tokens)?,
                        Some(Key(b"DTEND")) => self
                            .end_date_seen
                            .check::<DateTime<FixedOffset>>(&self.tokens)?,
                        Some(Key(b"STMTTRN")) => {
                            return StatementTransaction::read(&self.tokens)
                                .map(Some)
                                .wrap_err("Error parsing field \"STMTTRN\"");
                        }
                        Some(key) => bail!("Unexpected key '{:?}' for state {:?}", key, self.state),
                        None => self.state.set(ParserState::InStatementResponse(k1, k2, k3)),
                    }
                }
                ParserState::Done => return Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An OFX 1.02 (SGML) bank statement with sign-on and balance aggregates.
    const SGML: &str = "OFXHEADER:100\nDATA:OFXSGML\nVERSION:102\nSECURITY:NONE\nENCODING:USASCII\n\
        CHARSET:1252\nCOMPRESSION:NONE\nOLDFILEUID:NONE\nNEWFILEUID:NONE\n\n\
        <OFX><SIGNONMSGSRSV1><SONRS><STATUS><CODE>0<SEVERITY>INFO<MESSAGE>OK</STATUS>\
        <DTSERVER>20240105120000.000[-5:EST]<LANGUAGE>ENG<DTPROFUP>20240101120000.000[-5:EST]\
        <FI><ORG>Bank<FID>123</FI><INTU.BID>456</SONRS></SIGNONMSGSRSV1>\
        <BANKMSGSRSV1><STMTTRNRS><TRNUID>1<STATUS><CODE>0<SEVERITY>INFO</STATUS>\
        <STMTRS><CURDEF>CAD<BANKACCTFROM><BANKID>1<ACCTID>789<ACCTTYPE>SAVINGS</BANKACCTFROM>\
        <BANKTRANLIST><DTSTART>20240101120000<DTEND>20240131120000\
        <STMTTRN><TRNTYPE>POS<DTPOSTED>20240102120000.000[-5:EST]<DTUSER>20240102120000\
        <TRNAMT>-12.34<FITID>1<NAME>POS CAFE<CCACCTTO><ACCTID>0</CCACCTTO><MEMO>coffee</STMTTRN>\
        <STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20240103120000<DTUSER>20240103120000\
        <TRNAMT>100.00<FITID>2<NAME>PAYROLL {LQ}ACME{RQ}<CCACCTTO><ACCTID>0</CCACCTTO></STMTTRN>\
        </BANKTRANLIST><LEDGERBAL><BALAMT>87.66<DTASOF>20240131120000.000[-5:EST]</LEDGERBAL>\
        <AVAILBAL><BALAMT>87.66<DTASOF>20240131120000.000[-5:EST]</AVAILBAL>\
        </STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>\n";

    /// Write `text` to a file, replacing `{LQ}`/`{RQ}` with Windows-1252 curly quote bytes.
    async fn open_text(text: &[u8]) -> Result<QfxReader> {
        let text = String::from_utf8_lossy(text);
        let mut bytes = Vec::new();
        for (idx, part) in text.split("{LQ}").enumerate() {
            if idx > 0 {
                bytes.push(0x93);
            }
            for (idx, part) in part.split("{RQ}").enumerate() {
                if idx > 0 {
                    bytes.push(0x94);
                }
                bytes.extend_from_slice(part.as_bytes());
            }
        }
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("statement.qfx");
        std::fs::write(&path, bytes)?;
        QfxReader::open(&path).await
    }

    /// (type, amount, name, memo) of a parsed transaction.
    type Parsed = (QfxTransactionType, Decimal, String, Option<String>);

    /// Parse every statement transaction in `reader`.
    fn transactions(reader: QfxReader) -> Result<Vec<Parsed>> {
        let lexer = Lexer::new(reader.contents, reader.encoding, reader.is_xml);
        let parser = DocumentParser::new(lexer);
        let mut out = Vec::new();
        while let Some(t) = parser.next_statement_transaction()? {
            out.push((
                t.transaction_type,
                t.amount,
                t.name.into_owned(),
                t.memo.map(Cow::into_owned),
            ));
        }
        Ok(out)
    }

    #[tokio::test]
    async fn parses_sgml_statement() {
        let reader = open_text(SGML.as_bytes()).await.unwrap();
        assert!(!reader.is_xml);
        assert_eq!(reader.encoding, StringEncoding::Windows1252);
        let parsed = transactions(reader).unwrap();
        assert_eq!(parsed.len(), 2);
        assert!(matches!(parsed[0].0, QfxTransactionType::Pos));
        assert_eq!(parsed[0].1, Decimal::new(-1234, 2));
        assert_eq!(parsed[0].2, "POS CAFE");
        assert_eq!(parsed[0].3.as_deref(), Some("coffee"));
        assert!(matches!(parsed[1].0, QfxTransactionType::Credit));
        // Windows-1252 smart quotes are decoded.
        assert_eq!(parsed[1].2, "PAYROLL \u{201c}ACME\u{201d}");
    }

    #[tokio::test]
    async fn rejects_bad_documents() {
        assert!(open_text(b"").await.is_err(), "empty file");
        assert!(open_text(b"\x01").await.is_err(), "binary file");
        assert!(
            open_text(SGML.replace("VERSION:102", "VERSION:103").as_bytes())
                .await
                .is_err(),
            "unsupported version"
        );
        assert!(
            open_text(SGML.replace("OFXHEADER:100", "OFXHEADER:200").as_bytes())
                .await
                .is_err(),
            "unsupported header"
        );
        for bad in [
            SGML.replace("<TRNTYPE>POS", "<TRNTYPE>WIRE"),
            SGML.replace("<CURDEF>CAD", "<CURDEF>USD"),
            SGML.replace("<LANGUAGE>ENG", "<LANGUAGE>FRA"),
            SGML.replace("<SEVERITY>INFO<MESSAGE>", "<SEVERITY>ERROR<MESSAGE>"),
            SGML.replace("<ACCTTYPE>SAVINGS", "<ACCTTYPE>MONEYMRKT"),
            SGML.replace("<TRNAMT>-12.34", "<TRNAMT>abc"),
            SGML.replace("<FITID>1", ""),
            SGML.replace("<CURDEF>CAD", "<CURDEF>CAD<CURDEF>CAD"),
            SGML.replace("<DTSTART>20240101120000", "<BOGUS>1"),
            SGML.replace("<DTPOSTED>20240102120000.000[-5:EST]", "<DTPOSTED>2024"),
        ] {
            let reader = open_text(bad.as_bytes()).await.unwrap();
            assert!(transactions(reader).is_err());
        }
    }

    /// Variations seen in RBC `.ofx` exports.
    #[tokio::test]
    async fn accepts_rbc_variations() {
        let rbc = SGML
            .replace("SECURITY:NONE", "SECURITY:TYPE1")
            .replace("<FI><ORG>Bank<FID>123</FI>", "")
            .replace(
                "<ACCTID>789<ACCTTYPE>SAVINGS",
                "<ACCTID>12345678901<ACCTTYPE>CHECKING",
            )
            .replace("<TRNUID>1", "<TRNUID>RBC-1")
            .replace("<DTPOSTED>20240102120000.000[-5:EST]", "<DTPOSTED>20240102")
            .replace("<DTUSER>20240102120000", "")
            .replace("<CCACCTTO><ACCTID>0</CCACCTTO>", "");
        let parsed = transactions(open_text(rbc.as_bytes()).await.unwrap()).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].1, Decimal::new(-1234, 2));
    }
}
