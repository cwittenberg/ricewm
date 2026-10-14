//! Clients operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    /// Return one client top-level which completed the compositor's managed,
    /// viewable MapWindow transaction.
    ///
    /// Raw opcode counts are insufficient for launch readiness: GTK maps
    /// input-only helper children before it maps its real top-level. Exposing
    /// this post-condition keeps the browser authority tied to visible X11
    /// state rather than an arbitrary opcode-8 request.
    pub fn take_presented_managed_window(&mut self) -> Option<ResourceId> {
        self.presented_managed_windows.pop_front()
    }
}
