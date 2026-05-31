trait ErrorStrategy {
    type SubError;
    type Error;

    fn into_error(suberr: Self::SubError) -> RetryFlow<Self::Error>
}
