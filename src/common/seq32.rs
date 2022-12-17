#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Seq32(u32);

impl std::ops::Deref for Seq32 {
    type Target = u32;
    fn deref<'a>(&'a self) -> &'a u32 {
        &self.0
    }
}

impl From<u32> for Seq32 {
    fn from(seq: u32) -> Self {
        Seq32(seq)
    }
}

impl PartialOrd for Seq32 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Seq32 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.0.eq(other) {
            std::cmp::Ordering::Equal
        } else if self.0.wrapping_sub(other.0) < 0x80000000 {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Less
        }
    }
}

impl std::ops::Add for Seq32 {
    type Output = Self;

    fn add(self, other: Seq32) -> Self {
        Seq32::from(self.0.wrapping_add(*other))
    }
}

impl std::ops::Add<u32> for Seq32 {
    type Output = Self;

    fn add(self, other: u32) -> Self {
        Seq32::from(self.0.wrapping_add(other))
    }
}

impl std::ops::Sub for Seq32 {
    type Output = Self;

    fn sub(self, other: Seq32) -> Self {
        Seq32::from(self.0.wrapping_sub(*other))
    }
}

impl std::ops::Sub<u32> for Seq32 {
    type Output = Self;

    fn sub(self, other: u32) -> Self {
        Seq32::from(self.0.wrapping_sub(other))
    }
}

impl std::ops::AddAssign for Seq32 {
    fn add_assign(&mut self, other: Seq32) {
        self.0 = self.0.wrapping_add(*other);
    }
}

impl std::ops::AddAssign<u32> for Seq32 {
    fn add_assign(&mut self, other: u32) {
        self.0 = self.0.wrapping_add(other);
    }
}

impl std::ops::SubAssign for Seq32 {
    fn sub_assign(&mut self, other: Seq32) {
        self.0 = self.0.wrapping_sub(*other);
    }
}

impl std::ops::SubAssign<u32> for Seq32 {
    fn sub_assign(&mut self, other: u32) {
        self.0 = self.0.wrapping_sub(other);
    }
}
