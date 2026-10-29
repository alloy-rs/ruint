/// The sign of an [`Int`](crate::Int).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sign {
    /// Less than zero.
    Negative,
    /// Greater than or equal to zero.
    Positive,
}
