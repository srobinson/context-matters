use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr},
};

use cm_web::{ServeOptions, serve};

fn assert_serve_signature<F>(_: fn(ServeOptions) -> F)
where
    F: Future<Output = anyhow::Result<()>>,
{
}

#[test]
fn serve_entrypoint_is_public_with_cli_options() {
    let opts = ServeOptions {
        open: false,
        port: Some(3141),
        host: Some(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
    };

    assert!(!opts.open);
    assert_eq!(opts.port, Some(3141));
    assert_eq!(opts.host, Some(IpAddr::V4(Ipv4Addr::UNSPECIFIED)));
    assert_serve_signature(serve);
}
