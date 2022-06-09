CREATE TABLE upload_cells (
  id SERIAL PRIMARY KEY,
  upload_id INT NOT NULL,
  header BOOLEAN NOT NULL,
  row_num BIGINT NOT NULL,
  column_num BIGINT NOT NULL,
  contents TEXT NOT NULL,
  CONSTRAINT fk_upload FOREIGN KEY(upload_id) REFERENCES uploads(id)
);
