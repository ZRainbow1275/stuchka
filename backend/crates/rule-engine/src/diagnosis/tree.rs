//! M1 问诊树 (prd/04 §4.1.4) — a data-driven ≤7-question decision tree.
//!
//! The tree is authored in `data/diagnosis/decision_tree.yaml` and embedded at compile time. Each
//! node is either a [`Node::Question`] (a prompt + a set of answers, each pointing at the next node)
//! or a [`Node::Leaf`] (a terminal `LD-NN-NN` subcategory). Walking from the root and choosing one
//! answer per question deterministically narrows toward exactly one of the 85 subcategories —
//! no AI is involved (INV-01 / automation level A: AI 不参与).
//!
//! Invariants enforced by [`DecisionTree::validate`] (and `tests/diagnosis_tree.rs`):
//! 1. Every `answer.next` references an existing node.
//! 2. Every root→leaf path has depth ≤ 7 (question count; the root is question #1).
//! 3. Every reachable leaf carries a code present in the [`super::catalog::DiagnosisCatalog`].
//! 4. The graph is acyclic (the bounded-depth walk would otherwise diverge).
//! 5. All 85 subcategories are reachable (做能用 coverage, prd §4.1.5).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::diagnosis::catalog::DiagnosisCatalog;
use crate::error::RuleError;

/// The canonical decision-tree YAML bundled with the crate.
pub const DECISION_TREE_YAML: &str = include_str!("../../data/diagnosis/decision_tree.yaml");

/// Maximum questions on any root→leaf path (prd §4.1.4: 子类识别以问诊树推进, ≤ 7 问).
pub const MAX_QUESTIONS: usize = 7;

/// The node id of the tree root.
pub const ROOT_ID: &str = "root";

/// One answer to a question node: a stable machine value, a Chinese label, and the next node id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    /// Stable machine value (snake_case; the LLM maps free text onto this, INV-01-safe assist).
    pub value: String,
    /// Chinese label shown to the user.
    pub label_zh: String,
    /// The next node id this answer leads to.
    pub next: String,
}

/// A question node: a prompt plus the available answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    /// Chinese prompt — the system asks exactly this one item (prd §4.1.4: 每问只问一项).
    pub prompt_zh: String,
    /// The available answers.
    pub answers: Vec<Answer>,
}

/// A leaf node: a terminal `LD-NN-NN` subcategory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leaf {
    /// The terminal subcategory code.
    pub subcategory: String,
}

/// The raw YAML node shape: a map carrying exactly one of `question` / `leaf`. Modelled as a
/// struct (not an externally-tagged enum) because `serde_norway` reads YAML map-key style cleanly
/// this way (a single-key map for an externally-tagged enum is read as a YAML `!tag`, which the
/// authored YAML does not use). [`Node`] is the validated, exactly-one-of projection.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RawNode {
    #[serde(default)]
    question: Option<Question>,
    #[serde(default)]
    leaf: Option<Leaf>,
}

/// A decision-tree node — either a question or a leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// A question to ask.
    Question(Question),
    /// A terminal subcategory.
    Leaf(Leaf),
}

impl RawNode {
    /// Project the raw map onto the validated [`Node`], erroring unless exactly one slot is set.
    fn into_node(self, id: &str) -> Result<Node, RuleError> {
        match (self.question, self.leaf) {
            (Some(q), None) => Ok(Node::Question(q)),
            (None, Some(l)) => Ok(Node::Leaf(l)),
            (Some(_), Some(_)) => Err(RuleError::Schema(format!(
                "node {id} has both `question` and `leaf`"
            ))),
            (None, None) => Err(RuleError::Schema(format!(
                "node {id} has neither `question` nor `leaf`"
            ))),
        }
    }
}

/// The parsed decision tree: a node table indexed by id, rooted at [`ROOT_ID`].
#[derive(Debug, Clone)]
pub struct DecisionTree {
    nodes: BTreeMap<String, Node>,
}

/// The outcome of a single step through the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// More questioning required — here is the next question (with its node id).
    Ask { node_id: String, question: Question },
    /// Terminal — the walk reached a subcategory.
    Done { subcategory: String },
}

impl DecisionTree {
    /// Parse a decision-tree YAML and run the structural validation against `catalog`.
    pub fn parse(yaml: &str, catalog: &DiagnosisCatalog) -> Result<Self, RuleError> {
        let raw: BTreeMap<String, RawNode> =
            serde_norway::from_str(yaml).map_err(|e| RuleError::Schema(e.to_string()))?;
        let mut nodes = BTreeMap::new();
        for (id, raw_node) in raw {
            let node = raw_node.into_node(&id)?;
            nodes.insert(id, node);
        }
        let tree = Self { nodes };
        tree.validate(catalog)?;
        Ok(tree)
    }

    /// Load the bundled decision tree validated against the bundled catalog.
    pub fn bundled(catalog: &DiagnosisCatalog) -> Result<Self, RuleError> {
        Self::parse(DECISION_TREE_YAML, catalog)
    }

    /// Borrow a node by id.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// The root question (the tree always opens with a question).
    pub fn root_question(&self) -> Result<(&str, &Question), RuleError> {
        match self.nodes.get(ROOT_ID) {
            Some(Node::Question(q)) => Ok((ROOT_ID, q)),
            Some(Node::Leaf(_)) => Err(RuleError::Schema("root must be a question".into())),
            None => Err(RuleError::Schema("missing root node".into())),
        }
    }

    /// Resolve a `(node_id, answer_value)` pair into the next [`Step`]. Returns `None` when the node
    /// is unknown, is not a question, or the answer value is not offered (the caller surfaces this
    /// as a bad request, never a guess — C-C-6).
    pub fn step(&self, node_id: &str, answer_value: &str) -> Option<Step> {
        let q = match self.nodes.get(node_id)? {
            Node::Question(q) => q,
            Node::Leaf(_) => return None,
        };
        let answer = q.answers.iter().find(|a| a.value == answer_value)?;
        match self.nodes.get(&answer.next)? {
            Node::Question(next_q) => Some(Step::Ask {
                node_id: answer.next.clone(),
                question: next_q.clone(),
            }),
            Node::Leaf(l) => Some(Step::Done {
                subcategory: l.subcategory.clone(),
            }),
        }
    }

    /// Walk a full answer path from the root. `path` is the ordered list of answer values (one per
    /// question). Returns the terminal subcategory, or an error describing where the path broke
    /// (unknown answer / exceeded depth / ran out before a leaf). Never guesses (C-C-6).
    pub fn walk(&self, path: &[String]) -> Result<String, RuleError> {
        let mut current = ROOT_ID.to_string();
        for (depth, value) in path.iter().enumerate() {
            if depth >= MAX_QUESTIONS {
                return Err(RuleError::OutOfScope("问诊路径超过 7 问上限"));
            }
            match self.step(&current, value) {
                Some(Step::Done { subcategory }) => return Ok(subcategory),
                Some(Step::Ask { node_id, .. }) => current = node_id,
                None => {
                    return Err(RuleError::Schema(format!(
                        "无效问诊答案 `{value}`（节点 {current}）"
                    )))
                }
            }
        }
        Err(RuleError::OutOfScope("问诊未完成：仍需继续作答以确定子类"))
    }

    /// Validate the structural invariants (1-5). Called at load time.
    pub fn validate(&self, catalog: &DiagnosisCatalog) -> Result<(), RuleError> {
        // Root must exist and be a question.
        self.root_question()?;

        // 1. Every answer.next references an existing node.
        for (id, node) in &self.nodes {
            if let Node::Question(q) = node {
                if q.answers.is_empty() {
                    return Err(RuleError::Schema(format!("question {id} has no answers")));
                }
                for a in &q.answers {
                    if !self.nodes.contains_key(&a.next) {
                        return Err(RuleError::Schema(format!(
                            "answer `{}` in {id} points at missing node {}",
                            a.value, a.next
                        )));
                    }
                }
            }
            // 3. Every leaf code is in the catalog.
            if let Node::Leaf(l) = node {
                if catalog.get(&l.subcategory).is_none() {
                    return Err(RuleError::Schema(format!(
                        "leaf {id} references unknown subcategory {}",
                        l.subcategory
                    )));
                }
            }
        }

        // 2 + 4. Bounded-depth DFS from the root: every path reaches a leaf within MAX_QUESTIONS,
        // and the bound itself rules out cycles (a cycle would exceed the depth budget).
        let mut reachable_leaves: BTreeSet<String> = BTreeSet::new();
        self.dfs_depth(ROOT_ID, 1, &mut reachable_leaves)?;

        // 5. All 85 subcategories reachable.
        let mut missing: Vec<String> = Vec::new();
        for e in catalog.entries() {
            if !reachable_leaves.contains(&e.code) {
                missing.push(e.code.clone());
            }
        }
        if !missing.is_empty() {
            return Err(RuleError::Schema(format!(
                "decision tree does not reach {} subcategories: {:?}",
                missing.len(),
                missing
            )));
        }
        Ok(())
    }

    /// DFS asserting depth ≤ MAX_QUESTIONS; accumulates reachable leaf codes.
    fn dfs_depth(
        &self,
        node_id: &str,
        depth: usize,
        leaves: &mut BTreeSet<String>,
    ) -> Result<(), RuleError> {
        match self.nodes.get(node_id) {
            Some(Node::Leaf(l)) => {
                leaves.insert(l.subcategory.clone());
                Ok(())
            }
            Some(Node::Question(q)) => {
                if depth > MAX_QUESTIONS {
                    return Err(RuleError::Schema(format!(
                        "path through {node_id} exceeds {MAX_QUESTIONS} questions (cycle or too deep)"
                    )));
                }
                for a in &q.answers {
                    self.dfs_depth(&a.next, depth + 1, leaves)?;
                }
                Ok(())
            }
            None => Err(RuleError::Schema(format!("dangling node id {node_id}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> (DiagnosisCatalog, DecisionTree) {
        let cat = DiagnosisCatalog::bundled().unwrap();
        let tree = DecisionTree::bundled(&cat).unwrap();
        (cat, tree)
    }

    #[test]
    fn bundled_tree_loads_and_validates() {
        let _ = tree();
    }

    #[test]
    fn root_is_a_question_with_20_branches() {
        let (_, t) = tree();
        let (_, q) = t.root_question().unwrap();
        assert_eq!(q.answers.len(), 20, "root branches to 20 categories");
    }

    #[test]
    fn walk_reaches_expected_subcategory() {
        let (_, t) = tree();
        // contract_formation -> no_written = LD-01-01.
        let sub = t
            .walk(&["contract_formation".into(), "no_written".into()])
            .unwrap();
        assert_eq!(sub, "LD-01-01");
        // work_injury -> occupational -> confirmed = LD-05-06 (the deepest, 3-question path).
        let sub = t
            .walk(&[
                "work_injury".into(),
                "occupational".into(),
                "confirmed".into(),
            ])
            .unwrap();
        assert_eq!(sub, "LD-05-06");
    }

    #[test]
    fn unknown_answer_is_an_error_not_a_guess() {
        let (_, t) = tree();
        let err = t.walk(&["contract_formation".into(), "no_such_answer".into()]);
        assert!(err.is_err());
    }

    #[test]
    fn incomplete_path_is_out_of_scope() {
        let (_, t) = tree();
        // Stop after the first question on a path that needs more.
        let err = t.walk(&["work_injury".into(), "occupational".into()]);
        assert!(matches!(err, Err(RuleError::OutOfScope(_))));
    }

    #[test]
    fn step_offers_the_next_question() {
        let (_, t) = tree();
        let step = t.step(ROOT_ID, "work_injury").unwrap();
        match step {
            Step::Ask { node_id, .. } => assert_eq!(node_id, "q_ld05"),
            Step::Done { .. } => panic!("work_injury should ask another question"),
        }
    }
}
