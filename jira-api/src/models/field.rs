use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub id: String,
    pub key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orderable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub navigable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub searchable: Option<bool>,
    #[serde(rename = "schema")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<FieldSchema>,
    #[serde(rename = "clauseNames")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clause_names: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSchema {
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<String>,
    #[serde(rename = "customId")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_field_deserialization() {
        let json_data = json!({
            "id": "customfield_10001",
            "key": "customfield_10001",
            "name": "Story Points",
            "custom": true,
            "orderable": true,
            "navigable": true,
            "searchable": true,
            "schema": {
                "type": "number",
                "custom": "com.atlassian.jira.plugin.system.customfieldtypes:float",
                "customId": 10001
            },
            "clauseNames": ["cf[10001]", "Story Points"]
        });

        let field: Field = serde_json::from_value(json_data).unwrap();

        assert_eq!(field.id, "customfield_10001");
        assert_eq!(field.name, "Story Points");
        assert_eq!(field.custom, Some(true));
        assert!(field.schema.is_some());
        assert_eq!(field.schema.unwrap().field_type, "number");
    }
}
