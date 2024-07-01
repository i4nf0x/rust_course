use chat::ChatMessage;
use chat::ChatMessageContent;
use sqlx::Connection;
use sqlx::SqliteConnection;
use chat::EmptyResult;
use anyhow::{anyhow, Result,Context};
use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};

pub struct ServerDatabase {
    pub db: SqliteConnection,
    password_salt: Option<SaltString>
}


impl ServerDatabase {
    pub async fn new(file: &str) -> Result<ServerDatabase> {
        let connection = 
        SqliteConnection::connect(format!("sqlite:{file}?mode=rwc").as_str())
            .await
            .with_context(|| format!("Could not open database."))?;

        let mut db = ServerDatabase {
            db: connection,
            password_salt: None
        };

        db.init().await?;
        
        Ok(db)
    }

    async fn init(&mut self) -> EmptyResult {
        let (ver, ): (i32, ) = sqlx::query_as("PRAGMA user_version")
            .fetch_one(&mut self.db).await?;
        
        if ver==0 {
            log::warn!("Creating a new database.");

            let mut trans = self.db.begin().await?;
            

            sqlx::query(
                "
                CREATE TABLE IF NOT EXISTS users (
                    username TEXT PRIMARY KEY,
                    password TEXT
                )
                "
            ).execute(&mut *trans).await
            .context("Failed to create table: users")?;

            sqlx::query(
                "
                CREATE TABLE IF NOT EXISTS messages (
                    messages_id INTEGER PRIMARY KEY,
                    sender TEXT,
                    content_type INTEGER,
                    text TEXT,
                    filename TEXT,
                    content BLOB,
                    FOREIGN KEY(sender) REFERENCES users(username)
                )
                "
            ).execute(&mut *trans).await
            .context("Failed to create table: messages")?;

            sqlx::query(
                "
                CREATE TABLE IF NOT EXISTS config (
                key TEXT NOT NULL PRIMARY KEY, 
                value TEXT NOT NULL
                )
                ").execute(&mut *trans).await
                .context("Failed to create table: config")?;

            
            let salt = SaltString::generate(&mut OsRng);
            log::info!("Generated password salt: {salt}");

            sqlx::query(
                "
                INSERT INTO config (key, value) VALUES ('password_salt', $1)
                ",
                
            ).bind(salt.as_str()).execute(&mut *trans).await?;
            
            sqlx::query("PRAGMA user_version=1").execute(&mut *trans).await?;
            trans.commit().await?;
        }

        let (password_salt, ): (String, ) = sqlx::query_as("SELECT value FROM config WHERE key='password_salt'")
            .fetch_one(&mut self.db).await?;

        self.password_salt = Some(SaltString::from_b64(&password_salt)
            .map_err(|e| anyhow!(e))?);

        log::info!("Loaded password salt: {password_salt}");

        Ok(())
    }

    pub async fn check_auth(&mut self, username: &str, password: &str) -> Result<bool> {
        let argon = Argon2::default();
        let hash: Option<String> = sqlx::query_scalar(
            "
            SELECT password FROM users WHERE username=$1
            "
        ).bind(username)
        .fetch_optional(&mut self.db).await?;

        
        let hash = hash.context("No such user in the database.")?;
        let hash = PasswordHash::new(&hash).map_err(|e| anyhow!(e))?;

        if let Ok(_) = argon.verify_password(password.as_bytes(), &hash) {
            return Ok(true);
        } else {
            return Ok(false);
        }
    }

    pub async fn register_user(&mut self, username: &str, password: &str) -> EmptyResult {
        let argon = Argon2::default();
        let password_salt = self.password_salt.as_ref()
            .context("Password salt must be defined")?;

        let hash = argon.hash_password(password.as_bytes(), password_salt.as_salt());
        if let Ok(hash) = hash {
            let hash = hash.serialize();
            let hash = hash.as_str();
            log::debug!("Hashed password {hash}");
            sqlx::query(
                "
                INSERT INTO users(username, password) VALUES ($1,$2)
                "
            ).bind(username).bind(hash)
            .execute(&mut self.db).await?;
            Ok(())
        } else {
            Err(anyhow!("Failed to hash password."))
        }

        
    }

    pub async fn store_message(&mut self, message: &ChatMessage) -> EmptyResult {

        match &message.content {
            ChatMessageContent::Text(txt) => {
                sqlx::query(
                    "
                    INSERT INTO messages (sender, text, content_type)
                    VALUES ($1, $2, 1)
                    "
                )
                .bind(&message.sender).bind(txt)
                .execute(&mut self.db).await?;
                
            },
            ChatMessageContent::Image(data) => {
                sqlx::query(
                    "
                    INSERT INTO messages (sender, content, content_type)
                    VALUES ($1, $2, 2)
                    ",

                )
                .bind(&message.sender).bind(data)
                .execute(&mut self.db).await?;

            },
            ChatMessageContent::File(filename, data) => {
                sqlx::query(
                    "
                    INSERT INTO messages (sender, filename, content, content_type)
                    VALUES ($1, $2, $3, 3)
                    ",
                ).bind(&message.sender).bind(filename).bind(data)
                .execute(&mut self.db).await?;
            },
        }
        Ok(())
    }

}
