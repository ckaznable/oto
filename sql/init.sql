-- Create Album table
CREATE TABLE IF NOT EXISTS album (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    year INTEGER,
    track INTEGER,
    cover TEXT
);

-- Create Media table
CREATE TABLE IF NOT EXISTS media (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    artist TEXT,
    album_id INTEGER,
    track INTEGER,
    FOREIGN KEY (album_id) REFERENCES album(id)
);

-- Create indexes for better performance
CREATE INDEX IF NOT EXISTS idx_media_album_id ON media(album_id);
CREATE INDEX IF NOT EXISTS idx_media_artist ON media(artist);
CREATE INDEX IF NOT EXISTS idx_media_name ON media(name);
CREATE INDEX IF NOT EXISTS idx_album_name ON album(name);

-- Insert default album data when table is empty
INSERT INTO album (name, year, track, cover)
SELECT 'Unknown Album', NULL, NULL, NULL
WHERE NOT EXISTS (SELECT 1 FROM album LIMIT 1);

-- This trigger will insert a default album if all albums are deleted
CREATE TRIGGER IF NOT EXISTS ensure_default_album
AFTER DELETE ON album
WHEN (SELECT COUNT(*) FROM album) = 0
BEGIN
    INSERT INTO album (name, year, track, cover) 
    VALUES ('Unknown Album', NULL, NULL, NULL);
END;

CREATE VIEW IF NOT EXISTS media_with_album AS
SELECT 
    m.id,
    m.file,
    m.name,
    m.artist,
    m.track,
    a.name AS album_name,
    a.year AS album_year,
    a.cover AS album_cover
FROM media m
LEFT JOIN album a ON m.album_id = a.id;
