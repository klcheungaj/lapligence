// IEEE 1800-2009 7.3.2 permits real members in unpacked tagged unions; this
// backend has no fixed packed representation for them yet (SIM-007), so the
// declaration is rejected instead of being stored as integral bits.
typedef union tagged { void Empty; real Level; } item_t;

module tb;
    item_t value;
    initial value = tagged Level (1.5);
endmodule
