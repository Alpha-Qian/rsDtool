///所有权形式的Guard和State

struct GroupGuard {}

struct ReporterGuard {}

struct BusyGroup {}

struct IdleGroup {}

struct BusyReporter {}

struct IdleReporter {}

impl BusyGroup {
    fn into_idle(self) -> IdleGroup {
        todo!()
    }
}
