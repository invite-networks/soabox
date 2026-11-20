//! Zone templates.

use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;

use super::{Store, now};

/// One record in a template. `name` is relative: `@` or a label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TemplateRecord {
    pub name: String,
    #[serde(rename = "type")]
    pub rtype: String,
    pub ttl: i64,
    pub content: String,
}

/// A stored template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Template {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub records: Vec<TemplateRecord>,
}

impl Store {
    /// Lists templates with their records.
    pub async fn list_templates(&self) -> Result<Vec<Template>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, name, description FROM templates ORDER BY name")
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let id: i64 = r.try_get(0)?;
            out.push(Template {
                id,
                name: r.try_get(1)?,
                description: r.try_get(2)?,
                records: self.template_records(id).await?,
            });
        }
        Ok(out)
    }

    /// Gets one template.
    pub async fn get_template(&self, id: i64) -> Result<Option<Template>, sqlx::Error> {
        Ok(self
            .list_templates()
            .await?
            .into_iter()
            .find(|t| t.id == id))
    }

    async fn template_records(&self, id: i64) -> Result<Vec<TemplateRecord>, sqlx::Error> {
        sqlx::query(
            "SELECT name, type, ttl, content FROM template_records WHERE template_id = $1 ORDER BY id",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| {
            Ok(TemplateRecord {
                name: r.try_get(0)?,
                rtype: r.try_get(1)?,
                ttl: r.try_get(2)?,
                content: r.try_get(3)?,
            })
        })
        .collect()
    }

    /// Creates or replaces a template. `id` `None` creates.
    pub async fn save_template(
        &self,
        id: Option<i64>,
        name: &str,
        description: &str,
        records: &[TemplateRecord],
    ) -> Result<Option<i64>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let id = match id {
            None => sqlx::query(
                "INSERT INTO templates (name, description, created_at) VALUES ($1, $2, $3) RETURNING id",
            )
            .bind(name)
            .bind(description)
            .bind(now())
            .fetch_one(&mut *tx)
            .await?
            .try_get(0)?,
            Some(id) => {
                let n = sqlx::query("UPDATE templates SET name = $1, description = $2 WHERE id = $3")
                    .bind(name)
                    .bind(description)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected();
                if n == 0 {
                    return Ok(None);
                }
                sqlx::query("DELETE FROM template_records WHERE template_id = $1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                id
            }
        };
        for r in records {
            sqlx::query(
                "INSERT INTO template_records (template_id, name, type, ttl, content) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(&r.name)
            .bind(&r.rtype)
            .bind(r.ttl)
            .bind(&r.content)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(Some(id))
    }

    /// Deletes a template.
    pub async fn delete_template(&self, id: i64) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query("DELETE FROM templates WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected()
            > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testing;

    #[tokio::test]
    async fn template_crud() {
        let (s, _d) = testing::sqlite().await;
        let rec = TemplateRecord {
            name: "@".into(),
            rtype: "MX".into(),
            ttl: 3600,
            content: "10 mail.example.net.".into(),
        };
        let id = s
            .save_template(None, "mail", "d", std::slice::from_ref(&rec))
            .await
            .expect("save")
            .expect("id");
        assert_eq!(
            s.get_template(id).await.expect("q").expect("t").records,
            vec![rec]
        );
        assert!(s.save_template(None, "mail", "", &[]).await.is_err());
        s.save_template(Some(id), "mail2", "", &[])
            .await
            .expect("update")
            .expect("id");
        let t = s.get_template(id).await.expect("q").expect("t");
        assert_eq!(t.name, "mail2");
        assert!(t.records.is_empty());
        assert!(
            s.save_template(Some(999), "x", "", &[])
                .await
                .expect("q")
                .is_none()
        );
        assert!(s.delete_template(id).await.expect("del"));
        assert!(s.list_templates().await.expect("list").is_empty());
    }
}
