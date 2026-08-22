use serde::de::DeserializeOwned;

pub trait RowExt {
    fn node_as<T>(&self, key: &str) -> Result<T, neo4rs::DeError>
    where
        T: DeserializeOwned;

    fn rel_prop<T>(&self, column_alias: &str, property_name: &str) -> Result<T, neo4rs::DeError>
    where
        T: DeserializeOwned;
}

impl RowExt for neo4rs::Row {
    fn node_as<T>(&self, key: &str) -> Result<T, neo4rs::DeError>
    where
        T: DeserializeOwned,
    {
        let node = self.get::<neo4rs::Node>(key)?;
        let t = node.to::<T>()?;

        Ok(t)
    }

    fn rel_prop<T>(&self, column_alias: &str, property_name: &str) -> Result<T, neo4rs::DeError>
    where
        T: DeserializeOwned,
    {
        let edge = self.get::<neo4rs::Relation>(column_alias)?;
        let property: T = edge.get(property_name)?;

        Ok(property)
    }
}
