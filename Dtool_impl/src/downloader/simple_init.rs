use crate::base::request_info::RequestInfo;

struct RawIniter<F> {
    init_type: InitType<F>,
    info: RequestInfo,
}

enum InitType<F> {
    Segment { process: u64 },
    Task(F),
}
