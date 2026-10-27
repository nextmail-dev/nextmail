use super::*;
use crate::storage::message_read_repository::{
    ACCOUNT_FULL_TEXT_SEARCH_SQL, MAILBOX_FULL_TEXT_SEARCH_SQL,
};
use sqlx::Row;

#[tokio::test]
async fn full_text_search_runs_match_before_per_message_lookups() {
    let (_directory, repository, mailbox) = repository_with_mailbox(7).await;
    for (global, sql) in [
        (false, MAILBOX_FULL_TEXT_SEARCH_SQL),
        (true, ACCOUNT_FULL_TEXT_SEARCH_SQL),
    ] {
        for cursor in [None, Some(100_i64)] {
            // Only trusted production SQL is appended; all values remain bound.
            let explain = format!("EXPLAIN QUERY PLAN {sql}");
            let mut query = sqlx::query(sqlx::AssertSqlSafe(explain.as_str()));
            if global {
                query = query.bind("slot");
            }
            query = query
                .bind("subject : \"needle\" OR addresses : \"needle\" OR body : \"needle\"")
                .bind("slot")
                .bind("slot");
            if !global {
                query = query.bind(&mailbox.id).bind("slot");
            }
            let rows = query
                .bind(cursor)
                .bind(cursor)
                .bind(cursor)
                .bind(cursor.map(|_| "m040"))
                .bind(51_i64)
                .fetch_all(&repository.pool)
                .await
                .unwrap();
            let steps = rows
                .iter()
                .map(|row| row.get::<String, _>("detail"))
                .collect::<Vec<_>>();
            let fts = steps
                .iter()
                .position(|step| step.contains("message_search VIRTUAL TABLE"))
                .expect("search must use the FTS virtual table");
            for alias in ["m", "l"] {
                let lookup = steps
                    .iter()
                    .position(|step| {
                        step.starts_with(&format!("SEARCH {alias} "))
                            || step.starts_with(&format!("SCAN {alias} "))
                    })
                    .expect("search must resolve matching messages and locations");
                assert!(
                    fts < lookup,
                    "MATCH must not run once per message: {steps:?}"
                );
            }
        }
    }
}

#[tokio::test]
async fn full_text_search_preserves_tied_date_pagination_and_visible_locations() {
    let (_directory, repository, inbox) = repository_with_mailbox(7).await;
    sqlx::query(
        "WITH RECURSIVE seq(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM seq WHERE n<80)
         INSERT INTO messages(id,account_slot_id,subject,received_at)
         SELECT printf('m%03d',n),'slot','search needle',100+n/3 FROM seq",
    )
    .execute(&repository.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO message_locations(id,message_id,mailbox_id,uid,uid_validity,internal_date,local_hidden)
         SELECT 'l'||id,id,?,CAST(substr(id,2) AS INTEGER),7,received_at,id='m080' FROM messages",
    )
    .bind(&inbox.id)
    .execute(&repository.pool)
    .await
    .unwrap();
    sqlx::raw_sql(
        "INSERT INTO mailboxes(id,account_slot_id,remote_name) VALUES('archive','slot','Archive');
         INSERT INTO message_locations(id,message_id,mailbox_id,uid,uid_validity,internal_date,local_hidden)
         VALUES('copy79','m079','archive',1,1,9999,0),('copy78','m078','archive',2,1,10000,1);",
    )
    .execute(&repository.pool)
    .await
    .unwrap();
    let expected = (1..80)
        .rev()
        .map(|n| format!("m{n:03}"))
        .collect::<Vec<_>>();
    for scope in [Some(inbox.id.as_str()), None] {
        let mut ids = Vec::new();
        let mut cursor = None;
        // A finite bound also catches a cursor that stops advancing.
        for _ in 0..10 {
            let page = repository
                .read()
                .search_messages("slot", scope, "needle", cursor.as_deref(), 13)
                .await
                .unwrap();
            for item in page.items {
                let expected_mailbox = if scope.is_none() && item.id == "m079" {
                    "archive"
                } else {
                    &inbox.id
                };
                assert_eq!(item.mailbox_id, expected_mailbox);
                ids.push(item.id);
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert!(cursor.is_none());
        assert_eq!(ids, expected);
        assert!(repository
            .read()
            .search_messages("slot", scope, "absent search phrase", None, 50)
            .await
            .unwrap()
            .items
            .is_empty());
    }
}
