This is the code for a service to host accounts on the fediverse that
post messages from RSS feeds.


## Using the service

I run an instance of this code at
[feedsin.space](feedsin.space). Before you can setup a feed, you need
to send a fediverse message to @admin@feedsin.space with the word
"help". This is an automated account, and it will respond with a link
that you can use to authenticate with the service. Once you're logged
in, you can create a feed. The feed requires a username and the feed
URL. Once you've specified those, there will be an account at
@username@feedsin.space. feedsin.space will scan the RSS feed every
now and then, and if there's a new entry, it'll be posted as a status
by the account.

## Running the code

The service is written is Rust, so you need to set that up using your
favorite method. The code requires some features that are currently
only available in rust nightly, so:

```
rustup override set nightly
```

### Environment variables

There are several environment variables which you must set to run the service:


* `DATABASE_URL` specifies the database, and will look something like
  `postgres://username:password@host/dbname`
* `DOMAIN_NAME` is the host name of the instance, for example:
  `feedsin.space`. **NOTE**: Once you set this, there's no system in
  place to change it!
* `DISABLE_SIGNATURE_CHECKS` you probably shouldn't set this, but if
  you're having problem validating message signatures from other
  fediverse instances, you can set this to true to skip those. It's
  probably not a good idea though.


### Database

The database backend works with postgres, and is built with
[sqlx](https://github.com/launchbadge/sqlx) which has a command line
tool you might want to install to run migrations/etc:

```
cargo install sqlx-cli --no-default-features --features rustls,postgres
```

If you have sqlx installed you can run something like:

```
sqlx database setup
```

In the future if you need to run a database migration, you could use a command like:

```
sqlx migrate run
```


Once you've done that, you can run the web server with:

```
cargo run --bin server
```


And the service for background jobs runs with:
```
cargo run --bin worker
```

There's a `docker-compose.sample.yml` file in the code that you can
use to get an idea of how to run the service with Docker if that's
something you want to do.
