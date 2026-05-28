use anyhow::Result;
use magellan::CodeGraph;
use serde::Serialize;
use sqlitegraph::backend::BackendDirection;
use std::path::Path;

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct NavSymbol {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub file: Option<String>,
    pub line: usize,
}

#[derive(Clone, Serialize)]
pub struct DepthNavSymbol {
    pub depth: u32,
    pub node: NavSymbol,
}

#[derive(Clone, Serialize)]
pub struct EdgeInfo {
    pub edge_type: String,
    pub direction: String,
    pub target: NavSymbol,
}

#[derive(Serialize)]
pub struct NavigateResponse {
    pub node: Option<NavSymbol>,
    pub resolve: Option<Vec<NavSymbol>>,
    pub edges: Option<Vec<EdgeInfo>>,
    pub callers: Option<Vec<DepthNavSymbol>>,
    pub callees: Option<Vec<DepthNavSymbol>>,
}

impl From<magellan::graph::navigator::SymbolInfo> for NavSymbol {
    fn from(si: magellan::graph::navigator::SymbolInfo) -> Self {
        NavSymbol {
            id: si.id,
            name: si.name,
            kind: si.kind,
            file: si.file_path,
            line: si.start_line,
        }
    }
}

impl From<magellan::graph::navigator::DepthSymbol> for DepthNavSymbol {
    fn from(ds: magellan::graph::navigator::DepthSymbol) -> Self {
        DepthNavSymbol {
            depth: ds.depth,
            node: ds.info.into(),
        }
    }
}

fn direction_str(d: &BackendDirection) -> &'static str {
    match d {
        BackendDirection::Outgoing => "out",
        BackendDirection::Incoming => "in",
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run_navigate(
    db_path: &Path,
    symbol: &str,
    id: Option<i64>,
    show_edges: bool,
    show_callers: bool,
    show_callees: bool,
    depth: usize,
    output: crate::output::OutputFormat,
) -> Result<()> {
    let graph = CodeGraph::open(db_path)?;
    let nav = graph.navigator();

    let start_id = if let Some(eid) = id {
        eid
    } else {
        let resolved = nav.resolve(symbol)?;
        if resolved.is_empty() {
            match output {
                crate::output::OutputFormat::Json | crate::output::OutputFormat::Pretty => {
                    println!(r#"{{"error":"no symbols found for '{}'"}}"#, symbol);
                }
                crate::output::OutputFormat::Human => {
                    eprintln!("error: no symbols found for '{}'", symbol);
                }
            }
            return Ok(());
        }
        resolved[0].id
    };

    let info = nav.info(start_id)?;
    let node: Option<NavSymbol> = info.map(Into::into);

    let resolve = if id.is_none() {
        let r = nav.resolve(symbol)?;
        Some(r.into_iter().map(Into::into).collect())
    } else {
        None
    };

    let edges = if show_edges {
        let e = nav.expand(start_id)?;
        Some(
            e.into_iter()
                .map(|hop| EdgeInfo {
                    edge_type: hop.edge_type,
                    direction: direction_str(&hop.direction).to_string(),
                    target: hop.target.into(),
                })
                .collect(),
        )
    } else {
        None
    };

    let callers = if show_callers {
        Some(
            nav.k_hop_callers(start_id, depth as u32)?
                .into_iter()
                .map(Into::into)
                .collect(),
        )
    } else {
        None
    };

    let callees = if show_callees {
        Some(
            nav.k_hop_callees(start_id, depth as u32)?
                .into_iter()
                .map(Into::into)
                .collect(),
        )
    } else {
        None
    };

    let response = NavigateResponse {
        node,
        resolve,
        edges,
        callers,
        callees,
    };

    match output {
        crate::output::OutputFormat::Json | crate::output::OutputFormat::Pretty => {
            let wrapped = crate::output::json_response(&response);
            let json_str = match output {
                crate::output::OutputFormat::Pretty => serde_json::to_string_pretty(&wrapped)?,
                _ => serde_json::to_string(&wrapped)?,
            };
            println!("{}", json_str);
        }
        crate::output::OutputFormat::Human => print_human(&response),
    }

    Ok(())
}

fn print_human(resp: &NavigateResponse) {
    if let Some(ref node) = resp.node {
        println!(
            "id={} kind={} name=\"{}\" file={}",
            node.id,
            node.kind,
            node.name,
            node.file.as_deref().unwrap_or("?")
        );
    }
    if let Some(ref resolve) = resp.resolve {
        for s in resolve {
            println!(
                "id={} kind={} name=\"{}\" file={}",
                s.id,
                s.kind,
                s.name,
                s.file.as_deref().unwrap_or("?")
            );
        }
    }
    if let Some(ref callers) = resp.callers {
        println!("  callers:");
        for c in callers {
            println!(
                "    {} id={} depth={} ({})",
                c.node.name,
                c.node.id,
                c.depth,
                c.node.file.as_deref().unwrap_or("?")
            );
        }
    }
    if let Some(ref callees) = resp.callees {
        println!("  callees:");
        for c in callees {
            println!(
                "    {} id={} depth={} ({})",
                c.node.name,
                c.node.id,
                c.depth,
                c.node.file.as_deref().unwrap_or("?")
            );
        }
    }
    if let Some(ref edges) = resp.edges {
        println!("  edges:");
        for e in edges {
            println!(
                "    {} {} {} ({})",
                e.direction,
                e.edge_type,
                e.target.name,
                e.target.file.as_deref().unwrap_or("?")
            );
        }
    }
}
