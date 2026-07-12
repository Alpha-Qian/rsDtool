//!定义所有会强制中止组下载的错误

use crate::base::pwriter::BufWriter;

enum GroupError<W: BufWriter> {
    Writer,
    NetWork,
}

