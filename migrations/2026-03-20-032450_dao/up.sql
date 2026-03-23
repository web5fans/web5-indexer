-- Your SQL goes here
CREATE SCHEMA IF NOT EXISTS indexer;

CREATE TABLE IF NOT EXISTS indexer.dao_record (
    "id" BIGSERIAL PRIMARY KEY,
    "ckbAddress" VARCHAR NOT NULL,
    "txHash" VARCHAR NOT NULL,
    "outIndex" INT,
    "inIndex" INT,
    "ckbNumber" BIGINT NOT NULL,
    "depositOrWithdraw" BOOLEAN NOT NULL,
    "height" BIGINT NOT NULL,
    "txIndex" INT NOT NULL,
    "createdAt" character varying NOT NULL,
    "valid" BOOLEAN NOT NULL
);
