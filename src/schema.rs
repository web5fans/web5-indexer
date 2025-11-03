// @generated automatically by Diesel CLI.

pub mod indexer {
    diesel::table! {
        indexer.did_delete_record (did) {
            did -> Varchar,
            ckbAddress -> Varchar,
            handle -> Varchar,
            txHash -> Varchar,
            inIndex -> Int4,
            document -> Varchar,
            height -> Int8,
            deletedAt -> Varchar,
        }
    }

    diesel::table! {
        indexer.did_record (did) {
            did -> Varchar,
            ckbAddress -> Varchar,
            handle -> Varchar,
            txHash -> Varchar,
            outIndex -> Int4,
            document -> Varchar,
            height -> Int8,
            createdAt -> Varchar,
            valid -> Bool,
        }
    }

    diesel::allow_tables_to_appear_in_same_query!(
        did_delete_record,
        did_record,
    );
}
