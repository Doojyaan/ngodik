use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::Stage;

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Artifacts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<Flow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wireframes: Option<Vec<Wireframe>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_operations: Option<Vec<Operation>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_plan: Option<Vec<Step>>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Flow {
    pub nodes: Vec<String>,
    pub edges: Vec<[usize; 2]>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Wireframe {
    pub title: String,
    pub mobile: bool,
    pub elements: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Operation {
    pub method: Method,
    pub path: String,
    pub summary: String,
    pub request_description: Option<String>,
    pub response_description: String,
    pub response_status: u16,
    pub requirement_refs: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl Method {
    pub fn name(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post => "post",
            Self::Put => "put",
            Self::Patch => "patch",
            Self::Delete => "delete",
            Self::Head => "head",
            Self::Options => "options",
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step {
    pub id: String,
    pub title: String,
    pub description: String,
    pub depends_on: Vec<String>,
    pub acceptance_criteria: Vec<String>,
}

fn text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit
}

pub fn path_parameters(path: &str) -> Option<Vec<&str>> {
    let mut names = HashSet::new();
    let mut parameters = Vec::new();
    if path.len() > 256 || !path.starts_with('/') {
        return None;
    }
    for segment in path[1..].split('/') {
        if segment.is_empty() {
            if path != "/" {
                return None;
            }
        } else if let Some(name) = segment
            .strip_prefix('{')
            .and_then(|part| part.strip_suffix('}'))
        {
            if name.is_empty()
                || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
                || !names.insert(name)
            {
                return None;
            }
            parameters.push(name);
        } else if segment == "."
            || segment == ".."
            || !segment
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-.".contains(&c))
        {
            return None;
        }
    }
    Some(parameters)
}

impl Artifacts {
    pub fn valid(&self, stage: Stage, refs: &[&str], http_api: bool) -> bool {
        let valid_flow = self.flow.as_ref().is_none_or(|flow| {
            matches!(stage, Stage::Analyst | Stage::Architect)
                && (2..=16).contains(&flow.nodes.len())
                && flow.nodes.iter().all(|name| text(name, 160))
                && (1..=32).contains(&flow.edges.len())
                && flow
                    .edges
                    .iter()
                    .flatten()
                    .all(|&index| index < flow.nodes.len())
        });
        let valid_wireframes = self.wireframes.as_ref().is_none_or(|views| {
            stage == Stage::Ux
                && (1..=8).contains(&views.len())
                && views.iter().all(|view| {
                    text(&view.title, 160)
                        && (1..=10).contains(&view.elements.len())
                        && view.elements.iter().all(|label| text(label, 160))
                })
        });
        let valid_operations = self.api_operations.as_ref().is_none_or(|operations| {
            let mut seen = HashSet::new();
            let mut route_patterns = HashMap::new();
            stage == Stage::Architect
                && http_api
                && (1..=24).contains(&operations.len())
                && operations.iter().all(|operation| {
                    path_parameters(&operation.path).is_some()
                        && {
                            let key = operation
                                .path
                                .split('/')
                                .map(|segment| {
                                    if segment.starts_with('{') {
                                        "{}"
                                    } else {
                                        segment
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("/");
                            *route_patterns.entry(key).or_insert(&operation.path) == &operation.path
                        }
                        && seen.insert((operation.method, &operation.path))
                        && text(&operation.summary, 400)
                        && text(&operation.response_description, 800)
                        && operation
                            .request_description
                            .as_ref()
                            .is_none_or(|description| text(description, 800))
                        && (200..=599).contains(&operation.response_status)
                        && !operation.requirement_refs.is_empty()
                        && operation
                            .requirement_refs
                            .iter()
                            .all(|reference| refs.contains(&reference.as_str()))
                })
        });
        let valid_plan = self.implementation_plan.as_ref().is_none_or(|steps| {
            let mut seen = HashSet::new();
            stage == Stage::Architect
                && (1..=16).contains(&steps.len())
                && steps.iter().all(|step| {
                    step.id.strip_prefix("T-").is_some_and(|id| {
                        !id.is_empty() && id.len() <= 4 && id.bytes().all(|c| c.is_ascii_digit())
                    }) && text(&step.title, 160)
                        && text(&step.description, 800)
                        && !step.acceptance_criteria.is_empty()
                        && step
                            .acceptance_criteria
                            .iter()
                            .all(|criterion| text(criterion, 400))
                        && step.depends_on.iter().all(|id| seen.contains(id))
                        && seen.insert(step.id.clone())
                })
        });
        valid_flow && valid_wireframes && valid_operations && valid_plan
    }
}
