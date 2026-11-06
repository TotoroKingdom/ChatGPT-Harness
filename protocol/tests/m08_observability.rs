use harness_control_protocol::v1::{MetricsQueryParams, TraceGetResult};
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fs;

fn fixture(name: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(format!("fixtures/v1/{name}")).unwrap()).unwrap()
}

fn validate<T>(value: Value) -> T
where
    T: DeserializeOwned + JsonSchema,
{
    let schema = serde_json::to_value(schema_for!(T)).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    validator
        .validate(&value)
        .expect("fixture matches DTO schema");
    serde_json::from_value(value).expect("fixture deserializes into DTO")
}

#[test]
fn m08_observability_fixtures_are_closed_and_typed() {
    let params: MetricsQueryParams = validate(fixture("metrics.query.request.json"));
    assert_eq!(params.tool.as_deref(), Some("read_file"));
    let result: TraceGetResult = validate(fixture("trace.get.result.json"));
    assert_eq!(result.spans.len(), 1);
    assert_eq!(result.spans[0].duration_ms, Some(50));

    let mut unknown = fixture("metrics.query.request.json");
    unknown["unexpected"] = Value::String("rejected".into());
    assert!(serde_json::from_value::<MetricsQueryParams>(unknown).is_err());
}

#[test]
fn m08_generated_observability_artifacts_exist() {
    let generated = fs::read_to_string("generated/typescript/v1.ts").unwrap();
    assert!(generated.contains("export type MetricsQueryResult"));
    assert!(generated.contains("export type TraceGetResult"));
    for name in [
        "metrics-query-params.schema.json",
        "metrics-query-result.schema.json",
        "trace-get-params.schema.json",
        "trace-get-result.schema.json",
    ] {
        assert!(fs::metadata(format!("generated/json-schema/{name}")).is_ok());
    }
}
