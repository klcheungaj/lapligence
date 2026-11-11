// SIM-040 negative: a structure formal with a string member and a member
// array of 16 or more packed cells is stored as columns beside a native value,
// which the import thunk does not join. An array of such structures is
// marshalled.
module tb;
    typedef struct { string s; int big [20]; } wide_t;
    import "DPI-C" function void neg_wide(input wide_t v);
    wide_t v;
    initial neg_wide(v);
endmodule
