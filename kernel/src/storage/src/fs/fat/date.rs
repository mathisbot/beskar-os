/// A DOS date.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Date {
    /// Year number.
    /// Valid range is [1980, 2107].
    year: u16,
    /// Month of the year.
    /// Valid range is [1, 12].
    month: u8,
    /// Day of the month.
    /// Valid range is [1, 31] but it depends on the month
    /// and year (leap year).
    day: u8,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) struct DosDate {
    dos_date: u16,
}

impl DosDate {
    #[must_use]
    #[inline]
    pub const fn new(dos_date: u16) -> Self {
        Self { dos_date }
    }

    #[must_use]
    #[inline]
    pub const fn dos_date(&self) -> u16 {
        self.dos_date
    }
}

impl Date {
    const MIN_YEAR: u16 = 1980;
    const MAX_YEAR: u16 = 2107;

    /// Creates a new `Date` instance.
    ///
    /// # Panics
    ///
    /// Panics if one of provided arguments is out of the supported range.
    #[must_use]
    pub fn new(year: u16, month: u8, day: u8) -> Self {
        assert!(
            (Self::MIN_YEAR..=Self::MAX_YEAR).contains(&year),
            "year out of range"
        );
        assert!((1..=12).contains(&month), "month out of range");
        assert!((1..=31).contains(&day), "day out of range");
        Self { year, month, day }
    }

    #[must_use]
    /// Creates a new `Date` from a DOS encoded date.
    pub(crate) fn decode(dos_date: DosDate) -> Self {
        let dos_date = dos_date.dos_date();
        let year = (dos_date >> 9) + Self::MIN_YEAR;
        let month = u8::try_from((dos_date >> 5) & 0xF).unwrap();
        let day = u8::try_from(dos_date & 0x1F).unwrap();
        Self { year, month, day }
    }

    #[must_use]
    /// Encode the date into a DOS compatible format.
    pub(crate) fn encode(self) -> DosDate {
        let dos_date = ((self.year - Self::MIN_YEAR) << 9)
            | (u16::from(self.month) << 5)
            | u16::from(self.day);
        DosDate::new(dos_date)
    }

    #[must_use]
    #[inline]
    pub const fn year(&self) -> u16 {
        self.year
    }

    #[must_use]
    #[inline]
    pub const fn month(&self) -> u8 {
        self.month
    }

    #[must_use]
    #[inline]
    pub const fn day(&self) -> u8 {
        self.day
    }
}

/// A DOS time.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[non_exhaustive]
pub struct Time {
    /// Hours.
    /// Valid range is [0, 23]
    hour: u8,
    /// Minutes.
    /// Valid range is [0, 59]
    min: u8,
    /// Seconds.
    /// Valid range is [0, 59]
    sec: u8,
    /// Milliseconds.
    /// Valid range is [0, 999]
    ms: u16,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) struct DosTime {
    dos_time: u16,
    dos_time_hi_res: u8,
}

impl DosTime {
    #[must_use]
    #[inline]
    pub const fn new(dos_time: u16, dos_time_hi_res: u8) -> Self {
        Self {
            dos_time,
            dos_time_hi_res,
        }
    }

    #[must_use]
    #[inline]
    pub const fn dos_time(&self) -> u16 {
        self.dos_time
    }

    #[must_use]
    #[inline]
    pub const fn dos_time_hi_res(&self) -> u8 {
        self.dos_time_hi_res
    }
}

impl Time {
    /// Creates a new `Time` instance.
    ///
    /// # Panics
    ///
    /// Panics if one of provided arguments is out of the supported range.
    #[must_use]
    pub fn new(hour: u8, min: u8, sec: u8, ms: u16) -> Self {
        assert!(hour <= 23 && min <= 59 && sec <= 59 && ms <= 999);
        Self { hour, min, sec, ms }
    }

    #[must_use]
    pub(crate) fn decode(dos_time: DosTime) -> Self {
        let dos_time_hi_res = dos_time.dos_time_hi_res();
        let dos_time = dos_time.dos_time();
        let hour = u8::try_from(dos_time >> 11).unwrap();
        let min = u8::try_from((dos_time >> 5) & 0x3F).unwrap();
        let sec = u8::try_from((dos_time & 0x1F) * 2 + u16::from(dos_time_hi_res / 100)).unwrap();
        let ms = u16::from(dos_time_hi_res % 100) * 10;
        Self { hour, min, sec, ms }
    }

    #[must_use]
    pub(crate) fn encode(self) -> DosTime {
        let dos_time =
            (u16::from(self.hour) << 11) | (u16::from(self.min) << 5) | (u16::from(self.sec) / 2);
        let dos_time_hi_res = u8::try_from(self.ms / 10).unwrap() + (self.sec % 2) * 100;
        DosTime::new(dos_time, dos_time_hi_res)
    }
}

/// A DOS date and time.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct DateTime {
    date: Date,
    time: Time,
}

pub(crate) struct DosDateTime {
    dos_date: DosDate,
    dos_time: DosTime,
}

impl DosDateTime {
    #[must_use]
    #[inline]
    pub const fn new(dos_date: DosDate, dos_time: DosTime) -> Self {
        Self { dos_date, dos_time }
    }

    #[must_use]
    #[inline]
    pub const fn dos_date(&self) -> DosDate {
        self.dos_date
    }

    #[must_use]
    #[inline]
    pub const fn dos_time(&self) -> DosTime {
        self.dos_time
    }
}

impl DateTime {
    #[must_use]
    #[inline]
    pub fn new(date: Date, time: Time) -> Self {
        Self { date, time }
    }

    #[must_use]
    pub(crate) fn decode(dos_datetime: DosDateTime) -> Self {
        let dos_date = dos_datetime.dos_date();
        let dos_time = dos_datetime.dos_time();
        Self::new(Date::decode(dos_date), Time::decode(dos_time))
    }
}

/// A current time and date provider.
pub trait TimeProvider {
    fn get_current_date(&self) -> Date;
    fn get_current_time(&self) -> Time;
    fn get_current_date_time(&self) -> DateTime {
        DateTime::new(self.get_current_date(), self.get_current_time())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DosMinTimeProvider;

impl DosMinTimeProvider {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self
    }
}

impl TimeProvider for DosMinTimeProvider {
    fn get_current_date(&self) -> Date {
        // DOS minimum date is 1980-01-01.
        Date::new(1980, 1, 1)
    }

    fn get_current_time(&self) -> Time {
        // DOS minimum time is 00:00:00.000.
        Time::new(0, 0, 0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::{Date, DateTime, DosDate, DosDateTime, DosTime, Time};

    #[test]
    fn date() {
        let _ = Date::new(1980, 1, 1);
        let _ = Date::new(2107, 12, 31);
    }

    #[test]
    #[should_panic = "year out of range"]
    fn date_too_early_panic() {
        let _ = Date::new(1979, 12, 31);
    }

    #[test]
    #[should_panic = "year out of range"]
    fn date_too_late_panic() {
        let _ = Date::new(2108, 1, 1);
    }

    #[test]
    fn date_encode_decode() {
        let date = Date::new(2055, 7, 23);
        let encoded = date.encode();
        let decoded = Date::decode(encoded);
        assert_eq!(encoded.dos_date(), 38647);
        assert_eq!(date, decoded);
    }

    #[test]
    fn time_encode_decode() {
        let t1 = Time::new(15, 3, 29, 990);
        let t2 = Time { sec: 18, ..t1 };
        let t3 = Time { ms: 40, ..t1 };
        let dt1 = t1.encode();
        let dt2 = t2.encode();
        let dt3 = t3.encode();
        assert_eq!((dt1.dos_time(), dt1.dos_time_hi_res()), (30830, 199));
        assert_eq!((dt2.dos_time(), dt2.dos_time_hi_res()), (30825, 99));
        assert_eq!((dt3.dos_time(), dt3.dos_time_hi_res()), (30830, 104));
        assert_eq!(t1, Time::decode(dt1));
        assert_eq!(t2, Time::decode(dt2));
        assert_eq!(t3, Time::decode(dt3));
    }
}
