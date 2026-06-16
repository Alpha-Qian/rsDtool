use crate::base::pwriter::BufWriter;

struct FileWriter {}

impl BufWriter for FileWriter {
    type Error = ();

    fn pwrite_raw<S>(
        &self,
        pos: u64,
        buffer: S,
    ) -> impl Future<Output = (Result<(), Self::Error>, S)>
    where
        S: std::ops::Deref<Target = [u8]> + 'static,
    {
        todo!()
    }
}
