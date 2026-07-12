use tokio::time::Instant;

trait Creater {
    fn add_one();

    fn sub_lazy();

    fn sub_now();
}

trait Strategy {
    type Envioment = Instant;
    fn run_step(env: Self::Envioment);
}

///将线程创建器函数包装成控制器函数
fn strategy_builder(creater: impl FnMut()) -> impl FnMut //Control
{
}
