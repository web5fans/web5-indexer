# web5-indexer

It's a indexer for did web5 protocol. The indexer would follow every did transaction on [CKB](https://www.nervos.org/ckbpage) blockchain. And indexing every did record through path `<server url>\{:did}`.

## Quick start

At now, the indexer default would follow CKB testnet.

First you need a postgres db:

``` shell
docker run -d --name postgres -e POSTGRES_USER=pg -e POSTGRES_PASSWORD=password -p 5433:5432 postgres:14.4-alpine

# then you need to create db in postgres container
docker exec -it postgres bash

# in container, crate db
CREATE DATABASE indexer;
```

Then you need use [diesel](https://diesel.rs/) to initiate you database:

``` shell
# if you don't have diesel, you need to install it
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/diesel-rs/diesel/releases/latest/download/diesel_cli-installer.sh | sh


diesel migration run
```

``` shell
cd web5-indexer

cargo build -r

./target/release/web5-indexer
```

## To the mainnet

We need to wait the did contract deployed on mainnet. But if you are hard coder, you can change:

```
CKB_NODE=https://ckb.dev
CKB_NETWORK=ckb
START_HEIGHT=<contract height>
CODE_HASH=<type script hash>
DID_CODE_HASH=<type script hash>
DAO_CODE_HASH=<type script hash>
```

Change above envs, the indexer could run on CKB mainnet
