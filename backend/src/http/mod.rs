mod health;
mod user;
mod shuul;
mod util;
mod record;
mod rule;
mod ignored;

pub use health::health_router;
pub use shuul::shuul_router;
pub use util::util_router;
pub use record::record_router;
pub use ignored::ignored_router;
pub use rule::rule_router;
pub use user::{
    user_router,
    api_user_router,
};
