-- Your SQL goes here
CREATE TABLE IF NOT EXISTS indexer.vote_record (
    "address" VARCHAR NOT NULL,
    "args" CHAR(40) NOT NULL,
    "height" BIGINT NOT NULL,
    "epochRaw" BIGINT NOT NULL,
    "epochNum" BIGINT NOT NULL,
    "epochIndex" BIGINT NOT NULL,
    "epochLen" BIGINT NOT NULL,
    "voteIndex" INT[] NOT NULL,
    "timestamp" VARCHAR NOT NULL,
    "txHash" VARCHAR NOT NULL,
    "txIndex" INT NOT NULL,
    "outIndex" INT NOT NULL,
    PRIMARY KEY ("txHash", "outIndex")
);

