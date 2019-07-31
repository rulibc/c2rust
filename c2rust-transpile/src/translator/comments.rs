use std::collections::{HashMap, HashSet};
use syntax::source_map::{DUMMY_SP, Span};
use crate::c_ast::{CDeclId, CDeclKind, CommentContext, TypedAstContext};
use crate::c_ast::iterators::{NodeVisitor, SomeId};
use crate::rust_ast::pos_to_span;
use crate::rust_ast::comment_store::CommentStore;
use super::Translation;

struct CommentLocator<'c> {
    ast_context: &'c TypedAstContext,
    comment_context: &'c CommentContext,
    comment_store: &'c mut CommentStore,
    spans: HashMap<SomeId, Span>,
    top_decls: &'c HashSet<CDeclId>,
}

impl<'c> NodeVisitor for CommentLocator<'c> {
    fn pre(&mut self, id: SomeId) -> bool {
        // Don't traverse into unvisited top-level decls, we should visit those
        // in sorted order.
        if let SomeId::Decl(id) = id {
            if self.top_decls.contains(&id) {
                return false;
            }
        }
        if let Some(loc) = self.ast_context.get_src_loc(id) {
            let comments = self.comment_context
                .get_comments_before(loc.begin(), &self.ast_context);
            if let Some(pos) = self.comment_store
                .add_comments(&comments)
            {
                debug!("Attaching comments {:?} to id {:?} at pos {:?}", comments, id, pos);
                let span = pos_to_span(pos);
                self.spans.insert(id, span);
            }
        }
        // Don't traverse into macro object replacement expressions, as they are
        // in other places.
        if let SomeId::Decl(id) = id {
            if let CDeclKind::MacroObject{..} = self.ast_context[id].kind {
                return false;
            }
        }
        true
    }

    fn post(&mut self, id: SomeId) {
        if let Some(loc) = self.ast_context.get_src_loc(id) {
            let comments = self.comment_context
                .get_comments_before(loc.end(), &self.ast_context);
            if let Some(pos) = self.comment_store
                .add_comments(&comments)
            {
                debug!("Attaching comments {:?} to id {:?} at pos {:?}", comments, id, pos);
                let span = self.spans.entry(id)
                    .or_insert(DUMMY_SP);
                *span = span.with_hi(pos);
            }
        }
    }
}

impl<'c> Translation<'c> {
    /// Create spans for each C AST node that has a comment attached to it.
    pub fn locate_comments(&mut self) {
        let mut top_decls: HashSet<CDeclId> = self.ast_context
            .c_decls_top
            .iter()
            .copied()
            .collect();
        for decl_id in &self.ast_context.c_decls_top {
            top_decls.remove(decl_id);
            let mut visitor = CommentLocator {
                ast_context: &self.ast_context,
                comment_context: &self.comment_context,
                comment_store: &mut *self.comment_store.borrow_mut(),
                spans: HashMap::new(),
                top_decls: &top_decls,
            };
            visitor.visit_tree(&self.ast_context, SomeId::Decl(*decl_id));
            let CommentLocator {spans, ..} = visitor;
            self.spans.extend(spans);
        }
    }

    pub fn get_span(&self, id: SomeId) -> Option<Span> {
        self.spans.get(&id).copied()
    }
}
