// Compatible with Capital One CSV files

use std::borrow::Cow;
use std::path::Path;

use chrono::NaiveDate;
use color_eyre::Result;
use color_eyre::eyre::{Context, OptionExt, bail};
use csv_async::{AsyncReader, StringRecord};
use futures::TryStreamExt;
use indicatif::ProgressBar;
use rust_decimal::Decimal;
use tokio::fs::File;
use tokio::io::BufReader;

use crate::importer::{Transaction, TransactionImporter, TransactionReader, TransactionType};

struct CsvTransaction {
    posted_date: NaiveDate,
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
    reader: AsyncReader<BufReader<File>>,
    columns: ColumnMap,
}

impl CsvReader {
    pub async fn open(path: &Path) -> Result<Self> {
        let mut reader = AsyncReader::from_reader(BufReader::new(
            File::open(path).await.wrap_err("Failed to open file")?,
        ));

        let mut transaction_date_col = None;
        let mut posted_date_col = None;
        let mut card_number_col = None;
        let mut description_col = None;
        let mut category_col = None;
        let mut debit_col = None;
        let mut credit_col = None;
        let headers = reader.headers().await.wrap_err("Failed to read headers")?;
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
}

impl TransactionReader for CsvReader {
    async fn load(
        self,
        mut importer: TransactionImporter<'_>,
        progress: &ProgressBar,
    ) -> Result<()> {
        let mut records = self.reader.into_records();

        let mut i = 0usize;
        while let Some(row) = records.try_next().await.wrap_err("Failed to read row")? {
            let transaction = self
                .columns
                .unpack_transaction(row)
                .wrap_err("Failed to unpack CsvTransaction from row")
                .and_then(|t| t.into_transaction())
                .wrap_err("Failed to convert CsvTransaction")?;

            importer.import(transaction).await?;

            if i % 100 == 0 {
                progress.inc(100);
            }

            i += 1;
        }

        Ok(())
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
        let posted_date = record
            .get(self.posted_date_col)
            .ok_or_eyre("Failed to get posted_date column")
            .and_then(|s| {
                NaiveDate::parse_from_str(s, "%Y-%m-%d").wrap_err("Failed to parse posted_date")
            })?;
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
            posted_date,
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
