// Civil dates and wall-clock times deliberately have no time zone or timestamp.
export function dateValue(value) {
    if (value === '') return '';
    if (typeof value !== 'string' || !/^\d{4}-\d{2}-\d{2}$/u.test(value)) throw new TypeError('Date must use YYYY-MM-DD');
    const [year, month, day] = value.split('-').map(Number);
    if (year < 1 || month < 1 || month > 12 || day < 1 || day > daysInMonth(year, month)) throw new RangeError('Choose a real calendar date');
    return value;
}
export function daysInMonth(year, month) {
    return [31, year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1];
}
export function civilDate(year, month, day) {
    return `${String(year).padStart(4, '0')}-${String(month).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
}
export function dateObject(value) {
    const date = new Date(0), [year, month, day] = dateValue(value).split('-').map(Number);
    date.setUTCFullYear(year, month - 1, day); date.setUTCHours(12, 0, 0, 0);
    return date;
}
export function addDays(value, count) {
    const date = dateObject(value); date.setUTCDate(date.getUTCDate() + count);
    return civilDate(date.getUTCFullYear(), date.getUTCMonth() + 1, date.getUTCDate());
}
export function addMonths(value, count) {
    const [year, month, day] = value.split('-').map(Number), index = year * 12 + month - 1 + count;
    const nextYear = Math.floor(index / 12), nextMonth = index % 12 + 1;
    return civilDate(nextYear, nextMonth, Math.min(day, daysInMonth(nextYear, nextMonth)));
}
export function localToday(now = new Date()) { return civilDate(now.getFullYear(), now.getMonth() + 1, now.getDate()); }
export function timeValue(value) {
    if (value === '') return '';
    if (typeof value !== 'string' || !/^\d{2}:\d{2}(?::\d{2})?$/u.test(value)) throw new TypeError('Time must use HH:MM or HH:MM:SS');
    const [hour, minute, second = 0] = value.split(':').map(Number);
    if (hour > 23 || minute > 59 || second > 59) throw new RangeError('Choose a time from 00:00 through 23:59:59');
    return value;
}
export function timeSeconds(value) { const [h, m, s = 0] = timeValue(value).split(':').map(Number); return h * 3600 + m * 60 + s; }
export function clockTime(seconds, showSeconds = false) {
    const parts = [Math.floor(seconds / 3600), Math.floor(seconds % 3600 / 60)];
    if (showSeconds) parts.push(seconds % 60);
    return parts.map(part => String(part).padStart(2, '0')).join(':');
}
export function parseTime(text, showSeconds = false) {
    if (!text.trim()) return '';
    const match = /^(\d{1,2})(?::(\d{2}))?(?::(\d{2}))?\s*(am|pm)?$/iu.exec(text.trim());
    if (!match) throw new TypeError('Enter a time such as 09:30 or 9:30 AM');
    let hour = Number(match[1]); const minute = Number(match[2] || 0), second = Number(match[3] || 0);
    if (hour > 23 || minute > 59 || second > 59) throw new RangeError('Choose a real clock time');
    if (match[4]) {
        if (hour < 1 || hour > 12) throw new RangeError('AM/PM hours run from 1 through 12');
        hour = hour % 12 + (match[4].toLowerCase() === 'pm' ? 12 : 0);
    } else if (!match[2]) throw new TypeError('Include minutes, or AM/PM');
    if (!showSeconds && second !== 0) throw new RangeError('Seconds are not enabled');
    return timeValue(clockTime(hour * 3600 + minute * 60 + second, showSeconds));
}
