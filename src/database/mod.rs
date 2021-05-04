use std::{
    env::{
        self
    }
};
use tracing::{
    debug,
    error,
    event,
    Level,
    instrument
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
    pub user_uuid: String,
    pub facebook_uid: Option<String>,
    pub google_uid: Option<String>,
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
            .await
            .expect("Database connection failed");

        event!(Level::DEBUG, 
               db_type = %"sqlite", // Будет отформатировано как Display
               "Database open success");

        // Включаем все миграции базы данных сразу в наш бинарник, выполняем при старте
        sqlx::migrate!("./migrations")
            .run(&sqlite_conn)
            .await
            .expect("database migration failed");

        debug!(migration_file = ?"./migrations", // Будет отформатировано как debug
               "Database migration finished");

        Database{
            db: sqlite_conn
        }
    }

    /// Пытаемся найти нового пользователя для FB ID 
    #[instrument]
    pub async fn try_to_find_user_uuid_with_fb_id(&self, id: &str) -> Result<Option<String>, AppError>{
        struct Res{
            app_user_uuid: String
        }
        let res = sqlx::query_as!(Res,
                r#"   
                    SELECT facebook_users.app_user_uuid
                    FROM facebook_users 
                    WHERE facebook_users.facebook_uuid = ?
                "#, id)
            .fetch_optional(&self.db)
            .await
            .map_err(AppError::from)
            .tap_err(|e|{ 
                error!("Find failed: {}", e); 
            })?
            .map(|val|{
                val.app_user_uuid
            });

        debug!("User for id = {} found: uuid = {:?}", id, res);

        Ok(res)
    }

    #[instrument]
    pub async fn insert_facebook_user_with_uuid(&self, uuid: &str, fb_uid: &str) -> Result<(), AppError>{
        // Стартуем транзакцию, если будет ошибка, то вызовется rollback автоматически в drop
        // если все хорошо, то руками вызываем commit
        let transaction = self
            .db
            .begin()
            .await
            .tap_err(|err|{
                error!("Transaction start failed: {}", err);
            })?;

        // Если таблица иммет поле INTEGER PRIMARY KEY тогда last_insert_rowid - это алиас
        sqlx::query!(
                r#"
                    INSERT INTO app_users(app_user_uuid)
                        VALUES (?);
                "#, uuid)
            .execute(&self.db)
            .await
            .tap_err(|err|{
                error!("Insert native user failed: {}", err);
            })?;

        sqlx::query!(
                r#"
                    INSERT INTO facebook_users(facebook_uuid, app_user_uuid)
                    VALUES (?, ?);
                "#, fb_uid, uuid)
            .execute(&self.db)
            .await
            .tap_err(|err|{
                error!("Insert facebook user failed: {}", err);
            })?;

        transaction
            .commit()
            .await
            .tap_err(|err|{
                error!("Transaction commit failed: {}", err);
            })?;

        debug!("New facebook user included");

        Ok(())
    }

    #[instrument]
    pub async fn append_facebook_user_for_uuid(&self, uuid: &str, fb_uid: &str) -> Result<(), AppError>{
        // Стартуем транзакцию, если будет ошибка, то вызовется rollback автоматически в drop
        // если все хорошо, то руками вызываем commit
        let transaction = self.db.begin().await?;

        // TODO: ???
        // Если таблица иммет поле INTEGER PRIMARY KEY тогда last_insert_rowid - это алиас
        // Но вроде бы наиболее надежный способ - это сделать подзапрос
        // TODO: TRANSACTION
        let new_row_id = sqlx::query!(r#"
                                            INSERT INTO facebook_users(facebook_uid, app_user_id)
                                            VALUES (?, (SELECT id FROM app_users WHERE user_uuid = ?))
                                            RETURNING ;
                                        "#, fb_uid, uuid)
            .execute(&self.db)
            .await
            .tap_err(|err|{
                error!("Append facebook user failed: {}", err);
            })?
            .last_insert_rowid();

        transaction.commit().await?;

        debug!("New facebook user included: row_id = {}", new_row_id);

        Ok(())
    }

    /// Пытаемся найти нового пользователя для FB ID 
    #[instrument]
    pub async fn try_to_find_user_uuid_with_google_id(&self, id: &str) -> Result<Option<String>, AppError>{
        struct Res{
            user_uuid: String
        }
        let res = sqlx::query_as!(Res,
                        r#"   
                            SELECT app_users.user_uuid
                            FROM app_users 
                            INNER JOIN google_users 
                                    ON google_users.app_user_id = app_users.id
                            WHERE google_users.google_uid = ?
                        "#, id)
            .fetch_optional(&self.db)
            .await
            .map_err(AppError::from)
            .tap_err(|err|{
                error!("User with google id is not found: {}", err);
            })?
            .map(|val|{
                val.user_uuid
            });


        Ok(res)
    }

    #[instrument]
    pub async fn insert_google_user_with_uuid(&self, uuid: &str, google_uid: &str) -> Result<(), AppError>{
        // Стартуем транзакцию, если будет ошибка, то вызовется rollback автоматически в drop
        // если все хорошо, то руками вызываем commit
        let transaction = self.db.begin().await?;

        // TODO: ???
        // Если таблица иммет поле INTEGER PRIMARY KEY тогда last_insert_rowid - это алиас
        // Но вроде бы наиболее надежный способ - это сделать подзапрос
        // TODO: TRANSACTION
        let new_row_id = sqlx::query!(r#"
                                        INSERT INTO app_users(user_uuid)
                                        VALUES (?);
                                        INSERT INTO google_users(google_uid, app_user_id)
                                        VALUES (?, (SELECT id FROM app_users WHERE user_uuid = ?));
                                        "#, uuid, google_uid, uuid)
            .execute(&self.db)
            .await
            .tap_err(|err|{
                error!("User insert failed: {}", err);
            })?
            .last_insert_rowid();

        transaction.commit().await?;

        debug!("New google user included: row_id = {}", new_row_id);

        Ok(())
    }

    #[instrument]
    pub async fn append_google_user_for_uuid(&self, uuid: &str, google_uid: &str) -> Result<(), AppError>{
        // Стартуем транзакцию, если будет ошибка, то вызовется rollback автоматически в drop
        // если все хорошо, то руками вызываем commit
        let transaction = self.db.begin().await?;

        // TODO: ???
        // Если таблица иммет поле INTEGER PRIMARY KEY тогда last_insert_rowid - это алиас
        // Но вроде бы наиболее надежный способ - это сделать подзапрос
        let new_row_id = sqlx::query!(r#"
                                        INSERT INTO google_users(google_uid, app_user_id)
                                        VALUES (?, (SELECT id FROM app_users WHERE user_uuid = ?));
                                        "#, google_uid, uuid)
            .execute(&self.db)
            .await
            .tap_err(|err|{
                error!("User append failed: {}", err);
            })?
            .last_insert_rowid();

        transaction.commit().await?;

        debug!("New google user included: row_id = {}", new_row_id);

        Ok(())
    }

    /// Пытаемся найти нового пользователя для FB ID 
    #[instrument]
    pub async fn try_find_full_user_info_for_uuid(&self, uuid: &str) -> Result<Option<UserInfo>, AppError>{
        // Специальным образом описываем, что поле действительно может быть нулевым с 
        // помощью вопросика в переименовании - as 'facebook_uid?'
        // так же можно описать, что поле точно ненулевое, чтобы не использовать Option
        // as 'facebook_uid!'
        // https://docs.rs/sqlx/0.4.0-beta.1/sqlx/macro.query.html#overrides-cheatsheet
        sqlx::query_as!(UserInfo,
                        r#"   
                            SELECT 
                                app_users.user_uuid, 
                                facebook_users.facebook_uid as 'facebook_uid?',
                                google_users.google_uid as 'google_uid?'
                            FROM app_users
                            LEFT JOIN facebook_users
                                ON facebook_users.app_user_id = app_users.id
                            LEFT JOIN google_users
                                ON google_users.app_user_id = app_users.id                            
                            WHERE app_users.user_uuid = ?
                        "#, uuid)
            .fetch_optional(&self.db)
            .await
            .map_err(AppError::from)
            .tap_err(|err|{
                error!("Full user search failed: {}", err);
            })
    }

    /// Пытаемся найти нового пользователя для FB ID 
    #[instrument]
    pub async fn does_user_uuid_exist(&self, uuid: &str) -> Result<bool, AppError>{
        // TODO: Более оптимальный вариант
        let res = sqlx::query!(r#"   
                                    SELECT user_uuid 
                                    FROM app_users 
                                    WHERE app_users.user_uuid = ?
                                "#, uuid)
            .fetch_optional(&self.db)
            .await
            .map_err(AppError::from)
            .tap_err(|err|{
                error!("User search failed: {}", err);
            })?;
        
        Ok(res.is_some())
    }
}