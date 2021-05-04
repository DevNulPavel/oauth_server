use tracing::{
    debug,
    info,
    error,
    info_span,
    event,
    Level,
    instrument
};
use tracing_futures::{
    Instrument
};
use tap::{
    TapFallible
};
use sqlx::{
    // prelude::{
    //     *
    // },
    sqlite::{
        SqlitePool
    }
};
use crate::{
    error::{
        AppError
    }
};

////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct UserInfo {
    pub app_user_uuid: String,
    pub facebook_uuid: Option<String>,
    pub google_uuid: Option<String>,
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct Database{
    db: SqlitePool
}
impl Database{
    #[instrument(name = "database_open")]
    pub async fn open() -> Database {
        let db_url = std::env::var("DATABASE_URL")
            .expect("Missing DATABASE_URL variable");

        // Создаем файлик с пустой базой данных если его нету
        {
            const PREFIX: &str = "sqlite://";
            assert!(db_url.starts_with(PREFIX), "DATABASE_URL must stats with {}", PREFIX);

            let file_path = std::path::Path::new(db_url.trim_start_matches(PREFIX));
            if !file_path.exists() {
                if let Some(dir) = file_path.parent(){
                    std::fs::create_dir_all(dir)
                        .expect("Database directory create failed");
                }
                std::fs::File::create(file_path)
                    .expect("Database file create failed");
            }
        }

        let sqlite_conn = SqlitePool::connect(&db_url)
            .in_current_span()
            .await
            .expect("Database connection failed");

        event!(Level::DEBUG, 
               db_type = %"sqlite", // Будет отформатировано как Display
               "Database open success");

        // Включаем все миграции базы данных сразу в наш бинарник, выполняем при старте
        sqlx::migrate!("./migrations")
            .run(&sqlite_conn)
            .in_current_span()
            .await
            .expect("database migration failed");

        info!(
            migrations_folder = ?"./migrations",
            "Migrated"
        );

        Database{
            db: sqlite_conn
        }
    }

    /// Пытаемся найти нового пользователя для FB ID 
    #[instrument(err, skip(self))]
    pub async fn try_to_find_user_uuid_with_fb_id(&self, facebook_uuid: &str) -> Result<Option<String>, AppError>{
        struct Res{
            app_user_uuid: String
        }
        let res = sqlx::query_as!(Res,
                r#"   
                    SELECT facebook_users.app_user_uuid
                    FROM facebook_users 
                    WHERE facebook_users.facebook_uuid = ?
                "#, facebook_uuid)
            .fetch_optional(&self.db)
            .in_current_span()
            .await
            .map_err(AppError::from)
            .tap_err(|e|{ 
                error!(%e); 
            })?
            .map(|val|{
                val.app_user_uuid
            });

        debug!(facebook_uuid = %facebook_uuid, app_user_uuid = ?res, "Success");

        Ok(res)
    }

    #[instrument(err, skip(self))]
    pub async fn insert_facebook_user_with_uuid(&self, app_user_uuid: &str, facebook_uuid: &str) -> Result<(), AppError>{
        // Стартуем транзакцию, если будет ошибка, то вызовется rollback автоматически в drop
        // если все хорошо, то руками вызываем commit
        let transaction = self
            .db
            .begin()
            .instrument(info_span!("transaction_begin"))
            .await
            .tap_err(|err|{
                error!(%err);
            })?;

        // Если таблица иммет поле INTEGER PRIMARY KEY тогда last_insert_rowid - это алиас
        sqlx::query!(
                r#"
                    INSERT INTO app_users(app_user_uuid)
                        VALUES (?);
                "#, app_user_uuid)
            .execute(&self.db)
            .instrument(info_span!("app_user_insert"))
            .await
            .tap_err(|err|{
                error!(%err);
            })?;

        sqlx::query!(
                r#"
                    INSERT INTO facebook_users(facebook_uuid, app_user_uuid)
                    VALUES (?, ?);
                "#, facebook_uuid, app_user_uuid)
            .execute(&self.db)
            .instrument(info_span!("facebook_user_insert"))
            .await
            .tap_err(|err|{
                error!(%err);
            })?;

        transaction
            .commit()
            .instrument(info_span!("transaction_commit"))
            .await
            .tap_err(|err|{
                error!(%err);
            })?;

        info!("Success");

        Ok(())
    }

    #[instrument(err, skip(self))]
    pub async fn append_facebook_user_for_uuid(&self, app_user_uuid: &str, facebook_uuid: &str) -> Result<(), AppError>{
        // Стартуем транзакцию, если будет ошибка, то вызовется rollback автоматически в drop
        // если все хорошо, то руками вызываем commit
        let transaction = self
            .db
            .begin()
            .instrument(info_span!("transaction_begin"))
            .await
            .tap_err(|err|{
                error!(%err);
             })?;

        // Если таблица иммет поле INTEGER PRIMARY KEY тогда last_insert_rowid - это алиас
        sqlx::query!(
                r#"
                    INSERT INTO facebook_users(facebook_uuid, app_user_uuid)
                    VALUES (?, ?);
                "#, facebook_uuid, app_user_uuid)
            .execute(&self.db)
            .instrument(info_span!("facebook_user_insert"))
            .await
            .tap_err(|err|{
                error!(%err);
            })?;

        transaction
            .commit()
            .instrument(info_span!("transaction_commit"))
            .await
            .tap_err(|err|{
                error!(%err);
             })?;

        info!("Success");

        Ok(())
    }

    /// Пытаемся найти нового пользователя для Google ID 
    #[instrument(err, skip(self))]
    pub async fn try_to_find_user_uuid_with_google_id(&self, google_uuid: &str) -> Result<Option<String>, AppError>{
        struct Res{
            app_user_uuid: String
        }
        let res = sqlx::query_as!(Res,
                        r#"   
                            SELECT app_user_uuid
                            FROM google_users 
                            WHERE google_users.google_uuid = ?
                        "#, google_uuid)
            .fetch_optional(&self.db)
            .in_current_span()
            .await
            .map_err(AppError::from)
            .tap_err(|err|{
                error!(%err);
            })?
            .map(|val|{
                val.app_user_uuid
            });

        info!(app_user_uuid = ?res, "Found");

        Ok(res)
    }

    #[instrument(err, skip(self))]
    pub async fn insert_google_user_with_uuid(&self, app_user_uuid: &str, google_uuid: &str) -> Result<(), AppError>{
        // Стартуем транзакцию, если будет ошибка, то вызовется rollback автоматически в drop
        // если все хорошо, то руками вызываем commit
        let transaction = self
            .db
            .begin()
            .instrument(info_span!("transaction_begin"))
            .await
            .tap_err(|err|{
                error!(%err);
            })?;

        sqlx::query!(
                r#"
                    INSERT INTO app_users(app_user_uuid)
                    VALUES (?);
                "#, app_user_uuid)
            .execute(&self.db)
            .instrument(info_span!("app_user_insert"))
            .await
            .tap_err(|err|{
                error!(%err);
            })?;

        sqlx::query!(
                r#"
                    INSERT INTO google_users(google_uuid, app_user_uuid)
                    VALUES (?, ?);
                "#, google_uuid, app_user_uuid)
            .execute(&self.db)
            .instrument(info_span!("google_user_insert"))
            .await
            .tap_err(|err|{
                error!(%err);
            })?;

        transaction
            .commit()
            .instrument(info_span!("transaction_commit"))
            .await
            .tap_err(|err|{
                error!(%err);
            })?;

        info!("Success");

        Ok(())
    }

    #[instrument(err, skip(self))]
    pub async fn append_google_user_for_uuid(&self, app_user_uuid: &str, google_uuid: &str) -> Result<(), AppError>{
        // Стартуем транзакцию, если будет ошибка, то вызовется rollback автоматически в drop
        // если все хорошо, то руками вызываем commit
        let transaction = self
            .db
            .begin()
            .instrument(info_span!("transaction_begin"))
            .await
            .tap_err(|err|{
                error!(%err);
            })?;

        sqlx::query!(
                r#"
                    INSERT INTO google_users(google_uuid, app_user_uuid)
                    VALUES (?, ?);
                "#, google_uuid, app_user_uuid)
            .execute(&self.db)
            .instrument(info_span!("google_user_insert"))
            .await
            .tap_err(|err|{
                error!(%err);
            })?;

        transaction
            .commit()
            .instrument(info_span!("transaction_commit"))
            .await
            .tap_err(|err|{
                error!(%err);
             })?;

        info!("Success");

        Ok(())
    }

    /// Пытаемся найти нового пользователя для FB ID 
    #[instrument(err, skip(self))]
    pub async fn try_find_full_user_info_for_uuid(&self, app_user_uuid: &str) -> Result<Option<UserInfo>, AppError>{
        // Специальным образом описываем, что поле действительно может быть нулевым с 
        // помощью вопросика в переименовании - as 'facebook_uid?'
        // так же можно описать, что поле точно ненулевое, чтобы не использовать Option
        // as 'facebook_uid!'
        // https://docs.rs/sqlx/0.4.0-beta.1/sqlx/macro.query.html#overrides-cheatsheet
        let res: Option<UserInfo> = sqlx::query_as!(UserInfo,
                r#"   
                    SELECT 
                        app_users.app_user_uuid AS "app_user_uuid!", 
                        facebook_users.facebook_uuid AS "facebook_uuid?",
                        google_users.google_uuid AS "google_uuid?"
                    FROM app_users
                    LEFT JOIN facebook_users
                        ON facebook_users.app_user_uuid = app_users.app_user_uuid
                    LEFT JOIN google_users
                        ON google_users.app_user_uuid = app_users.app_user_uuid
                    WHERE app_users.app_user_uuid = ?
                "#, app_user_uuid)
            .fetch_optional(&self.db)
            .instrument(info_span!("users_select"))
            .await
            .map_err(AppError::from)
            .tap_err(|err|{
                error!(%err);
            })?;

        Ok(res)
    }

    /// Пытаемся найти нового пользователя для FB ID 
    #[instrument(err, skip(self))]
    pub async fn does_user_uuid_exist(&self, app_user_uuid: &str) -> Result<bool, AppError>{
        // TODO: Более оптимальный вариант
        let res = sqlx::query!(r#"   
                                    SELECT app_user_uuid 
                                    FROM app_users 
                                    WHERE app_users.app_user_uuid = ?
                                "#, app_user_uuid)
            .fetch_optional(&self.db)
            .await
            .map_err(AppError::from)
            .tap_err(|err|{
                error!("User search failed: {}", err);
            })?;
        
        Ok(res.is_some())
    }
}