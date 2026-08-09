# ADR 0003: Project exact quota windows onto every touched natural date

Quota windows remain strict 604800-second half-open intervals, while the ledger and UI project every policy-timezone natural date that intersects that interval. A non-midnight reset therefore usually produces eight date cells rather than dropping the reset date; the seven-day policy template remains seven Monday-through-Sunday weights and does not define the display count. This avoids making the reset day—and its daily availability and alerts—disappear before the actual reset boundary.
