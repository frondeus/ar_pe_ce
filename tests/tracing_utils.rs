use once_cell::sync::Lazy;

static TRACING: Lazy<()> = Lazy::new(|| {
    tracing_subscriber::fmt().pretty().compact().init();
});

pub fn init_tracing() {
    Lazy::force(&TRACING);
}
