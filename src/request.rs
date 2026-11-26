#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Pending,
    Accepted,
    Done,
    Rejected,
    Cancelled,
    Expired,
}

impl Status {
    pub const ALL: [Self; 6] = [
        Self::Pending,
        Self::Accepted,
        Self::Done,
        Self::Rejected,
        Self::Cancelled,
        Self::Expired,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Done => "done",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "pending" => Some(Self::Pending),
            "accepted" => Some(Self::Accepted),
            "done" => Some(Self::Done),
            "rejected" => Some(Self::Rejected),
            "cancelled" => Some(Self::Cancelled),
            "expired" => Some(Self::Expired),
            _ => None,
        }
    }
}

pub fn transition(from: Status, to: Status) -> Option<Status> {
    use Status::*;
    match (from, to) {
        (Pending, Accepted | Rejected | Cancelled | Expired) | (Accepted, Done | Cancelled) => {
            Some(to)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Status::*, transition};

    #[test]
    fn only_documented_transitions_are_valid() {
        let states = [Pending, Accepted, Done, Rejected, Cancelled, Expired];
        let allowed = [
            (Pending, Accepted),
            (Pending, Rejected),
            (Pending, Cancelled),
            (Pending, Expired),
            (Accepted, Done),
            (Accepted, Cancelled),
        ];
        for from in states {
            for to in states {
                assert_eq!(
                    transition(from, to),
                    allowed.contains(&(from, to)).then_some(to)
                );
            }
        }
    }
}
