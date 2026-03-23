// @generated automatically by Diesel CLI.

pub mod indexer {
    diesel::table! {
        indexer.dao_record (id) {
            id -> Int8,
            ckbAddress -> Varchar,
            txHash -> Varchar,
            outIndex -> Nullable<Int4>,
            inIndex -> Nullable<Int4>,
            ckbNumber -> Int8,
            depositOrWithdraw -> Bool,
            height -> Int8,
            txIndex -> Int4,
            createdAt -> Varchar,
            valid -> Bool,
        }
    }

    diesel::table! {
        indexer.did_delete_record (did) {
            did -> Varchar,
            ckbAddress -> Varchar,
            handle -> Varchar,
            signingKey -> Varchar,
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
            signingKey -> Varchar,
            txHash -> Varchar,
            outIndex -> Int4,
            document -> Varchar,
            height -> Int8,
            createdAt -> Varchar,
            valid -> Bool,
        }
    }

    diesel::table! {
        indexer.pds_list (pds_url) {
            pds_url -> Varchar,
            user_num -> Int8,
        }
    }

    diesel::table! {
        indexer.vote_record (txHash, outIndex) {
            address -> Varchar,
            #[max_length = 40]
            args -> Bpchar,
            height -> Int8,
            epochRaw -> Int8,
            epochNum -> Int8,
            epochIndex -> Int8,
            epochLen -> Int8,
            voteIndex -> Array<Nullable<Int4>>,
            timestamp -> Varchar,
            txHash -> Varchar,
            txIndex -> Int4,
            outIndex -> Int4,
        }
    }

    diesel::allow_tables_to_appear_in_same_query!(
        dao_record,
        did_delete_record,
        did_record,
        pds_list,
        vote_record,
    );
}
