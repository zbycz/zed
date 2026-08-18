use git::repository::LogSource;
use gpui::{
    App, AsyncWindowContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Pixels,
    Subscription, WeakEntity, Window, actions, px,
};
use project::git_store::{GitStore, GitStoreEvent, RepositoryId};
use ui::prelude::*;
use workspace::{
    Workspace,
    dock::{DockPosition, Panel, PanelEvent},
};

use crate::git_graph::GitGraph;

actions!(
    git_graph,
    [
        /// Toggles focus on the git graph panel.
        ToggleFocus
    ]
);

const GIT_GRAPH_PANEL_KEY: &str = "GitGraphPanel";

pub fn register(workspace: &mut Workspace) {
    workspace.register_action(|workspace, _: &ToggleFocus, window, cx| {
        workspace.toggle_panel_focus::<GitGraphPanel>(window, cx);
    });
}

/// Hosts the [`GitGraph`] in a dock so that the log can span the full width of
/// the window instead of competing with editors for space in the center pane.
pub struct GitGraphPanel {
    workspace: WeakEntity<Workspace>,
    git_store: Entity<GitStore>,
    graph: Option<Entity<GitGraph>>,
    focus_handle: FocusHandle,
    position: DockPosition,
    _subscriptions: Vec<Subscription>,
}

impl GitGraphPanel {
    pub async fn load(
        workspace: WeakEntity<Workspace>,
        mut cx: AsyncWindowContext,
    ) -> anyhow::Result<Entity<Self>> {
        workspace.update_in(&mut cx, |workspace, _window, cx| {
            let git_store = workspace.project().read(cx).git_store().clone();
            let workspace_handle = workspace.weak_handle();
            cx.new(|cx| GitGraphPanel::new(workspace_handle, git_store, cx))
        })
    }

    fn new(
        workspace: WeakEntity<Workspace>,
        git_store: Entity<GitStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe(&git_store, |this, _, event, cx| {
            if let GitStoreEvent::ActiveRepositoryChanged(Some(repo_id)) = event
                && this
                    .graph
                    .as_ref()
                    .is_some_and(|graph| graph.read(cx).repo_id() != *repo_id)
            {
                this.graph = None;
                cx.notify();
            }
        });

        Self {
            workspace,
            git_store,
            graph: None,
            focus_handle: cx.focus_handle(),
            position: DockPosition::Bottom,
            _subscriptions: vec![subscription],
        }
    }

    /// Shows `log_source` for `repo_id`, reusing the existing graph when it
    /// already displays the same log.
    pub fn show_graph(
        &mut self,
        repo_id: RepositoryId,
        log_source: LogSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<GitGraph> {
        if let Some(graph) = self.graph.as_ref()
            && graph.read(cx).repo_id() == repo_id
            && *graph.read(cx).log_source() == log_source
        {
            return graph.clone();
        }

        let git_store = self.git_store.clone();
        let workspace = self.workspace.clone();
        let graph =
            cx.new(|cx| GitGraph::new(repo_id, git_store, workspace, Some(log_source), window, cx));
        self.graph = Some(graph.clone());
        cx.notify();
        graph
    }

    fn active_repo_id(&self, cx: &App) -> Option<RepositoryId> {
        Some(self.git_store.read(cx).active_repository()?.read(cx).id)
    }

    /// Makes sure a graph is present, defaulting to the whole history of the
    /// active repository.
    fn ensure_graph(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.graph.is_some() {
            return;
        }
        let Some(repo_id) = self.active_repo_id(cx) else {
            return;
        };
        self.show_graph(repo_id, LogSource::All, window, cx);
    }
}

impl Focusable for GitGraphPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<PanelEvent> for GitGraphPanel {}

impl Render for GitGraphPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_graph(window, cx);

        v_flex()
            .key_context("GitGraphPanel")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(cx.theme().colors().editor_background)
            .map(|this| match self.graph.as_ref() {
                Some(graph) => this.child(graph.clone()),
                None => this.justify_center().items_center().child(
                    Label::new("No repository found")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                ),
            })
    }
}

impl Panel for GitGraphPanel {
    fn persistent_name() -> &'static str {
        "GitGraphPanel"
    }

    fn panel_key() -> &'static str {
        GIT_GRAPH_PANEL_KEY
    }

    fn activation_focus_handle(&self, cx: &App) -> FocusHandle {
        self.graph
            .as_ref()
            .map_or_else(|| self.focus_handle.clone(), |graph| graph.focus_handle(cx))
    }

    fn position(&self, _: &Window, _: &App) -> DockPosition {
        self.position
    }

    fn position_is_valid(&self, _: DockPosition) -> bool {
        true
    }

    fn set_position(&mut self, position: DockPosition, _: &mut Window, cx: &mut Context<Self>) {
        self.position = position;
        cx.notify();
    }

    fn default_size(&self, _: &Window, _: &App) -> Pixels {
        match self.position {
            DockPosition::Left | DockPosition::Right => px(480.),
            DockPosition::Bottom => px(360.),
        }
    }

    fn icon(&self, _: &Window, _: &App) -> Option<IconName> {
        Some(IconName::GitGraph)
    }

    fn icon_tooltip(&self, _: &Window, _: &App) -> Option<&'static str> {
        Some("Git Graph")
    }

    fn toggle_action(&self) -> Box<dyn gpui::Action> {
        Box::new(ToggleFocus)
    }

    fn activation_priority(&self) -> u32 {
        6
    }
}
