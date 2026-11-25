//! Runtime calls that queue nonblocking updates.
//!
//! Statements of [`IrContainerStmt`] bracket a nonblocking write to say what
//! happens to the updates it queues: they are bound to their issuer
//! (`NbaBindOpen`), built into a hold (`NbaHoldOpen`) or find their item again
//! at commit (`NbaLocate`). A bracket holds for the updates of the write and
//! for none of its operands: a function an operand calls may queue updates of
//! its own, end its process or stop the run. The operands are therefore
//! values before a bracket reaches the C source, and it encloses only the one
//! call that queues the update.
use super::*;

/// The brackets the statements being emitted are in.
#[derive(Default)]
pub(super) struct UpdateMarks {
    pub(super) bound: bool,
    /// The hold, a C value.
    held: Option<String>,
    located: Option<LocatedUpdate>,
}

/// The request for the item of a located write and the temporaries it names.
struct LocatedUpdate {
    request: String,
    values: Vec<Value>,
}

impl UpdateMarks {
    /// Whether the statements being emitted are in a bracket.
    pub(super) fn any(&self) -> bool {
        self.bound || self.name_locals()
    }

    /// Whether a bracket names temporaries or locals of this C function.
    pub(super) fn name_locals(&self) -> bool {
        self.held.is_some() || self.located.is_some()
    }
}

impl Frame<'_, '_> {
    /// Emit `operation` when it opens or closes a bracket of updates.
    pub(super) fn update_mark(&mut self, operation: &IrContainerStmt) -> Result<bool, String> {
        match operation {
            IrContainerStmt::NbaBindOpen => self.updates.bound = true,
            IrContainerStmt::NbaBindClose => self.updates.bound = false,
            IrContainerStmt::NbaHoldOpen(held) => {
                let held = self.chandle(held)?;
                self.updates.held = Some(self.scalar("void*", held));
            }
            IrContainerStmt::NbaHoldClose => self.updates.held = None,
            IrContainerStmt::NbaLocate {
                root,
                items,
                selects,
            } => {
                let mut values = Vec::new();
                let request = containers::locate_request(self, root, items, selects, &mut values)?;
                self.updates.located = Some(LocatedUpdate { request, values });
            }
            IrContainerStmt::NbaLocateEnd => {
                if let Some(located) = self.updates.located.take() {
                    for value in located.values {
                        self.discard(value);
                    }
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Open the brackets around a call that queues updates.
    pub(super) fn open_updates(&mut self) {
        if self.updates.bound {
            self.line("llg_nba_bind_begin();");
        }
        if let Some(held) = &self.updates.held {
            let open = format!("llg_nba_hold_open({held});");
            self.line(open);
        }
        if let Some(located) = &self.updates.located {
            let request = located.request.clone();
            self.line(request);
        }
    }

    /// Close the brackets [`Self::open_updates`] opened.
    pub(super) fn close_updates(&mut self) {
        if self.updates.located.is_some() {
            self.line("llg_nba_locate_end();");
        }
        if self.updates.held.is_some() {
            self.line("llg_nba_hold_close();");
        }
        if self.updates.bound {
            self.line("llg_nba_bind_end();");
        }
    }

    /// Emit `call`, which queues updates and evaluates nothing.
    pub(super) fn queue_update(&mut self, call: String) {
        self.open_updates();
        self.line(call);
        self.close_updates();
    }

    /// Emit `call`, which stores, or with `nba` queues updates, and evaluates
    /// nothing.
    pub(super) fn store_or_queue(&mut self, nba: bool, call: String) {
        if nba {
            self.queue_update(call);
        } else {
            self.line(call);
        }
    }
}
