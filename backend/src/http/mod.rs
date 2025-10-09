mod health;
mod user;
mod zuul;
mod util;

pub use health::health_router;
pub use zuul::zuul_router;
pub use util::util_router;
pub use user::{
    user_router,
    api_user_router,
};
