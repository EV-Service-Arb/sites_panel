//! `contracts/proto/sa/v1` → TS types of the protojson the panel reads and writes.
//!
//! The envelope (`Event`, `Subject`, the ingest request and answer) is named as protojson writes
//! it, in lowerCamelCase; an event's `properties` are kept as sent (`wire::decode`), and every
//! site sends the proto's own snake_case names there, so a properties message and what it
//! names keep them. int64 is a number: pbjson reads a number or a string, and the sites send
//! numbers.

use std::collections::{BTreeMap, BTreeSet};

use ev_lib::ts_gen::Ts;
use panel::wire::REGISTERED;
use prost::Message;
use prost_types::{
	DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet,
	field_descriptor_proto::{Label, Type},
};

/// What the panel answers: pbjson leaves a field at its default out of what it writes.
const ANSWERS: [&str; 2] = ["IngestResponse", "EventResult"];

pub fn types() -> Ts {
	let set = FileDescriptorSet::decode(panel_contracts::SA_V1_DESCRIPTORS).expect("panel_contracts embeds a FileDescriptorSet");
	let file = set.file.iter().find(|f| f.package() == "sa.v1").expect("the descriptors hold sa.v1");
	let by_name: BTreeMap<&str, &DescriptorProto> = file.message_type.iter().map(|m| (m.name(), m)).collect();

	let mut properties: BTreeSet<&str> = REGISTERED.iter().map(|(.., m)| m.strip_prefix("sa.v1.").expect("a registered message is sa.v1's")).collect();
	let mut todo: Vec<&str> = properties.iter().copied().collect();
	while let Some(name) = todo.pop() {
		for field in &by_name[name].field {
			// A nested name is a map's entry, written inline.
			if let Some(named) = field.type_name().strip_prefix(".sa.v1.").filter(|n| !n.contains('.'))
				&& properties.insert(named)
			{
				todo.push(named);
			}
		}
	}

	let comments = comments(file);
	let mut decls = BTreeMap::new();
	for (i, message) in file.message_type.iter().enumerate() {
		let name = message.name();
		let snake = properties.contains(name);
		let answer = ANSWERS.contains(&name);
		let mut out = doc(comments.get(&vec![4, i as i32]), "");
		if message.field.is_empty() {
			out.push_str(&format!("export type {name} = Record<string, never>;"));
		} else {
			out.push_str(&format!("export type {name} = {{\n"));
			for (j, field) in message.field.iter().enumerate() {
				out.push_str(&doc(comments.get(&vec![4, i as i32, 2, j as i32]), "  "));
				let key = if snake { field.name() } else { field.json_name() };
				let implicit = field.r#type() != Type::Message && field.label() != Label::Repeated && !field.proto3_optional();
				let (ty, map) = ts_type(field, message);
				// A map left out is an empty one.
				let optional = field.proto3_optional() || map || (answer && implicit);
				out.push_str(&format!("  {key}{}: {ty};\n", if optional { "?" } else { "" }));
			}
			out.push_str("};");
		}
		decls.insert(name.to_owned(), out);
	}
	let registered: Vec<String> = REGISTERED
		.iter()
		.map(|(ty, version, m)| format!("  \"{ty}@{version}\": {};\n", m.trim_start_matches("sa.v1.")))
		.collect();
	decls.insert(
		"Registered".into(),
		format!(
			"/** Every `type@version` the panel projects, and its properties. */\nexport type Registered = {{\n{}}};",
			registered.concat()
		),
	);
	Ts::Types(decls)
}

/// The field's TS type, and whether it is a map.
fn ts_type(field: &FieldDescriptorProto, parent: &DescriptorProto) -> (String, bool) {
	let one = match field.r#type() {
		Type::String | Type::Bytes => "string".to_owned(),
		Type::Bool => "boolean".to_owned(),
		Type::Message => match field.type_name() {
			".google.protobuf.Timestamp" => "string".to_owned(),
			".google.protobuf.Struct" => "{ [key: string]: unknown }".to_owned(),
			named => {
				let local = named.rsplit('.').next().expect("a type name has a last segment");
				if let Some(entry) = parent.nested_type.iter().find(|n| n.name() == local && n.options.as_ref().is_some_and(|o| o.map_entry())) {
					return (format!("{{ [key: string]: {} }}", ts_type(&entry.field[1], entry).0), true);
				}
				named.strip_prefix(".sa.v1.").unwrap_or_else(|| panic!("{named}: a message panel_gen has no TS for")).to_owned()
			}
		},
		Type::Enum | Type::Group => panic!("{}: sa.v1 spells its vocabularies as strings", field.name()),
		_ => "number".to_owned(),
	};
	(if field.label() == Label::Repeated { format!("{one}[]") } else { one }, false)
}

/// Each element's leading comment, by its path in the file (`[4, message]`, `[4, message, 2, field]`).
fn comments(file: &FileDescriptorProto) -> BTreeMap<Vec<i32>, String> {
	file.source_code_info
		.as_ref()
		.expect("protox keeps the source info")
		.location
		.iter()
		.filter_map(|l| Some((l.path.clone(), l.leading_comments.clone()?)))
		.collect()
}

fn doc(comment: Option<&String>, indent: &str) -> String {
	let Some(comment) = comment.map(|c| c.trim()).filter(|c| !c.is_empty()) else {
		return String::new();
	};
	let lines: Vec<String> = comment
		.lines()
		.map(|l| format!("{indent} *{}{}", if l.trim().is_empty() { "" } else { " " }, l.trim_start_matches(' ').replace("*/", "*\\/")))
		.collect();
	format!("{indent}/**\n{}\n{indent} */\n", lines.join("\n"))
}
