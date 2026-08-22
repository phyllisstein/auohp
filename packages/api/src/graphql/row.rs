use auohp_core::eval::Op;
use neo4rs::{BoltType, DeError, Row, Version};
use serde::de::DeserializeOwned;

pub trait RowExt {
    fn column_as<T>(&self, key: &str) -> Result<T, neo4rs::DeError>
    where
        T: DeserializeOwned;

    fn node_as<T>(&self, key: &str) -> Result<T, neo4rs::DeError>
    where
        T: DeserializeOwned,
    {
        let vertex = self.column_as::<neo4rs::Node>(key)?;
        let projection = vertex.to::<T>()?;

        Ok(projection)
    }

    fn rel_prop<T>(&self, column_alias: &str, property_name: &str) -> Result<T, neo4rs::DeError>
    where
        T: DeserializeOwned,
    {
        let edge = self.column_as::<neo4rs::Relation>(column_alias)?;
        let property: T = edge.get(property_name)?;

        Ok(property)
    }
}

impl RowExt for neo4rs::Row {
    fn column_as<T>(&self, key: &str) -> Result<T, neo4rs::DeError>
    where
        T: DeserializeOwned,
    {
        let column = self.get::<T>(key)?;

        Ok(column)
    }
}

pub trait RowStreamExt {
    async fn first_as<T>(&mut self, key: &str) -> Result<Option<T>, neo4rs::Error>
    where
        T: DeserializeOwned,
    {
        let r = self.first_row().await?;
        let row = r.map(|row| row.column_as::<T>(key)).transpose()?;
        Ok(row)
    }

    async fn first_row(&mut self) -> Result<Option<Row>, neo4rs::Error>;
}

impl RowStreamExt for neo4rs::DetachedRowStream {
    async fn first_row(&mut self) -> Result<Option<Row>, neo4rs::Error> {
        self.next().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::nodes::StatementNode;
    use neo4rs::{BoltInteger, BoltList, BoltMap, BoltNode, BoltRelation, BoltString};

    // These tests build Bolt values by hand rather than talking to Neo4j.
    // `Row::new`, `BoltNode::new` and the `Bolt*` types are all public and
    // re-exported at the neo4rs crate root, so the whole trait can be exercised
    // in-process with no database and no async runtime.
    //
    // What is under test here is *our* composition --- that `node_as` reaches
    // through a node column and `rel_prop` reaches through a relationship
    // column --- not neo4rs's deserializer, which we take as given.

    /// Collects `(key, value)` pairs into a `BoltMap` of node or relationship
    /// properties.
    ///
    /// Takes its pairs by value so that `BoltType` never needs to be cloned;
    /// each helper below hands ownership straight through to the map.
    fn properties(pairs: Vec<(&str, BoltType)>) -> BoltMap {
        let mut map = BoltMap::with_capacity(pairs.len());

        for (key, value) in pairs {
            map.put(BoltString::new(key), value);
        }

        map
    }

    /// A `(:Statement)`-shaped node value, ready to be dropped into a row column.
    ///
    /// The id and labels are arbitrary --- `StatementNode` reads neither, since
    /// it deserializes from the property map alone.
    fn node(props: Vec<(&str, BoltType)>) -> BoltType {
        BoltType::Node(BoltNode::new(
            BoltInteger::new(1),
            BoltList::new(),
            properties(props),
        ))
    }

    /// A relationship value carrying `props` on the edge itself.
    ///
    /// `BoltRelation` has no constructor, but all five of its fields are public,
    /// so it is built as a struct literal. The node ids are placeholders; only
    /// the property map is read back.
    fn relation(typ: &str, props: Vec<(&str, BoltType)>) -> BoltType {
        BoltType::Relation(BoltRelation {
            id: BoltInteger::new(10),
            start_node_id: BoltInteger::new(1),
            end_node_id: BoltInteger::new(2),
            typ: BoltString::new(typ),
            properties: properties(props),
        })
    }

    /// Assembles a `Row` from `(alias, value)` pairs.
    ///
    /// `Row::new` takes two parallel lists --- column names and column values
    /// --- and zips them by position into a map. Building the pair together here
    /// makes it impossible for the two lists to fall out of alignment, which is
    /// the one mistake this API invites.
    fn row(columns: Vec<(&str, BoltType)>) -> Row {
        let mut fields = BoltList::with_capacity(columns.len());
        let mut data = BoltList::with_capacity(columns.len());

        for (alias, value) in columns {
            fields.push(BoltType::String(BoltString::new(alias)));
            data.push(value);
        }

        Row::new(fields, data)
    }

    #[test]
    fn node_as_deserializes_a_node_column() {
        let row = row(vec![(
            "span",
            node(vec![
                ("uid", "statement-1".into()),
                ("text", "we were not silent".into()),
                ("words", r#"[{"word":"we"}]"#.into()),
            ]),
        )]);

        let statement = row
            .node_as::<StatementNode>("span")
            .expect("a well-formed node column should deserialize");

        assert_eq!(statement.uid, "statement-1");
        assert_eq!(statement.text, "we were not silent");
        assert_eq!(statement.words.as_deref(), Some(r#"[{"word":"we"}]"#));
    }

    #[test]
    fn node_as_defaults_absent_words_to_none() {
        // Older transcripts were seeded before word-level timing existed, so the
        // `words` property is missing from the node entirely. `#[serde(default)]`
        // on the field is what turns that absence into `None` instead of an error.
        let row = row(vec![(
            "span",
            node(vec![
                ("uid", "statement-2".into()),
                ("text", "act up".into()),
            ]),
        )]);

        let statement = row
            .node_as::<StatementNode>("span")
            .expect("a node missing an optional property should still deserialize");

        assert_eq!(statement.words, None);
    }

    #[test]
    fn node_as_errors_on_an_unknown_alias() {
        let row = row(vec![("span", node(vec![("uid", "statement-3".into())]))]);

        // Asking for a column the query never returned is an error, not a panic
        // and not a silent `None` --- a missing alias means the Cypher and the
        // Rust have drifted apart, which callers should hear about.
        assert!(row.node_as::<StatementNode>("statement").is_err());
    }

    #[test]
    fn node_as_errors_when_the_column_is_not_a_node() {
        // The column exists but holds a scalar, so the intermediate
        // `column_as::<neo4rs::Node>` step inside `node_as` fails. This is the
        // case a hand-written `row.get::<Node>(..)` would hit too --- the point
        // is that routing through the trait does not swallow it.
        let row = row(vec![("span", "not a node".into())]);

        assert!(row.node_as::<StatementNode>("span").is_err());
    }

    #[test]
    fn rel_prop_reads_a_property_off_the_edge() {
        // Timing lives on the `:CONTAINS` relationship rather than the statement
        // node, which is the whole reason `rel_prop` exists as a separate method.
        let row = row(vec![(
            "meta",
            relation(
                "CONTAINS",
                vec![
                    ("startTime", 12.5_f64.into()),
                    ("endTime", 19.25_f64.into()),
                ],
            ),
        )]);

        let start_time = row
            .rel_prop::<f64>("meta", "startTime")
            .expect("startTime should read back off the relationship");
        let end_time = row
            .rel_prop::<f64>("meta", "endTime")
            .expect("endTime should read back off the relationship");

        assert_eq!(start_time, 12.5);
        assert_eq!(end_time, 19.25);
    }

    #[test]
    fn rel_prop_errors_on_an_unknown_property() {
        let row = row(vec![(
            "meta",
            relation("CONTAINS", vec![("startTime", 0.0_f64.into())]),
        )]);

        assert!(row.rel_prop::<f64>("meta", "endTime").is_err());
    }
}
