-- Add migration script here

PRAGMA foreign_keys=OFF;

------------------------------------------------------
--  Users table update
ALTER TABLE app_users
    RENAME TO app_users_old;

CREATE TABLE app_users(
    app_user_uuid VARCHAR(36) PRIMARY KEY
);

INSERT INTO app_users(app_user_uuid)
    SELECT user_uuid FROM app_users_old;

SAVEPOINT app_users_table_update;
-- ROLLBACK TO app_users_table_update;

------------------------------------------------------
-- Facebook table update
ALTER TABLE facebook_users
    RENAME TO facebook_users_old;

CREATE TABLE facebook_users(
    facebook_uuid VARCHAR(64) PRIMARY KEY,
    app_user_uuid VARCHAR(36) NOT NULL UNIQUE,

    CONSTRAINT app_user_ref 
        FOREIGN KEY (app_user_uuid) 
        REFERENCES app_users(app_user_uuid)
);

INSERT INTO facebook_users(facebook_uuid, app_user_uuid)
    SELECT facebook_uid, user_uuid 
        FROM facebook_users_old 
        INNER JOIN app_users_old ON facebook_users_old.app_user_id = app_users_old.id;

SAVEPOINT facebool_users_table_update;

------------------------------------------------------
-- Facebook table update
ALTER TABLE google_users
    RENAME TO google_users_old;

CREATE TABLE google_users(
    google_uuid VARCHAR(64) PRIMARY KEY,
    app_user_uuid VARCHAR(36) NOT NULL UNIQUE,

    CONSTRAINT app_user_ref 
        FOREIGN KEY (app_user_uuid) 
        REFERENCES app_users(app_user_uuid)
);

INSERT INTO google_users(google_uuid, app_user_uuid)
    SELECT google_uid, user_uuid 
        FROM google_users_old 
        INNER JOIN app_users_old ON google_users_old.app_user_id = app_users_old.id;

SAVEPOINT facebool_users_table_update;

------------------------------------------------------
-- Drop old
DROP TABLE app_users_old;
DROP TABLE facebook_users_old;
DROP TABLE google_users_old;

PRAGMA foreign_keys=ON;

------------------------------------------------------
-- Indexes create

CREATE INDEX facebook_search_index ON facebook_users(facebook_uuid);
CREATE INDEX google_search_index ON google_users(google_uuid);