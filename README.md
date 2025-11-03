# web5-indexer

It's a indexer for did:web5. The indexer would follow every did transaction on [CKB](https://www.nervos.org/ckbpage) blockchain. And indexing every did record through path `<server url>\{:did}`.

## Quick start

At now, the indexer default would follow CKB testnet.

First you need a postgres db:

``` shell
docker run -d --name postgres -e POSTGRES_USER=pg -e POSTGRES_PASSWORD=password -p 5433:5432 postgres:14.4-alpine
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
```

Change above envs, the indexer could run on CKB mainnet
