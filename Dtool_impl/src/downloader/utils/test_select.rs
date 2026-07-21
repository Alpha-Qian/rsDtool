fn test() {
    let f1 = async { 1 };

    let f2 = async { 2 };

    let r = futures::future::select(f1, f2)
}
