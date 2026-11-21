use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

use super::Stage;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub stage: Stage,
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

fn excerpt(value: &Value) -> String {
    const LIMIT: usize = 560;
    let text = match value {
        Value::String(text) => text.clone(),
        _ => serde_json::to_string_pretty(value).unwrap_or_default(),
    };
    let mut chars = text.chars();
    let mut preview: String = chars.by_ref().take(LIMIT).collect();
    if chars.next().is_some() {
        preview.push('…');
    }
    preview
}

fn add(
    changes: &mut Vec<Change>,
    stage: Stage,
    field: &str,
    before: Option<&Value>,
    after: Option<&Value>,
) {
    if before == after {
        return;
    }
    changes.push(Change {
        stage,
        field: field.into(),
        before: before.map(excerpt),
        after: after.map(excerpt),
    });
}

fn by_id(value: &Value) -> BTreeMap<&str, &Value> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| Some((item.get("id")?.as_str()?, item)))
        .collect()
}

pub fn compare(before: &[Value; 4], after: &[Value; 4]) -> Vec<Change> {
    let mut changes = Vec::new();
    for stage in Stage::ALL {
        let initial = &before[stage as usize];
        let proposed = &after[stage as usize];
        if stage == Stage::Product {
            let a = initial.as_object().into_iter().flatten();
            let b = proposed.as_object().into_iter().flatten();
            for name in a
                .chain(b)
                .map(|(name, _)| name.as_str())
                .collect::<BTreeSet<_>>()
            {
                if name == "requirements" {
                    let left = by_id(&initial[name]);
                    let right = by_id(&proposed[name]);
                    for id in left
                        .keys()
                        .chain(right.keys())
                        .copied()
                        .collect::<BTreeSet<_>>()
                    {
                        add(
                            &mut changes,
                            stage,
                            id,
                            left.get(id).copied(),
                            right.get(id).copied(),
                        );
                    }
                } else {
                    add(
                        &mut changes,
                        stage,
                        name,
                        initial.get(name),
                        proposed.get(name),
                    );
                }
            }
        } else {
            add(
                &mut changes,
                stage,
                "sections",
                initial.get("sections"),
                proposed.get("sections"),
            );
        }
    }
    changes
}
