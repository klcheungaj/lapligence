// IEEE 1800-2009 11.9: the member expression of a tagged union expression
// must have the member's type; an unpacked record payload needs a record
// value, not an integral one.
typedef struct { logic [3:0] lo; logic [3:0] hi; } pair_t;
typedef union tagged { void Empty; pair_t Pair; } item_t;

module tb;
    item_t value;
    initial value = tagged Pair (8'h12);
endmodule
