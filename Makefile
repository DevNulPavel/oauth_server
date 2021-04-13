# Описание командной утилиты по работе с базой данных:
# https://github.com/launchbadge/sqlx/tree/master/sqlx-cli

.SILENT:

DATABASE_INITIALIZE:
	export DATABASE_URL=sqlite://db/oauth_db.sqlite && \
	sqlx database create

DATABASE_MIGRATION_CREATE:
	export DATABASE_URL=sqlite://db/oauth_db.sqlite && \
	sqlx migrate add initialize

DATABASE_MIGRATION_PERFORM:
	export DATABASE_URL=sqlite://db/oauth_db.sqlite && \
	sqlx migrate run

########################################################################################

ENCRYPT_TEST_ENV:
	gpg -a -r 0x0BD10E4E6E578FB6 -o credentials/test_environment.env.asc -e credentials/test_environment.env
	gpg -a -r 0x0BD10E4E6E578FB6 -o credentials/test_environment_docker.env.asc -e credentials/test_environment_docker.env

DECRYPT_TEST_ENV:
	rm -rf test_environment.env
	rm -rf test_environment_docker.env
	gpg -a -r 0x0BD10E4E6E578FB6 -o credentials/test_environment.env -d credentials/test_environment.env.asc
	gpg -a -r 0x0BD10E4E6E578FB6 -o credentials/test_environment_docker.env -d credentials/test_environment_docker.env.asc

ENCRYPT_GOOGLE_CREDENTIALS:
	gpg -a -r 0x0BD10E4E6E578FB6 -o credentials/test_google_auth_credentials.json.asc -e credentials/test_google_auth_credentials.json

DECRYPT_GOOGLE_CREDENTIALS:
	rm -rf test_google_auth_credentials.json
	gpg -a -r 0x0BD10E4E6E578FB6 -o credentials/test_google_auth_credentials.json -d credentials/test_google_auth_credentials.json.asc

########################################################################################

RUN_SERVER:
	source credentials/test_environment.env && \
	cargo run

########################################################################################

DOCKER_UPDATE_SQLX_OFFLINE_MODE:
	# https://www.lpalmieri.com/posts/2020-11-01-zero-to-production-5-how-to-deploy-a-rust-application/
	cargo sqlx prepare

DOCKER_IMAGE_BUILD:
	docker build -t devnul/oauth_server .

DOCKER_PUSH_IMAGE:
	docker push devnul/oauth_server

DOCKER_PULL_IMAGE: 
	docker pull devnul/oauth_server

# DOCKER_RUN_IMAGE:
# 	# -it - Interactive mode
# 	# -d - Daemon mode
# 	# --network host
# 	docker run \
# 		-d \
# 		--restart unless-stopped \
# 		-p 8888:8888 \
# 		--env-file credentials/test_environment_docker.env \
# 		--volume /Users/devnul/projects/Rust_Examples/oauth_server/credentials:/oauth_server/credentials \
# 		--volume /Users/devnul/projects/Rust_Examples/oauth_server/logs:/oauth_server/logs \
# 		--volume /Users/devnul/projects/Rust_Examples/oauth_server/db:/oauth_server/db \
# 		--name oauth_server \
# 		devnul/oauth_server

DOCKER_RUN_IMAGE:
	docker-compose up