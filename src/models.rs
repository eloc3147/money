use diesel::{Insertable, Queryable};
use rocket_sync_db_pools::diesel;
use serde::{Deserialize, Serialize};

use crate::schema::{upload_cells, uploads};

#[derive(Debug, Clone, Deserialize, Serialize, Queryable, Insertable, Associations)]
#[belongs_to(Upload)]
#[table_name = "upload_cells"]
pub struct UploadCell {
    pub id: Option<i32>,
    pub upload_id: i32,
    pub header: bool,
    pub row_num: i64,
    pub column_num: i64,
    pub contents: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Queryable, Insertable)]
#[table_name = "uploads"]
pub struct Upload {
    pub id: i32,
}
