use diesel::{Insertable, Queryable};
use rocket_sync_db_pools::diesel;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::{accounts, upload_cells, uploads};

#[derive(Debug, Clone, Deserialize, Serialize, Insertable, Associations)]
#[belongs_to(Upload)]
#[table_name = "upload_cells"]
pub struct UploadCellInsert {
    pub upload_id: i32,
    pub header: bool,
    pub row_num: i64,
    pub column_num: i64,
    pub contents: String,
}

#[derive(Identifiable, Debug, Clone, Deserialize, Serialize, Queryable, Associations)]
#[belongs_to(Upload)]
#[table_name = "upload_cells"]
pub struct UploadCell {
    pub id: i32,
    pub upload_id: i32,
    pub header: bool,
    pub row_num: i64,
    pub column_num: i64,
    pub contents: String,
}

#[derive(Debug, Clone, Insertable)]
#[table_name = "uploads"]
pub struct UploadInsert {
    pub web_id: Uuid,
}

#[derive(Identifiable, Debug, Clone, Deserialize, Serialize, Queryable)]
#[table_name = "uploads"]
pub struct Upload {
    pub id: i32,
    pub web_id: Uuid,
    pub row_count: i64,
    pub column_count: i64,
}

#[derive(Debug, Clone, Insertable)]
#[table_name = "accounts"]
pub struct AccountInsert {
    pub account_name: String,
}

#[derive(Identifiable, Debug, Clone, Deserialize, Serialize, Queryable)]
#[table_name = "accounts"]
pub struct Account {
    pub id: i32,
    pub account_name: String,
}
