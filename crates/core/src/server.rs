use tokio::net::TcpListener;

pub struct ServerInner {
    pub listener: TcpListener,
    //pub methods: Vec<Box<dyn Rpc>>
}

impl ServerInner {
    pub async fn new<A: tokio::net::ToSocketAddrs>(addr: A) -> anyhow::Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(addr).await?,
        })
    }

    pub fn port(&self) -> anyhow::Result<u16> {
        Ok(self.listener.local_addr()?.port())
    }
}
