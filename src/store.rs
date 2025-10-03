use anyhow::Result;
use sqlx::{Pool, Sqlite, Row};

use crate::{media::{Album, AlbumInDb, Media}, shared::PROJ_DIRS};

const TRASITION_COMMIT_LIMIT: u8 = 64;

pub struct Store {
    conn: Pool<Sqlite>,
    tx: Option<sqlx::SqliteTransaction<'static>>,
    trasition: u8,
}

impl Store {
    pub async fn new() -> Result<Self> {
        let db_path = format!("sqlite:///{}", PROJ_DIRS.data_dir().join("db.sqlite").to_string_lossy());
        let conn = Pool::<Sqlite>::connect(&db_path).await?;
        let mut store = Self {
            conn,
            tx: None,
            trasition: 0,
        };
        store.init().await?;
        Ok(store)
    }

    async fn init(&mut self) -> Result<()> {
        let query = include_str!("../sql/init.sql");
        sqlx::raw_sql(query).execute(&self.conn).await?;
        Ok(())
    }

    pub async fn commit(&mut self) -> Result<()> {
        if let Some(tx) = self.tx.take() {
            tx.commit().await?;
        }

        Ok(())
    }

    pub async fn add_media(&mut self, media: Media, album: Album) -> Result<()> {
        if self.tx.is_none() {
            self.tx = Some(self.conn.begin().await?);
        }

        todo!();

        self.trasition += 1;
        if self.trasition >= TRASITION_COMMIT_LIMIT {
            self.tx.take().unwrap().commit().await?;
            self.trasition = 0;
        }

        Ok(())
    }

    pub async fn get_album(&mut self, album: Album) -> Result<Vec<AlbumInDb>> {
        let query = "SELECT * FROM album WHERE name = ? AND cover = ?;";
        let albums = sqlx::query_as::<_, AlbumInDb>(query)
            .bind(album.name)
            .bind(album.cover)
            .fetch_all(&self.conn).await?;

        Ok(albums)
    }

    pub async fn insert_album(&mut self, album: Album) -> Result<i32> {
        let query = "
INSERT INTO album (name, year, track, cover)
VALUES (?, ?, ?, ?)
RETURNING id;
        ";

        let id: i32 = sqlx::query(query)
            .bind(album.name)
            .bind(album.year)
            .bind(album.track)
            .bind(album.cover)
            .fetch_one(&self.conn)
            .await?
            .try_get("id")?;

        Ok(id)
    }
}
