use task_compat::{poll_01_to_03, poll_03_to_01, with_context, with_notify};

#[tokio::test]
async fn notify() {
    use futures01::{sync::oneshot, Future};
    use futures03::future::poll_fn;

    let (tx, mut rx) = oneshot::channel();

    let jh =
        tokio::spawn(
            async move { poll_fn(|cx| poll_01_to_03(with_notify(cx, || rx.poll()))).await },
        );

    tx.send(42).unwrap();
    let out = jh.await.unwrap().unwrap();
    assert_eq!(out, 42);
}

#[tokio::test]
async fn context() {
    use futures01::future::poll_fn;
    use futures03::compat::Compat01As03;
    use std::{future::Future, pin::Pin};
    use tokio::sync::oneshot;

    let (tx, mut rx) = oneshot::channel();

    let jh = tokio::spawn(async move {
        let fut = poll_fn(|| {
            let poll = with_context(|cx| Pin::new(&mut rx).poll(cx));
            poll_03_to_01(poll)
        });
        Compat01As03::new(fut).await
    });

    tx.send(42).unwrap();

    let out = jh.await.unwrap().unwrap();
    assert_eq!(out, 42);
}
