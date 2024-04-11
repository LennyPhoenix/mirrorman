pub trait WalkdirResultExtension<T> {
    fn handle_to_string(self) -> Result<T, String>;
}

impl<T> WalkdirResultExtension<T> for walkdir::Result<T> {
    fn handle_to_string(self) -> Result<T, String> {
        self.map_err(|e| {
            let depth = e.depth();

            let start = match e.path() {
                None => format!("Traversal aborted at depth {depth}"),
                Some(path) => {
                    format!("Traversal aborted at `{0}` (depth {depth})", path.display())
                }
            };

            match e.io_error() {
                Some(io_error) => format!("{start}: {io_error}"),
                None => format!("{start}: unknown error"),
            }
        })
    }
}
