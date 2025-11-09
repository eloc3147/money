// Compatible with Capital One CSV files

use std::borrow::Cow;
use std::fs::File;
use std::path::Path;

use chrono::NaiveDate;
use color_eyre::Result;
use color_eyre::eyre::{Context, OptionExt, bail};
use csv::{Reader, StringRecord, StringRecordsIter};
use rust_decimal::Decimal;

use crate::importer::{Transaction, TransactionType};

pub struct CsvTransaction {
    // transaction_date: NaiveDate,
    posted_date: NaiveDate,
    // card_number: u16,
    description: String,
    category: String,
    debit: Option<Decimal>,
    credit: Option<Decimal>,
}

impl<'a> CsvTransaction {
    fn into_transaction(self) -> Result<Transaction<'a>> {
        let (transaction_type, amount) = match (self.debit, self.credit) {
            (Some(debit), None) => (TransactionType::Debit, -debit),
            (None, Some(credit)) => (TransactionType::Credit, credit),
            (Some(_), Some(_)) => {
                bail!("Cannot convert CsvTransaction with both debit and credit values")
            }
            (None, None) => bail!("Cannot convert CsvTransaction without a debit or credit value"),
        };

        Ok(Transaction {
            transaction_type,
            date_posted: self.posted_date,
            amount,
            transaction_id: None,
            category: Some(Cow::Owned(self.category)),
            name: Cow::Owned(self.description),
            memo: None,
        })
    }
}

pub struct CsvReader {
    reader: Reader<File>,
    columns: ColumnMap,
}

impl<'a> CsvReader {
    pub fn open(path: &Path) -> Result<Self> {
        let mut reader = Reader::from_reader(File::open(path).wrap_err("Failed to open file")?);

        let mut transaction_date_col = None;
        let mut posted_date_col = None;
        let mut card_number_col = None;
        let mut description_col = None;
        let mut category_col = None;
        let mut debit_col = None;
        let mut credit_col = None;
        let headers = reader.headers().wrap_err("Failed to read headers")?;
        for (idx, header) in headers.iter().enumerate() {
            match header.trim() {
                "Transaction Date" => {
                    if transaction_date_col.is_some() {
                        bail!("Multiple columns match transaction date")
                    }
                    transaction_date_col = Some(idx);
                }
                "Posted Date" => {
                    if posted_date_col.is_some() {
                        bail!("Multiple columns match posted date")
                    }
                    posted_date_col = Some(idx);
                }
                "Card No." => {
                    if card_number_col.is_some() {
                        bail!("Multiple columns match card number")
                    }
                    card_number_col = Some(idx);
                }
                "Description" => {
                    if description_col.is_some() {
                        bail!("Multiple columns match description")
                    }
                    description_col = Some(idx);
                }
                "Category" => {
                    if category_col.is_some() {
                        bail!("Multiple columns match category")
                    }
                    category_col = Some(idx);
                }
                "Debit" => {
                    if debit_col.is_some() {
                        bail!("Multiple columns match debit")
                    }
                    debit_col = Some(idx);
                }
                "Credit" => {
                    if credit_col.is_some() {
                        bail!("Multiple columns match credit")
                    }
                    credit_col = Some(idx);
                }
                h => bail!("Unrecognized header: \"{}\"", h),
            }
        }

        let columns = ColumnMap {
            // transaction_date_col: transaction_date_col.ok_or_eyre("File missing transaction date column")?,
            posted_date_col: posted_date_col.ok_or_eyre("File missing posted date column")?,
            // card_number_col: card_number_col.ok_or_eyre("File missing card number column")?,
            description_col: description_col.ok_or_eyre("File missing description column")?,
            category_col: category_col.ok_or_eyre("File missing category column")?,
            debit_col: debit_col.ok_or_eyre("File missing debit column")?,
            credit_col: credit_col.ok_or_eyre("File missing credit column")?,
        };

        Ok(Self { reader, columns })
    }

    pub fn read(&'a mut self) -> Result<CsvTransactionIter<'a>> {
        Ok(CsvTransactionIter {
            records: self.reader.records(),
            columns: &self.columns,
        })
    }
}

pub struct CsvTransactionIter<'a> {
    records: StringRecordsIter<'a, File>,
    columns: &'a ColumnMap,
}

impl<'a> CsvTransactionIter<'a> {
    fn next_transaction(&mut self) -> Result<Option<Transaction<'a>>> {
        let Some(record) = self.records.next() else {
            return Ok(None);
        };

        let csv_transaction = self
            .columns
            .unpack_transaction(record.wrap_err("Failed to read CSV row")?)
            .wrap_err("Failed to unpack CsvTransaction from row")?;

        let transaction = csv_transaction
            .into_transaction()
            .wrap_err("Failed to convert CsvTransaction")?;

        Ok(Some(transaction))
    }
}

impl<'a> Iterator for CsvTransactionIter<'a> {
    type Item = Result<Transaction<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_transaction().transpose()
    }
}

struct ColumnMap {
    // transaction_date_col: usize,
    posted_date_col: usize,
    // card_number_col: usize,
    description_col: usize,
    category_col: usize,
    debit_col: usize,
    credit_col: usize,
}

impl ColumnMap {
    fn unpack_transaction(&self, record: StringRecord) -> Result<CsvTransaction> {
        // let transaction_date = record
        //     .get(self.transaction_date_col)
        //     .ok_or_eyre("Failed to get transaction_date column")
        //     .and_then(|s| {
        //         NaiveDate::parse_from_str(s, "%Y-%m-%d")
        //             .wrap_err("Failed to parse transaction_date")
        //     })?;
        let posted_date = record
            .get(self.posted_date_col)
            .ok_or_eyre("Failed to get posted_date column")
            .and_then(|s| {
                NaiveDate::parse_from_str(s, "%Y-%m-%d").wrap_err("Failed to parse posted_date")
            })?;
        // let card_number = record
        //     .get(self.card_number_col)
        //     .ok_or_eyre("Failed to get card_number column")
        //     .and_then(|s| s.parse().wrap_err("Failed to parse card_number"))?;
        let description = record
            .get(self.description_col)
            .ok_or_eyre("Failed to get description column")
            .map(|s| s.to_string())?;
        let category = record
            .get(self.category_col)
            .ok_or_eyre("Failed to get category column")
            .map(|s| s.to_string())?;
        let debit = record
            .get(self.debit_col)
            .ok_or_eyre("Failed to get debit column")
            .and_then(|s| parse_optional_amount(s).wrap_err("Failed to parse debit"))?;
        let credit = record
            .get(self.credit_col)
            .ok_or_eyre("Failed to get credit column")
            .and_then(|s| parse_optional_amount(s).wrap_err("Failed to parse credit"))?;

        Ok(CsvTransaction {
            // transaction_date,
            posted_date,
            // card_number,
            description,
            category,
            debit,
            credit,
        })
    }
}

fn parse_optional_amount(value: &str) -> Result<Option<Decimal>> {
    if value.is_empty() {
        return Ok(None);
    }

    Ok(Some(Decimal::from_str_exact(value)?))
}
