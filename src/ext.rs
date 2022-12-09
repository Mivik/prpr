use ordered_float::{Float, NotNan};

pub trait NotNanExt: Sized {
    fn not_nan(self) -> NotNan<Self>;
}

impl<T: Sized + Float> NotNanExt for T {
    fn not_nan(self) -> NotNan<Self> {
        NotNan::new(self).unwrap()
    }
}
