use chrono::Timelike;

pub struct OffHoursCache {
    cached_off_hours: Option<chrono::DateTime<chrono::Utc>>,
}

impl OffHoursCache {
    pub fn new() -> Self {
        Self {
            cached_off_hours: None,
        }
    }

    pub fn set(&mut self) {
        let now: chrono::DateTime<chrono::Utc> = chrono::Utc::now();
        // CST [23:20, 06:20)
        let in_possible_off_hours = (now.hour() > 15 && now.hour() < 22)
            || (now.hour() == 15 && now.minute() >= 20)
            || (now.hour() == 22 && now.minute() <= 20);
        if in_possible_off_hours {
            self.cached_off_hours = Some(now.with_hour(23).unwrap().with_minute(5).unwrap());
        }
    }

    pub fn clear(&mut self) {
        self.cached_off_hours = None;
    }

    pub fn expiration(&self) -> std::time::Duration {
        if let Some(off_hours) = self.cached_off_hours {
            let now: chrono::DateTime<chrono::Utc> = chrono::Utc::now();
            if now > off_hours {
                return std::time::Duration::ZERO;
            } else {
                return (off_hours - now).to_std().unwrap();
            }
        }
        std::time::Duration::ZERO
    }
}
