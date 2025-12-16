#![allow(unused)]
use alanthinker_dynamic_get_field_macro::*;
use alanthinker_dynamic_get_field_trait::*;

use std::collections::HashMap;

// 使用宏自动生成实现
#[derive(dynamic_fields, Debug)]
struct Person {
    name: String,
    age: i32,
    score: f64,
    tags: Vec<String>,
    metadata: HashMap<String, String>,
}

impl Person {
    fn new() -> Self {
        let mut metadata = HashMap::new();
        metadata.insert("department".to_string(), "Engineering".to_string());
        metadata.insert("role".to_string(), "Developer".to_string());

        Self {
            name: "Alice".to_string(),
            age: 25,
            score: 95.5,
            tags: vec!["rust".to_string(), "programming".to_string()],
            metadata,
        }
    }
}

#[test]
fn test_basic_field_access() {
    let person = Person::new();

    // 测试 get_field
    let name_field = person.get_field("name").unwrap();
    assert!(name_field.is::<String>());

    // 测试 get_field_safe
    let age_field = person.get_field_safe("age").unwrap();
    assert!(age_field.is::<i32>());

    // 测试 get_field_as
    let name: &String = person.get_field_as("name").unwrap();
    assert_eq!(name, "Alice");

    let age: &i32 = person.get_field_as("age").unwrap();
    assert_eq!(*age, 25);

    // 测试错误情况
    let result = person.get_field_safe("nonexistent");
    assert!(result.is_err());
    assert!(format!("{}", result.unwrap_err()).contains("not found"));

    let result = person.get_field_as::<bool>("name");
    assert!(result.is_err());
    assert!(format!("{}", result.unwrap_err()).contains("is not of type"));
}

#[test]
fn test_has_field() {
    let person = Person::new();

    assert!(person.has_field("name"));
    assert!(person.has_field("age"));
    assert!(person.has_field("score"));
    assert!(person.has_field("tags"));
    assert!(person.has_field("metadata"));
    assert!(!person.has_field("nonexistent"));
}

#[test]
fn test_field_names() {
    let person = Person::new();
    let names = person.field_names();

    assert_eq!(names.len(), 5);
    assert!(names.contains(&"name".to_string()));
    assert!(names.contains(&"age".to_string()));
    assert!(names.contains(&"score".to_string()));
    assert!(names.contains(&"tags".to_string()));
    assert!(names.contains(&"metadata".to_string()));
}

#[test]
fn test_get_all_fields() {
    let person = Person::new();
    let all_fields = person.get_all_fields().unwrap();

    assert_eq!(all_fields.len(), 5);

    // 验证字段名称存在
    let field_names: Vec<String> = all_fields.iter().map(|(name, _)| name.clone()).collect();

    assert!(field_names.contains(&"name".to_string()));
}

#[test]
fn test_get_multiple_fields() {
    let person = Person::new();

    // 测试获取多个字段
    let fields = person.get_multiple_fields(&["name", "age"]).unwrap();
    assert_eq!(fields.len(), 2);

    // // 测试类型化的多个字段
    // let names: Vec<&String> = person.get_multiple_fields_as(&["name", "tags"]).unwrap(); // 这里会失败，因为 tags 是 Vec<String> 不是 &String

    // 测试 has_all_fields
    assert!(person.has_all_fields(&["name", "age"]));
    assert!(!person.has_all_fields(&["name", "nonexistent"]));
}

#[test]
fn test_get_field_cloned() {
    let person = Person::new();

    let name: String = person.get_field_cloned("name").unwrap();
    assert_eq!(name, "Alice");

    let age: i32 = person.get_field_cloned("age").unwrap();
    assert_eq!(age, 25);
}

#[test]
fn test_search_field_name() {
    let person = Person::new();

    let found = person.search_field_name("nam");
    assert_eq!(found, Some("name".to_string()));

    let found = person.search_field_name("meta");
    assert_eq!(found, Some("metadata".to_string()));

    let found = person.search_field_name("nonexistent");
    assert_eq!(found, None);
}

#[test]
fn test_debug_implementation() {
    let person = Person::new();
    let debug_output = format!("{:?}", person);

    assert!(debug_output.contains("Person"));
    assert!(debug_output.contains("name"));
    assert!(debug_output.contains("Alice"));
    assert!(debug_output.contains("age"));
    assert!(debug_output.contains("25"));
}

// 测试嵌套结构
#[derive(dynamic_fields)]
struct NestedStruct {
    person: Person,
    count: usize,
    description: String,
}

#[test]
fn test_nested_struct() {
    let person = Person::new();
    let nested = NestedStruct {
        person,
        count: 42,
        description: "Test nested".to_string(),
    };

    // 测试顶级字段
    let count: &usize = nested.get_field_as("count").unwrap();
    assert_eq!(*count, 42);

    // 测试嵌套字段（注意：这里只能访问顶层的 person 字段，不能访问 person 的内部字段）
    let person_field = nested.get_field("person").unwrap();
    assert!(person_field.is::<Person>());

    // 需要获取 Person 实例后才能访问其内部字段
    let inner_person: &Person = nested.get_field_as("person").unwrap();
    let inner_name: &String = inner_person.get_field_as("name").unwrap();
    assert_eq!(inner_name, "Alice");
}
