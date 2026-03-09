//! Exploration of different strategies for handling optional/missing Neo4j node
//! properties in neo4rs 0.8.
//!
//! The problem: Neo4j nodes seeded at different times may or may not have a
//! given property. In Rust, we model this as `Option<String>`. But neo4rs's
//! `Node::get::<Option<String>>("words")` fails with `NoSuchProperty` when the
//! key is entirely absent from the node — the HashMap lookup fails before the
//! `Option` type parameter ever gets a chance to run.
//!
//! This binary exercises seven different approaches against live Neo4j data,
//! demonstrating where each one succeeds, fails, and gets weird.
//!
//! Run with:
//!   cargo run --bin explore_deser

use anyhow::{Context, Result};
use neo4rs::{query, Graph, Node};
use serde::Deserialize;

// ═══════════════════════════════════════════════════════════════════════════════
// Approach 0: The Bug
// ═══════════════════════════════════════════════════════════════════════════════
//
// This is the original broken code. It calls `node.get::<Option<String>>("words")`
// and expects the `Option` to absorb missing properties. It doesn't.

async fn approach_0_the_bug(graph: &Graph) -> Result<()> {
    println!("\n{}", "=".repeat(72));
    println!("APPROACH 0: The Bug (node.get::<Option<String>>)");
    println!("{}", "=".repeat(72));

    let mut stream = graph
        .execute(query(
            "MATCH (t:Transcript)-[c:CONTAINS]->(s:Statement)<-[:SAYS]-(p:Person)
             RETURN s, p, c
             ORDER BY c.startTime",
        ))
        .await?;

    while let Some(row) = stream.next().await? {
        let s: Node = row.get("s")?;
        let uid: String = s.get("uid")?;

        // This is the line that blows up on st-002. Node::get does a HashMap
        // lookup first. If the key "words" isn't in the property map at all,
        // it returns Err(NoSuchProperty) immediately — before TryFrom<BoltType>
        // or serde's Option deserializer ever run.
        //
        // The type parameter Option<String> is *irrelevant* to the failure.
        // It might as well be Option<i64> or Option<Vec<u8>>. The error fires
        // at the map lookup layer, not the type conversion layer.
        let words: std::result::Result<Option<String>, _> = s.get("words");

        match words {
            Ok(w) => println!(
                "  {} => words: {:?}",
                uid,
                w.map(|s| s[..30.min(s.len())].to_string())
            ),
            Err(e) => println!("  {} => ERROR: {}", uid, e),
        }

        // The root cause in neo4rs source (map.rs:45-53):
        //
        //   pub fn get<'this, T>(&'this self, key: &str) -> Result<T, DeError>
        //   where T: Deserialize<'this>
        //   {
        //       match self.value.get(key) {      // <-- HashMap::get
        //           Some(v) => v.to(),            // <-- serde runs HERE
        //           None => Err(DeError::NoSuchProperty),  // <-- short-circuit
        //       }
        //   }
        //
        // Two completely different error semantics are collapsed:
        //   1. "key absent from map"  (→ should be None for Option<T>)
        //   2. "key present but wrong type" (→ genuine error)
        //
        // The get() method can't distinguish them because it never sees the
        // type parameter in the None branch.
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Approach A: Cypher Projection ("push nullability into the query")
// ═══════════════════════════════════════════════════════════════════════════════
//
// Instead of returning whole nodes, project individual properties in the RETURN
// clause. Cypher's property access (`s.words`) returns `null` for missing
// properties — not an absent key. So `row.get::<Option<String>>("words")`
// receives a BoltType::Null, which the serde Deserializer handles correctly
// via `deserialize_option` → `visitor.visit_none()`.
//
// The key insight: this moves the "absent → null" coercion from Rust into
// Cypher. The Cypher engine does the normalization for us.

async fn approach_a_cypher_projection(graph: &Graph) -> Result<()> {
    println!("\n{}", "=".repeat(72));
    println!("APPROACH A: Cypher Projection (s.words AS words)");
    println!("{}", "=".repeat(72));

    // Notice: we return `s.words AS words` instead of the whole node `s`.
    // Cypher evaluates `s.words` and, if the property doesn't exist on the
    // node, yields `null`. That null travels over Bolt as BoltType::Null,
    // arrives in the Row's BoltMap, and serde's Option deserializer handles it.
    let mut stream = graph
        .execute(query(
            "MATCH (t:Transcript)-[c:CONTAINS]->(s:Statement)<-[:SAYS]-(p:Person)
             RETURN s.uid AS uid,
                    s.text AS text,
                    s.words AS words,
                    p.name AS speaker,
                    c.startTime AS start_time,
                    c.endTime AS end_time
             ORDER BY c.startTime",
        ))
        .await?;

    while let Some(row) = stream.next().await? {
        let uid: String = row.get("uid")?;
        let _text: String = row.get("text")?;
        let words: Option<String> = row.get("words")?; // Works! BoltType::Null → None
        let speaker: String = row.get("speaker")?;
        let start: f64 = row.get("start_time")?;

        println!(
            "  {} [{}] @ {:.1}s => words: {}",
            uid,
            speaker,
            start,
            words.as_ref().map_or("None", |_| "Some(...)")
        );
    }

    // Trade-offs:
    //
    // + Simplest mental model — the query tells you exactly what you get.
    // + No intermediate structs, no serde derives needed.
    // + Works with plain row.get() calls.
    //
    // - Breaks the "return whole nodes" convention. The query now returns a
    //   flat bag of scalars, losing the structural information about which
    //   values came from which node.
    //
    // - Column names become an implicit contract between Cypher and Rust.
    //   Rename `AS words` to `AS word_timing` and Rust silently gets
    //   NoSuchProperty at runtime — no compile-time safety net.
    //
    // - Scales poorly: as nodes gain properties, the RETURN clause grows into
    //   a long list of `s.foo AS foo, s.bar AS bar, ...` projections.

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Approach A': Cypher COALESCE — the weirder sibling
// ═══════════════════════════════════════════════════════════════════════════════
//
// What if you don't want null at all? COALESCE lets you substitute a sentinel
// value for missing properties. This can be useful if downstream code can't
// handle Option at all, or if you want to distinguish "property was null" from
// "property was absent" (though Neo4j itself doesn't store null — a null
// property is just an absent property).

async fn approach_a_prime_coalesce(graph: &Graph) -> Result<()> {
    println!("\n{}", "=".repeat(72));
    println!("APPROACH A': Cypher COALESCE (sentinel substitution)");
    println!("{}", "=".repeat(72));

    let mut stream = graph
        .execute(query(
            "MATCH (t:Transcript)-[c:CONTAINS]->(s:Statement)<-[:SAYS]-(p:Person)
             RETURN s.uid AS uid,
                    s.text AS text,
                    COALESCE(s.words, '[]') AS words,
                    p.name AS speaker
             ORDER BY c.startTime",
        ))
        .await?;

    while let Some(row) = stream.next().await? {
        let uid: String = row.get("uid")?;
        let words: String = row.get("words")?; // Always a String — never None
        let speaker: String = row.get("speaker")?;

        // Now `words` is always a valid JSON array string. We can parse it
        // directly without an Option wrapper.
        let word_count: usize = serde_json::from_str::<Vec<serde_json::Value>>(&words)
            .map(|v| v.len())
            .unwrap_or(0);

        println!("  {} [{}] => {} words", uid, speaker, word_count);
    }

    // This is weird because it pushes *business logic* into the query:
    // "an absent words property means an empty JSON array." That's a semantic
    // decision that lives in Cypher now, invisible to Rust's type system.
    //
    // It also means the Rust type is `String` (not `Option<String>`), which
    // loses the ability to distinguish "no word timing" from "word timing with
    // zero words." Probably fine here, but a real footgun in other contexts.

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Approach B: The serde(default) Intermediate Struct (chosen solution)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Create a struct that covers only the node-native properties, with
// `#[serde(default)]` on optional fields. Use `node.to::<StatementNode>()`.
//
// `node.to()` drives neo4rs's custom serde Deserializer. The Deserializer's
// `deserialize_outer_struct` method (element.rs:60-93) iterates the struct's
// declared fields. For fields NOT present in the node's property map, it
// synthesizes an `AdditionalData::Element` entry — a phantom deserializer
// that, when asked to `deserialize_option`, calls `visitor.visit_none()`,
// and when asked to `deserialize_any`, returns `PropertyMissingButRequired`.
//
// So the path for `words: Option<String>` with `#[serde(default)]` on a node
// that lacks the property:
//
//   1. serde's generated Deserialize impl sees "words" isn't in the data
//   2. #[serde(default)] kicks in → calls Default::default() → None
//   3. The AdditionalData deserializer is never consulted at all
//
// Without `#[serde(default)]`, serde would try to deserialize the
// AdditionalData::Element, which calls `deserialize_option` →
// `visitor.visit_none()` → Ok(None). So actually, for `Option<T>` fields
// specifically, `#[serde(default)]` is redundant! The neo4rs Deserializer
// already handles it. But `#[serde(default)]` is more explicit and protects
// against changes in neo4rs's Deserializer implementation.

#[derive(Deserialize, Debug)]
struct StatementNode {
    uid: String,
    text: String,
    #[serde(default)]
    words: Option<String>,
}

async fn approach_b_serde_default(graph: &Graph) -> Result<()> {
    println!("\n{}", "=".repeat(72));
    println!("APPROACH B: #[serde(default)] Intermediate Struct");
    println!("{}", "=".repeat(72));

    let mut stream = graph
        .execute(query(
            "MATCH (t:Transcript)-[c:CONTAINS]->(s:Statement)<-[:SAYS]-(p:Person)
             RETURN s, p, c
             ORDER BY c.startTime",
        ))
        .await?;

    while let Some(row) = stream.next().await? {
        let s: Node = row.get("s")?;
        let sn: StatementNode = s.to()?; // Whole-node deserialization

        let p: Node = row.get("p")?;
        let speaker: String = p.get("name")?;

        println!(
            "  {} [{}] => words: {:?}",
            sn.uid,
            speaker,
            sn.words
                .as_ref()
                .map(|s| format!("{}...", &s[..20.min(s.len())]))
        );
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Approach C: Row::to() — Flatten the Entire Row into a Single Struct
// ═══════════════════════════════════════════════════════════════════════════════
//
// Row::to() (row.rs:170-178) tries two things:
//   1. `to_strict()` — deserialize the row's BoltMap directly as a struct
//   2. If that fails AND the row has exactly one column, try deserializing
//      that single value as the struct
//
// With `to_strict`, the row's column names become struct field names. If we
// carefully name our RETURN columns to match struct fields, we can deserialize
// the whole row in one shot — including nested nodes.
//
// This is the weirdest approach because it treats the Row itself as a
// first-class map deserializer, blurring the line between "query result shape"
// and "application data model."

#[derive(Deserialize, Debug)]
struct FlatRow {
    uid: String,
    text: String,
    #[serde(default)]
    words: Option<String>,
    speaker: String,
    start_time: f64,
    end_time: f64,
}

async fn approach_c_row_to(graph: &Graph) -> Result<()> {
    println!("\n{}", "=".repeat(72));
    println!("APPROACH C: row.to::<FlatRow>() (full-row deserialization)");
    println!("{}", "=".repeat(72));

    // Column aliases in the RETURN must exactly match FlatRow field names.
    let mut stream = graph
        .execute(query(
            "MATCH (t:Transcript)-[c:CONTAINS]->(s:Statement)<-[:SAYS]-(p:Person)
             RETURN s.uid AS uid,
                    s.text AS text,
                    s.words AS words,
                    p.name AS speaker,
                    c.startTime AS start_time,
                    c.endTime AS end_time
             ORDER BY c.startTime",
        ))
        .await?;

    while let Some(row) = stream.next().await? {
        let flat: FlatRow = row.to()?;
        println!(
            "  {} [{}] @ {:.1}-{:.1}s => words: {}",
            flat.uid,
            flat.speaker,
            flat.start_time,
            flat.end_time,
            flat.words.as_ref().map_or("None", |_| "Some(...)")
        );
    }

    // Trade-offs:
    //
    // + One deserialization call for the whole row. Very ergonomic.
    // + Compile-time struct fields enforce that all expected columns exist.
    //
    // - Same Cypher projection problem as Approach A — you're returning
    //   scalar columns, not whole nodes.
    // - The query and the struct are tightly coupled by field name. Refactoring
    //   either one requires changing the other.
    // - Can't nest: if you want `Person` as a sub-struct, you'd need to return
    //   `p` as a column and somehow tell serde to deserialize a BoltNode into
    //   a nested field. (Spoiler: Approach D does this.)

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Approach D: Row::to() with Nested Node Deserialization
// ═══════════════════════════════════════════════════════════════════════════════
//
// Can we return whole nodes in the RETURN clause AND deserialize the entire
// row into a struct with nested sub-structs? Yes — with a catch.
//
// row.to_strict() deserializes the Row's BoltMap, where each column is a key.
// If a column's value is a BoltType::Node, neo4rs's Deserializer will call
// BoltNodeDeserializer, which iterates the node's properties and feeds them
// to the nested struct's Deserialize impl.
//
// So `RETURN s, p` gives a BoltMap with two entries:
//   "s" → BoltType::Node(...)
//   "p" → BoltType::Node(...)
//
// And row.to::<NestedRow>() can deserialize `s` into a StatementNode and
// `p` into a PersonNode, as nested fields.

#[derive(Deserialize, Debug)]
struct PersonNode {
    #[allow(dead_code)]
    uid: String,
    name: String,
}

#[derive(Deserialize, Debug)]
struct NestedRow {
    s: StatementNode,
    p: PersonNode,
    // Scalars projected from the query still work alongside whole nodes:
    is_interviewer: bool,
}

async fn approach_d_nested_row(graph: &Graph) -> Result<()> {
    println!("\n{}", "=".repeat(72));
    println!("APPROACH D: row.to::<NestedRow>() (nested node deserialization)");
    println!("{}", "=".repeat(72));

    // Mix of whole nodes and scalar projections in the RETURN clause.
    // The column names must match the struct field names.
    let mut stream = graph
        .execute(query(
            "MATCH (t:Transcript)-[c:CONTAINS]->(s:Statement)<-[:SAYS]-(p:Person)
             MATCH (i:Interview)-[:HAS_TRANSCRIPT]->(t)
             OPTIONAL MATCH (i)-[:INTERVIEWED_BY]->(interviewer:Person)
               WHERE interviewer = p
             RETURN s, p,
                    interviewer IS NOT NULL AS is_interviewer
             ORDER BY c.startTime",
        ))
        .await?;

    while let Some(row) = stream.next().await? {
        let nr: NestedRow = row.to()?;
        println!(
            "  {} [{}{}] => words: {:?}",
            nr.s.uid,
            nr.p.name,
            if nr.is_interviewer {
                " (interviewer)"
            } else {
                ""
            },
            nr.s.words
                .as_ref()
                .map(|s| format!("{}...", &s[..20.min(s.len())]))
        );
    }

    // This is genuinely cool:
    //
    // + Whole nodes in the RETURN clause — structural information preserved.
    // + Single deserialization call for the entire row.
    // + Nested structs keep node data organized.
    // + #[serde(default)] on StatementNode.words still works — the node
    //   deserializer handles missing properties the same way.
    //
    // - Column names must match struct field names. RETURN `s` means the struct
    //   field must be named `s`, not `statement`. You can work around this with
    //   `RETURN statement AS s` or `#[serde(rename)]`, but it's a constraint.
    // - Relationships (like :CONTAINS with timing data) are awkward. BoltRelation
    //   has id, start_node_id, end_node_id, typ, and properties — it doesn't
    //   deserialize as cleanly as nodes because of the extra structural fields.

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Approach E: Extension Trait — Fallible get-or-default
// ═══════════════════════════════════════════════════════════════════════════════
//
// Instead of changing the query or adding intermediate structs, patch the
// behavior of Node::get itself with an extension trait. This wraps the
// NoSuchProperty error and converts it to the type's Default value.
//
// This is philosophically different from the other approaches: instead of
// working around the gap between "absent key" and "null value," it papers
// over it at the API boundary with a trait that says "if the key is missing,
// pretend it was the default."

trait NodeExt {
    /// Like `node.get()`, but returns `T::default()` when the property
    /// is absent from the node. Only suitable for types where Default is
    /// a meaningful "not present" sentinel (e.g., Option<T>, Vec<T>, String).
    fn get_or_default<'a, T>(&'a self, key: &str) -> anyhow::Result<T>
    where
        T: serde::Deserialize<'a> + Default;
}

impl NodeExt for Node {
    fn get_or_default<'a, T>(&'a self, key: &str) -> anyhow::Result<T>
    where
        T: serde::Deserialize<'a> + Default,
    {
        match self.get(key) {
            Ok(v) => Ok(v),
            Err(e) => {
                // neo4rs::DeError doesn't expose variant-level matching publicly,
                // so we match on the error's Display string. This is admittedly
                // fragile — but the alternative (reaching into neo4rs internals)
                // is worse. In production you'd want neo4rs to expose this variant
                // or provide a `get_opt()` method.
                let msg = e.to_string();
                if msg.contains("does not exist") {
                    // NoSuchProperty means the key wasn't in the map at all.
                    // For Option<T>, Default::default() is None — exactly what we want.
                    // For String, it's "" — probably fine as a sentinel.
                    // For i64, it's 0 — almost certainly wrong and dangerous.
                    //
                    // This is why the trait bound says Default but the *wisdom*
                    // of using it depends on the semantics of the type. The compiler
                    // can't enforce "Default means absent" — that's a human contract.
                    Ok(T::default())
                } else {
                    Err(anyhow::anyhow!(e))
                }
            }
        }
    }
}

async fn approach_e_extension_trait(graph: &Graph) -> Result<()> {
    println!("\n{}", "=".repeat(72));
    println!("APPROACH E: Extension Trait (get_or_default)");
    println!("{}", "=".repeat(72));

    let mut stream = graph
        .execute(query(
            "MATCH (t:Transcript)-[c:CONTAINS]->(s:Statement)<-[:SAYS]-(p:Person)
             RETURN s, p, c
             ORDER BY c.startTime",
        ))
        .await?;

    while let Some(row) = stream.next().await? {
        let s: Node = row.get("s")?;
        let uid: String = s.get("uid")?;
        let words: Option<String> = s.get_or_default("words")?; // NoSuchProperty → None

        let p: Node = row.get("p")?;
        let speaker: String = p.get("name")?;

        println!(
            "  {} [{}] => words: {}",
            uid,
            speaker,
            words.as_ref().map_or("None", |_| "Some(...)")
        );
    }

    // Trade-offs:
    //
    // + Minimal code change — just swap `get` for `get_or_default`.
    // + Keeps whole-node returns, no intermediate structs, no query changes.
    // + Explicit at the call site: you see `get_or_default` and know this
    //   field tolerates absence.
    //
    // - The Default bound is too permissive. `get_or_default::<i64>("count")`
    //   silently returns 0 for missing properties — a real value that could
    //   mask bugs. Only safe for types where Default genuinely means "absent."
    //
    // - Matching on the error string is fragile — if neo4rs changes the
    //   wording of NoSuchProperty, this breaks silently.
    //
    // - Mixes concerns: every call site decides individually whether absence
    //   is OK. With serde(default), the struct itself declares its defaults —
    //   the knowledge lives in one place.

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Approach F: Cypher Map Projection — Reshape the Node in the Query
// ═══════════════════════════════════════════════════════════════════════════════
//
// This is the weirdest Cypher-side approach. Instead of returning the raw node
// or individual properties, use Cypher's map projection syntax to reshape the
// node into a map *inside the query*. The projected map includes all properties
// we care about, and absent properties become null keys in the map (rather than
// absent keys).
//
// Syntax: `s { .uid, .text, .words }` produces a map like:
//   { uid: "st-001", text: "...", words: "[...]" }
// or, for a node without `words`:
//   { uid: "st-002", text: "...", words: null }
//
// The crucial difference from `RETURN s`: the raw node's property map omits
// keys for absent properties, but map projection *includes* them as null.

async fn approach_f_map_projection(graph: &Graph) -> Result<()> {
    println!("\n{}", "=".repeat(72));
    println!(
        "APPROACH F: Cypher Map Projection (s {{{{ .uid, .text, .words }}}})"
    );
    println!("{}", "=".repeat(72));

    let mut stream = graph
        .execute(query(
            "MATCH (t:Transcript)-[c:CONTAINS]->(s:Statement)<-[:SAYS]-(p:Person)
             RETURN s { .uid, .text, .words } AS statement,
                    p { .uid, .name } AS person,
                    c.startTime AS start_time
             ORDER BY c.startTime",
        ))
        .await?;

    while let Some(row) = stream.next().await? {
        // The projected map arrives as a BoltType::Map (not BoltType::Node!).
        // We can still deserialize it into a struct via row.get(), because
        // row.get() calls BoltType::to(), and BoltType::Map's Deserializer
        // uses serde's standard MapDeserializer over the map entries.
        //
        // Since the map projection *includes* the "words" key with a null
        // value (BoltType::Null), serde's Option deserializer handles it:
        //   BoltType::Null → deserialize_option → visitor.visit_none() → None
        //
        // No #[serde(default)] needed — the key IS present.

        // Deserialize the projected map directly into our struct:
        let sn: StatementNode = row.get("statement")?;
        let pn: PersonNode = row.get("person")?;
        let start: f64 = row.get("start_time")?;

        println!(
            "  {} [{}] @ {:.1}s => words: {:?}",
            sn.uid,
            pn.name,
            start,
            sn.words
                .as_ref()
                .map(|s| format!("{}...", &s[..20.min(s.len())]))
        );
    }

    // Trade-offs:
    //
    // + The null is *in the data*, not inferred. The BoltMap has the key.
    // + No #[serde(default)] needed — the key IS present (with null value).
    // + You choose which properties to project — acts as a query-level
    //   select list, like a SQL SELECT clause.
    // + Returns a BoltMap, not a BoltNode — no neo4j node ID, labels, or
    //   other metadata. Lighter weight over the wire.
    //
    // - The projected map is a BoltMap, not a BoltNode. You lose labels, IDs,
    //   and the ability to use neo4rs's node-specific deserialization features
    //   (like extracting Labels or Id newtypes).
    // - You must enumerate every property in the projection. If the node gains
    //   a new property later, you must update the query — it won't appear
    //   automatically like it would with `RETURN s`.
    //
    // This is the "SQL mindset" approach: treat the query as a SELECT that
    // defines its output shape, and let the struct match that shape.

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<()> {
    let uri =
        std::env::var("NEO4J_URI").unwrap_or_else(|_| "bolt://localhost:7687".to_string());
    let user =
        std::env::var("NEO4J_USERNAME").unwrap_or_else(|_| "neo4j".to_string());
    let pass =
        std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "auohpauohp".to_string());

    let graph = Graph::new(&uri, &user, &pass)
        .await
        .context("connecting to Neo4j")?;

    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════╗"
    );
    println!(
        "{}",
        "║  Exploring Neo4j Node Deserialization Strategies (neo4rs 0.8)       ║"
    );
    println!(
        "{}",
        "╠══════════════════════════════════════════════════════════════════════╣"
    );
    println!(
        "{}",
        "║  3 statements in the DB: st-001 (has words), st-002 (NO words),    ║"
    );
    println!(
        "{}",
        "║  st-003 (has words). Approach 0 should fail on st-002.             ║"
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════╝"
    );

    // Approach 0: Show the bug
    if let Err(e) = approach_0_the_bug(&graph).await {
        println!("  [Approach 0 errored out: {}]", e);
    }

    // Approach A: Cypher projection
    approach_a_cypher_projection(&graph).await?;

    // Approach A': COALESCE sentinel
    approach_a_prime_coalesce(&graph).await?;

    // Approach B: serde(default) intermediate struct
    approach_b_serde_default(&graph).await?;

    // Approach C: Row::to with flat struct
    approach_c_row_to(&graph).await?;

    // Approach D: Row::to with nested node deserialization
    approach_d_nested_row(&graph).await?;

    // Approach E: Extension trait
    approach_e_extension_trait(&graph).await?;

    // Approach F: Map projection
    approach_f_map_projection(&graph).await?;

    println!("\n{}", "=".repeat(72));
    println!("SUMMARY");
    println!("{}", "=".repeat(72));
    println!(
        "  0  node.get::<Option<String>>()     BROKEN  HashMap miss before serde runs"
    );
    println!(
        "  A  Cypher s.words AS words           Works   Cypher coerces absent -> null"
    );
    println!(
        "  A' COALESCE(s.words, '[]')           Works   sentinel substitution, no Option needed"
    );
    println!(
        "  B  node.to::<StatementNode>()        Works   #[serde(default)] on struct field"
    );
    println!(
        "  C  row.to::<FlatRow>()               Works   full-row flat deserialization"
    );
    println!(
        "  D  row.to::<NestedRow>() w/ nodes    Works   nested node structs in one shot"
    );
    println!(
        "  E  node.get_or_default()             Works   extension trait, NoSuchProperty -> Default"
    );
    println!(
        "  F  s {{{{ .uid, .text, .words }}}}       Works   map projection includes null keys"
    );

    Ok(())
}
