//! The builder thread's side of live playback: building the graphs
//! the render thread plays.
//!
//! Not real-time code: it reads the project, loads media and allocates.
//!
//! - **Play and record** build a [`fresh`](GraphBuilder::fresh) graph, reset like an offline
//!   render's, because [`RenderGraph::reset`] prepares every processor again, which allocates.
//!   It takes over nothing from the graph it replaces.
//! - **An edit during playback** builds a [`rebuild`](GraphBuilder::rebuild): it plans against
//!   the last graph built, so the render thread can hand the old graph's state to the new one
//!   ([`RenderGraph::adopt`]).
//! - **Setup requests** of the render thread's processors are answered with
//!   [`SetupTicket::prepare`](crate::SetupTicket::prepare).
//! - **Clip sources** of a graph the player plays go to its prefetch thread: the builder keys them, so a rebuild that plays the same clips keeps the audio
//!   already read ahead.
//!
//! Every graph a builder returns must reach the render thread, in build order, through one
//! `sonara_rt::handoff` channel of [`Update`](crate::Update)s: a rebuild is planned against the
//! graph built before it, and the render thread rejects a graph whose predecessor it does not
//! play (`Applied::Rejected`). After a rejection or a lost graph, build a fresh one.

use std::num::NonZeroUsize;

use sonara_core::model::project::Project;
use sonara_core::plugins::PluginRegistry;
use sonara_media::MediaProvider;

use crate::EngineError;
use crate::clips::ClipsSignature;
use crate::graph::{ClipSources, GraphId, GraphLayout, LiveBuild, MediaKey, RenderGraph};

/// Builds the live graphs of one player on its builder thread, and keeps the layout of the
/// last one for planning the next rebuild and for mapping meters and reports to IDs.
#[derive(Debug)]
pub struct GraphBuilder {
    block_size: NonZeroUsize,
    last: Option<GraphLayout>,
    /// The key and signature of the clip sources keyed last ([`GraphBuilder::key_media`]).
    media: Option<(MediaKey, Vec<(String, ClipsSignature)>)>,
    /// The number of the next media key.
    next_media: u64,
}

impl GraphBuilder {
    /// A builder for blocks of `block_size` frames (the player's 512).
    #[must_use]
    pub fn new(block_size: NonZeroUsize) -> Self {
        Self {
            block_size,
            last: None,
            media: None,
            next_media: 1,
        }
    }

    /// A fresh live graph of `project`, reset: for play and record (the prototype's
    /// `graph_for` followed by `reset`). Its processors are new; it takes over nothing.
    ///
    /// # Errors
    ///
    /// The errors of [`RenderGraph::new`]; the builder then keeps its last layout.
    pub fn fresh(
        &mut self,
        project: &Project,
        registry: &PluginRegistry,
        media: &MediaProvider,
    ) -> Result<Box<RenderGraph>, EngineError> {
        let mut graph = self.build(project, registry, media, false)?;
        graph.reset();
        Ok(graph)
    }

    /// A live graph of the edited `project` that takes over the state of the last graph this
    /// builder built, once the render thread adopts it ([`RenderGraph::adopt`]): the
    /// prototype's rebuild with `previous`. Without a last graph, or at another sample rate,
    /// it takes over nothing and is not reset.
    ///
    /// # Errors
    ///
    /// The errors of [`RenderGraph::new`]; the builder then keeps its last layout.
    pub fn rebuild(
        &mut self,
        project: &Project,
        registry: &PluginRegistry,
        media: &MediaProvider,
    ) -> Result<Box<RenderGraph>, EngineError> {
        self.build(project, registry, media, true)
    }

    /// Takes the clip sources out of `graph`, a live graph of this builder, and keys them
    /// ([`RenderGraph::take_clip_sources`]): the graph plays the clips of its key from now on.
    /// Returns the sources with their new key when they differ from the sources keyed last;
    /// `None` when the graph plays the same clips, so it gets the last key and the audio read
    /// ahead for it stays valid. The new sources are then dropped here.
    pub(crate) fn key_media(&mut self, graph: &mut RenderGraph) -> Option<(MediaKey, ClipSources)> {
        let sources = graph.take_clip_sources();
        let signature = sources.signature();
        if let Some((key, last)) = &self.media
            && *last == signature
        {
            graph.set_media_key(*key);
            return None;
        }
        let key = MediaKey::new(self.next_media);
        self.next_media = self.next_media.wrapping_add(1);
        self.media = Some((key, signature));
        graph.set_media_key(key);
        Some((key, sources))
    }

    /// The layout of the last graph built: the IDs behind its node and device indices.
    #[must_use]
    pub fn layout(&self) -> Option<&GraphLayout> {
        self.last.as_ref()
    }

    fn build(
        &mut self,
        project: &Project,
        registry: &PluginRegistry,
        media: &MediaProvider,
        reuse: bool,
    ) -> Result<Box<RenderGraph>, EngineError> {
        let live = LiveBuild {
            id: GraphId::next(),
            previous: self.last.as_ref().filter(|_| reuse),
        };
        let (graph, layout) =
            RenderGraph::new_live(project, registry, media, self.block_size, live)?;
        self.last = Some(layout);
        Ok(Box::new(graph))
    }
}
