//! Audit history rows.

use serde::Serialize;
use sqlx::Row;

use super::{Store, now};

/// A history row to insert.
#[derive(Debug, Clone, Default)]
pub struct NewHistory {
    pub actor_type: String,
    pub actor_id: Option<i64>,
    pub actor_name: String,
    pub client_ip: String,
    pub operation: String,
    pub zone_name: Option<String>,
    pub outcome: String,
    pub upstream_status: Option<i64>,
    pub diff: Option<serde_json::Value>,
    pub detail: Option<serde_json::Value>,
}

/// A stored history row.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HistoryEntry {
    pub id: i64,
    pub at: i64,
    pub actor_type: String,
    pub actor_id: Option<i64>,
    pub actor_name: String,
    pub client_ip: String,
    pub operation: String,
    pub zone_name: Option<String>,
    pub outcome: String,
    pub upstream_status: Option<i64>,
    pub diff: Option<serde_json::Value>,
    pub detail: Option<serde_json::Value>,
}

/// Filters for listing history. Empty fields do not filter.
#[derive(Debug, Clone, Default)]
pub struct HistoryFilter {
    /// Only these zones. `None` means all zones.
    pub zones: Option<Vec<String>>,
    pub zone: Option<String>,
    pub actor: Option<String>,
    pub operation: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub before_id: Option<i64>,
    pub limit: i64,
}

impl Store {
    /// Inserts a history row.
    pub async fn insert_history(&self, h: &NewHistory) -> Result<i64, sqlx::Error> {
        sqlx::query(
            "INSERT INTO history (at, actor_type, actor_id, actor_name, client_ip, operation, \
             zone_name, outcome, upstream_status, diff_json, detail_json) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING id",
        )
        .bind(now())
        .bind(&h.actor_type)
        .bind(h.actor_id)
        .bind(&h.actor_name)
        .bind(&h.client_ip)
        .bind(&h.operation)
        .bind(&h.zone_name)
        .bind(&h.outcome)
        .bind(h.upstream_status)
        .bind(h.diff.as_ref().map(|v| v.to_string()))
        .bind(h.detail.as_ref().map(|v| v.to_string()))
        .fetch_one(&self.pool)
        .await?
        .try_get(0)
    }

    /// Lists history, newest first.
    pub async fn list_history(&self, f: &HistoryFilter) -> Result<Vec<HistoryEntry>, sqlx::Error> {
        let mut sql = String::from(
            "SELECT id, at, actor_type, actor_id, actor_name, client_ip, operation, zone_name, \
             outcome, upstream_status, diff_json, detail_json FROM history WHERE 1 = 1",
        );
        let mut binds: Vec<Bind> = Vec::new();
        let mut push = |sql: &mut String, clause: &str, b: Bind| {
            binds.push(b);
            sql.push_str(&clause.replace("$?", &format!("${}", binds.len())));
        };
        if let Some(z) = &f.zone {
            push(&mut sql, " AND zone_name = $?", Bind::S(z.clone()));
        }
        if let Some(a) = &f.actor {
            push(&mut sql, " AND actor_name = $?", Bind::S(a.clone()));
        }
        if let Some(o) = &f.operation {
            push(&mut sql, " AND operation = $?", Bind::S(o.clone()));
        }
        if let Some(s) = f.since {
            push(&mut sql, " AND at >= $?", Bind::I(s));
        }
        if let Some(u) = f.until {
            push(&mut sql, " AND at <= $?", Bind::I(u));
        }
        if let Some(b) = f.before_id {
            push(&mut sql, " AND id < $?", Bind::I(b));
        }
        if let Some(zones) = &f.zones {
            if zones.is_empty() {
                return Ok(Vec::new());
            }
            sql.push_str(" AND zone_name IN (");
            for (i, z) in zones.iter().enumerate() {
                if i > 0 {
                    sql.push_str(", ");
                }
                push(&mut sql, "$?", Bind::S(z.clone()));
            }
            sql.push(')');
        }
        let limit = if f.limit <= 0 { 100 } else { f.limit.min(1000) };
        push(&mut sql, " ORDER BY id DESC LIMIT $?", Bind::I(limit));

        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
        for b in &binds {
            q = match b {
                Bind::S(s) => q.bind(s.clone()),
                Bind::I(i) => q.bind(*i),
            };
        }
        let rows = q.fetch_all(&self.pool).await?;
        rows.iter()
            .map(|r| {
                let diff: Option<String> = r.try_get(10)?;
                let detail: Option<String> = r.try_get(11)?;
                Ok(HistoryEntry {
                    id: r.try_get(0)?,
                    at: r.try_get(1)?,
                    actor_type: r.try_get(2)?,
                    actor_id: r.try_get(3)?,
                    actor_name: r.try_get(4)?,
                    client_ip: r.try_get(5)?,
                    operation: r.try_get(6)?,
                    zone_name: r.try_get(7)?,
                    outcome: r.try_get(8)?,
                    upstream_status: r.try_get(9)?,
                    diff: diff.and_then(|s| serde_json::from_str(&s).ok()),
                    detail: detail.and_then(|s| serde_json::from_str(&s).ok()),
                })
            })
            .collect()
    }
}

enum Bind {
    S(String),
    I(i64),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testing;

    #[tokio::test]
    async fn insert_and_filter() {
        let (s, _d) = testing::sqlite().await;
        for (zone, actor) in [("a.test.", "k1"), ("b.test.", "k2"), ("a.test.", "k2")] {
            s.insert_history(&NewHistory {
                actor_type: "api_key".into(),
                actor_name: actor.into(),
                operation: "rrsets.patch".into(),
                zone_name: Some(zone.into()),
                outcome: "success".into(),
                diff: Some(serde_json::json!([{"name": "www"}])),
                ..Default::default()
            })
            .await
            .expect("insert");
        }
        let all = s
            .list_history(&HistoryFilter::default())
            .await
            .expect("all");
        assert_eq!(all.len(), 3);
        assert!(all[0].id > all[1].id);
        assert_eq!(all[0].diff, Some(serde_json::json!([{"name": "www"}])));

        let f = HistoryFilter {
            zone: Some("a.test.".into()),
            actor: Some("k2".into()),
            ..Default::default()
        };
        assert_eq!(s.list_history(&f).await.expect("f").len(), 1);

        let f = HistoryFilter {
            zones: Some(vec!["b.test.".into()]),
            ..Default::default()
        };
        assert_eq!(s.list_history(&f).await.expect("f").len(), 1);

        let f = HistoryFilter {
            zones: Some(vec![]),
            ..Default::default()
        };
        assert!(s.list_history(&f).await.expect("f").is_empty());

        let f = HistoryFilter {
            before_id: Some(all[0].id),
            limit: 1,
            ..Default::default()
        };
        let page = s.list_history(&f).await.expect("f");
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, all[1].id);
    }
}
