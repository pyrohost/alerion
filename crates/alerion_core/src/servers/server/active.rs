#[derive(Debug, Clone, Copy)]
pub enum State {
    Installing,
    Installed,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Egg;

#[derive(Debug)]
pub struct ActiveServer {
    egg: Egg,
    state: State,
}

impl ActiveServer {
    pub fn with_egg(egg: Egg) -> ActiveServer {
        let server = ActiveServer {
            egg,
            state: State::Installing,
        };

        server
    }

    /// Begins a reinstallation task.
    pub fn reinstall(&self) {}

    /// Forces an uninstallation task.
    pub fn force_uninstall(&self) {}
}
